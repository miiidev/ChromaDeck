// ── NVCP transfer math & ramp builder (behavioral reimplementation) ──────
// Windows-only, x64-only. Pure math only — no Win32, no NVAPI calls here.
//
// Behavioral source: nvBrightness by Pete Batard (GPL-3.0).
//   https://github.com/pbatard/nvBrightness
// NO code was copied; the math below is a behavioral reimplementation of
// the transfer function, scale mapping, and ramp layout observed in
// nvBrightness's CalculateGamma logic by observing the driver's behavior
// and confirmed against an RTX 3050 (driver 610.74).

#![allow(dead_code)]

use crate::nvapi;

// ── Scale mapping (UI ↔ internal) ────────────────────────────────────────
// Internal range 80–120 where 100 = neutral.
// UI scale 0–100 maps as: internal = 80 + ui × 0.4

pub fn ui_to_internal(ui: f64) -> f64 {
    80.0 + ui * 0.4
}

pub fn internal_to_ui(v: f64) -> f64 {
    (v - 80.0) / 0.4
}

// ── NVCP transfer function ────────────────────────────────────────────────
// Behavioral reimplementation of nvBrightness's CalculateGamma:
//   c = (contrast−100)/100
//   shaped = if c ≤ 0 { (c+1)(x−0.5) } else { (x−0.5)/(1−c) }
//   v = (b−100)/100 + shaped + 0.5, clamped
//   out = pow(v, 1/(g/100)), clamped
// with x = index/1023, per-channel float 0..1.
//
// The spec also uses gamma_exp (exponent) directly rather than g/100.
// We receive gamma_exp already as the UI exponent (1.0–3.0).

/// Compute a single ramp value for channel index `i` (0..1023) given
/// internal brightness, contrast, and gamma exponent.
pub fn nvcp_ramp_value(
    index: u16,
    brightness: f64, // internal: 80–120, 100 = neutral
    contrast: f64,   // internal: 80–120, 100 = neutral
    gamma_exp: f64,  // gamma exponent (1.0–3.0)
) -> f32 {
    let x = index as f64 / 1023.0;
    let c = (contrast - 100.0) / 100.0;
    let shaped = if c <= 0.0 {
        (c + 1.0) * (x - 0.5)
    } else {
        (x - 0.5) / (1.0 - c)
    };
    let v = ((brightness - 100.0) / 100.0 + shaped + 0.5).clamp(0.0, 1.0);
    let out = v.powf(1.0 / gamma_exp).clamp(0.0, 1.0);
    out as f32
}

/// Build a 3072-entry channel-interleaved gamma ramp.
/// Layout: `[i * 3 + c]` for channel c (0=R, 1=G, 2=B).
pub fn nvcp_ramp(
    brightness: f64, // internal
    contrast: f64,   // internal
    gamma_exp: f64,  // exponent
) -> [f32; 3072] {
    let mut ramp = [0.0f32; 3072];
    for i in 0..1024 {
        let v = nvcp_ramp_value(i as u16, brightness, contrast, gamma_exp);
        for c in 0..3 {
            ramp[i * 3 + c] = v;
        }
    }
    ramp
}

// ── NV_GAMMA_CORRECTION_EX struct ────────────────────────────────────────
// Observed from live NVAPI calls against an RTX 3050.

/// NV_GAMMA_CORRECTION_EX — the struct passed to `SetTargetGammaCorrection`.
///
/// Version field is computed as `sizeof(Self) | (1 << 16)` → proves 0x13008.
/// `unknown` is always 1 (observed driver behavior).
#[repr(C)]
pub(crate) struct NvGammaRampEx {
    pub version: u32,
    pub ramp: [f32; 3072],
    pub unknown: u32,
}

// ── NVAPI call helpers (wrap raw function pointers) ──────────────────────

/// Resolve the LUID for a given display ID via `GetLUIDFromDisplayID`.
///
/// LUID = little-endian u32 from GUID bytes 4..8, XOR'd with 0xF0000000.
pub(crate) fn display_luid(
    fns: &nvapi::NvapiFns,
    display_id: u32,
) -> Result<u32, String> {
    let mut guid = windows::core::GUID::default();
    // SAFETY: GetLUIDFromDisplayID writes one GUID on success.
    let status = unsafe { (fns.get_luid)(display_id, 1, &mut guid) };
    if status != nvapi::NVAPI_OK {
        return Err(nvapi::status_to_string(fns, status));
    }
    // GUID layout in memory: data1(4), data2(2), data3(2), data4(8)
    // Bytes 4..8 = data2 | (data3 << 16) in little-endian.
    let raw = (guid.data2 as u32) | ((guid.data3 as u32) << 16);
    Ok(raw ^ 0xF0000000)
}

/// Set the target gamma correction for a display via `SetTargetGammaCorrection`.
pub(crate) fn set_target_gamma(
    fns: &nvapi::NvapiFns,
    display_id: u32,
    ramp: &[f32; 3072],
) -> Result<(), String> {
    let gamma = NvGammaRampEx {
        version: (std::mem::size_of::<NvGammaRampEx>() as u32) | (1 << 16),
        ramp: *ramp,
        unknown: 1,
    };
    // SAFETY: SetTargetGammaCorrection reads the struct on success.
    let status = unsafe { (fns.set_gamma)(display_id, &gamma) };
    if status != nvapi::NVAPI_OK {
        return Err(nvapi::status_to_string(fns, status));
    }
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_inputs_yield_identity_float_ramp() {
        let ramp = nvcp_ramp(100.0, 100.0, 1.0);
        for i in 0..1024 {
            let expected = i as f32 / 1023.0;
            for c in 0..3 {
                assert!(
                    (ramp[i * 3 + c] - expected).abs() < 1e-6,
                    "index {i}"
                );
            }
        }
    }

    #[test]
    fn ui_internal_mapping_roundtrips() {
        assert_eq!(ui_to_internal(50.0), 100.0);
        assert_eq!(ui_to_internal(0.0), 80.0);
        assert_eq!(ui_to_internal(100.0), 120.0);
        // internal 88 = 80 + 20*0.4, so ui = (88-80)/0.4 = 20
        assert_eq!(internal_to_ui(88.0), 20.0);
        // roundtrip: ui=60 → internal=104 → ui=60
        assert_eq!(internal_to_ui(ui_to_internal(60.0)), 60.0);
    }

    #[test]
    fn below_normal_crushes_never_lifts() {
        // brightness 90 (< 100): peak must stay below 1.0 AND index 0 must be 0
        let ramp = nvcp_ramp(90.0, 100.0, 1.0);
        assert!(ramp[1023 * 3] < 1.0);
        assert_eq!(ramp[0], 0.0, "black stays black (no fog lift)");
    }

    #[test]
    fn gamma_struct_version_is_computed() {
        assert_eq!(
            (std::mem::size_of::<NvGammaRampEx>() as u32) | (1 << 16),
            0x13008
        );
    }

    #[test]
    fn nvcp_ramp_value_zero_index_is_zero() {
        // At any valid (non-negative) inputs, index 0 should be 0 or very near.
        let v = nvcp_ramp_value(0, 100.0, 100.0, 1.0);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn nvcp_ramp_value_full_index_is_one_at_neutral() {
        let v = nvcp_ramp_value(1023, 100.0, 100.0, 1.0);
        assert!((v - 1.0).abs() < 1e-6);
    }
}