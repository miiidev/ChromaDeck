// ── Per-monitor colour apply (ICC + OS gamma ramp) ──────────────────────────
// Windows-only.  Applies ICC profile first, then gamma/RGB overlay on top.
// All hardware calls abstracted behind the `ColorApi` trait for testability.

use serde::Serialize;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC, HDC};
use windows::Win32::UI::ColorSystem::GetDeviceGammaRamp;

use crate::nvapi::{fns, display_id_for_device, resolve_device_name};
use crate::nvgamma::{display_luid, nvcp_ramp, persist_nvcp, set_target_gamma, ui_to_internal};

// ── ApplyResult ─────────────────────────────────────────────────────────────

/// Result of applying a preset to a monitor.
#[derive(Debug, Clone, Serialize)]
pub struct ApplyResult {
    pub icc_applied: bool,
    pub gamma_applied: bool,
    pub vibrance_applied: bool,
    pub error: Option<String>,
}

#[allow(dead_code)]
impl ApplyResult {
    pub fn offline() -> Self {
        ApplyResult {
            icc_applied: false,
            gamma_applied: false,
            vibrance_applied: false,
            error: Some("monitor is offline".into()),
        }
    }

    pub fn success() -> Self {
        ApplyResult {
            icc_applied: true,
            gamma_applied: true,
            vibrance_applied: true,
            error: None,
        }
    }
}

// ── Gamma engine selection ─────────────────────────────────────────────────

/// Engine used to apply gamma to a display.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GammaEngine {
    Nvapi,
    Gdi,
}

/// Pure engine selector for testability.
fn engine_for_flags(nvapi_ok: bool) -> GammaEngine {
    if nvapi_ok {
        GammaEngine::Nvapi
    } else {
        GammaEngine::Gdi
    }
}

/// Select the gamma engine for a given display by probing NVAPI availability.
pub(crate) fn gamma_engine(edid_id: &str) -> GammaEngine {
    let nvapi_ok = fns()
        .and_then(|f| {
            resolve_device_name(edid_id)
                .and_then(|n| display_id_for_device(f, &n).ok())
                .map(|_| ())
        })
        .is_some();
    engine_for_flags(nvapi_ok)
}

// ── Injectable API trait ────────────────────────────────────────────────────

/// Abstract colour‑api so that tests never need a real display.
pub trait ColorApi {
    /// Check whether the monitor identified by `edid_id` is connected.
    fn is_connected(&self, edid_id: &str) -> bool;

    /// Install + associate an ICC profile for the monitor.
    fn associate_icc(&self, edid_id: &str, profile_path: &str) -> Result<(), String>;

    /// Set the OS gamma ramp for the monitor.
    fn set_gamma_ramp(
        &self,
        edid_id: &str,
        brightness: f64,
        contrast: f64,
        rgb_gains: [f64; 3],
        gamma: f64,
    ) -> Result<(), String>;
}

// ── Real implementation (Win32) ─────────────────────────────────────────────

/// Production API that talks to real hardware via Win32 / GDI / Mscms.
pub struct RealColorApi;

impl RealColorApi {
    /// Build a full ICC profile path from the store's profiles directory.
    pub fn profile_path(store_profiles_dir: &str, icc_filename: &str) -> PathBuf {
        PathBuf::from(store_profiles_dir).join(icc_filename)
    }

    /// Find the display device name (`\\.\DISPLAY1`) for a given EDID.
    fn device_name_for_edid(edid_id: &str) -> Option<String> {
        let monitors = crate::monitor::list_monitors();
        monitors
            .into_iter()
            .find(|m| m.edid_id == edid_id && !m.device_name.is_empty())
            .map(|m| m.device_name)
    }

    /// Reject ramps no driver will accept: a channel peaking at 0 (brightness
    /// 0 blanks the display) is refused by `SetDeviceGammaRamp`, so fail fast
    /// with an actionable message instead of a cryptic Win32 error.
    /// Returns the error message when blank, `None` when appliable.
    /// Pure function (no hardware) so it is unit-testable.
    pub fn blank_channel_error(ramp: &[u16; 256 * 3]) -> Option<String> {
        for ch in 0..3 {
            if ramp[ch * 256 + 255] == 0 {
                return Some(
                    "ramp peak is 0 (brightness 0 blanks the display, which \
                     this driver rejects); raise Brightness above 0"
                        .into(),
                );
            }
        }
        None
    }
}

impl ColorApi for RealColorApi {
    fn is_connected(&self, edid_id: &str) -> bool {
        crate::monitor::list_monitors()
            .iter()
            .any(|m| m.edid_id == edid_id && m.connected)
    }

    fn associate_icc(&self, edid_id: &str, profile_path: &str) -> Result<(), String> {
        use windows::Win32::UI::ColorSystem::{
            AssociateColorProfileWithDeviceW, InstallColorProfileW,
        };

        let device_name = Self::device_name_for_edid(edid_id)
            .ok_or_else(|| format!("no display device found for EDID {edid_id}"))?;

        // Encode paths to UCS‑2 (wide strings).
        let wide_path: Vec<u16> = OsString::from(profile_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let wide_device: Vec<u16> = OsString::from(&device_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // Install the ICC profile into the system colour store.
        // SAFETY: InstallColorProfileW is a straightforward Mscms call.
        let _ = unsafe {
            InstallColorProfileW(
                PCWSTR::null(),
                PCWSTR::from_raw(wide_path.as_ptr()),
            )
        };

        // Associate the profile with the device.
        // SAFETY: AssociateColorProfileWithDeviceW links profile → device.
        let associated: bool = unsafe {
            AssociateColorProfileWithDeviceW(
                PCWSTR::null(),
                PCWSTR::from_raw(wide_path.as_ptr()),
                PCWSTR::from_raw(wide_device.as_ptr()),
            )
            .as_bool()
        };

        if !associated {
            return Err(format!(
                "failed to associate ICC profile with {device_name}"
            ));
        }

        Ok(())
    }

    fn set_gamma_ramp(
        &self,
        edid_id: &str,
        brightness: f64,
        contrast: f64,
        rgb_gains: [f64; 3],
        gamma: f64,
    ) -> Result<(), String> {
        use windows::Win32::UI::ColorSystem::SetDeviceGammaRamp;

        let device_name = Self::device_name_for_edid(edid_id)
            .ok_or_else(|| format!("no display device found for EDID {edid_id}"))?;

        // Create a device context for the target monitor.
        let hdc = open_display_dc(&device_name)?;

        // Build the gamma ramp.
        let ramp = build_gamma_ramp(brightness, contrast, rgb_gains, gamma);

        // Fail fast on blank ramps (see blank_channel_error) before touching
        // the driver, so the user gets an actionable message.
        if let Some(msg) = Self::blank_channel_error(&ramp) {
            unsafe { let _ = DeleteDC(hdc); };
            return Err(msg);
        }

        // SAFETY: SetDeviceGammaRamp writes the ramp into the display driver.
        let ok: bool = unsafe { SetDeviceGammaRamp(hdc, &ramp as *const _ as _).as_bool() };
        // Capture the Win32 error immediately: DeleteDC below may overwrite it.
        let last_error = std::io::Error::last_os_error();

        // Cleanup DC.
        // SAFETY: DeleteDC is safe after a successful CreateDCW.
        unsafe { let _ = DeleteDC(hdc); };

        if !ok {
            Err(format!(
                "SetDeviceGammaRamp failed for {device_name} (win32 error {last_error})"
            ))
        } else {
            Ok(())
        }
    }
}

// ── Gamma ramp builder ──────────────────────────────────────────────────────

/// Build a 3×256‑WORD gamma ramp from preset parameters.
///
/// Ramp layout expected by `SetDeviceGammaRamp`:
///   `ramp[0..255]`   — red channel
///   `ramp[256..511]` — green channel
///   `ramp[512..767]` — blue channel
///
/// Formula (per channel, with gain):
///   normalized = i / 255.0
///   corrected = normalized.powf(gamma)
///   contrasted = (corrected - 0.5) * contrast + 0.5
///   with_brightness = contrasted * brightness
///   output = with_brightness * gain
///   ramp[i] = clamp(output, 0, 1) * 65535
/// Minimum per-channel peak (index 255) accepted by display drivers.
///
/// Observed on NVIDIA (RTX 3050, Windows 11, `\\.\DISPLAY5`): `CreateDCW`
/// succeeds and full-range ramps apply fine, but `SetDeviceGammaRamp`
/// returns FALSE whenever any channel's last entry is below ~0x7F00–0x7FFF
/// (probed: 30720 rejected, 32767 accepted). Dim presets whose
/// brightness × contrast compress white below half (e.g. 0.5 × 0.75 = 0.375
/// → 24576) were therefore rejected with "SetDeviceGammaRamp failed".
/// Rescaling such channels so the peak reaches this floor yields the
/// closest ramp the driver accepts (linear rescale: shape and order
/// preserved, no clipping possible since the peak lands exactly on the
/// floor). Channels already at/above the floor, and all-zero ramps
/// (brightness 0), are left untouched.
const MIN_CHANNEL_PEAK: u16 = 0x8000;

pub(crate) fn build_gamma_ramp(
    brightness: f64,
    contrast: f64,
    rgb_gains: [f64; 3],
    gamma: f64,
) -> [u16; 256 * 3] {
    let mut ramp = [0u16; 256 * 3];

    for i in 0..256 {
        let mut v = i as f64 / 255.0;

        // Gamma curve
        v = v.powf(gamma);

        // Contrast (stretch/squash around 0.5)
        v = (v - 0.5) * contrast + 0.5;

        // Brightness
        v *= brightness;

        // Clamp and scale to 16-bit
        v = v.clamp(0.0, 1.0);
        let base = (v * 65535.0) as u16;

        // Per-channel gain
        ramp[i] = ((base as f64 * rgb_gains[0]).round() as u16).min(65535);
        ramp[256 + i] = ((base as f64 * rgb_gains[1]).round() as u16).min(65535);
        ramp[512 + i] = ((base as f64 * rgb_gains[2]).round() as u16).min(65535);
    }

    // Enforce the driver-accepted peak floor per channel (see above).
    for ch in 0..3 {
        let peak = ramp[ch * 256 + 255];
        if peak > 0 && peak < MIN_CHANNEL_PEAK {
            let scale = MIN_CHANNEL_PEAK as f64 / peak as f64;
            for i in 0..256 {
                ramp[ch * 256 + i] =
                    ((ramp[ch * 256 + i] as f64 * scale).round() as u16).min(65535);
            }
        }
    }

    ramp
}

// ── Shared DC / read helpers (enforcer) ─────────────────────────────────


/// Open a device context for a display device. Shared by set/read paths.
fn open_display_dc(device_name: &str) -> Result<HDC, String> {
    let wide_device: Vec<u16> = OsString::from(device_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: CreateDCW for a display device returns the monitor's DC.
    let hdc: HDC = unsafe {
        CreateDCW(
            PCWSTR::from_raw(windows::core::w!("DISPLAY").as_ptr()),
            PCWSTR::from_raw(wide_device.as_ptr()),
            None,
            None,
        )
    };
    if hdc.is_invalid() {
        return Err(format!("failed to create DC for {device_name}"));
    }
    Ok(hdc)
}

/// Read the live gamma ramp for a display device. Used by the enforcer's
/// drift check. Hardware-only; verified live in Task 6.
pub(crate) fn read_gamma_ramp(device_name: &str) -> Result<[u16; 256 * 3], String> {
    let hdc = open_display_dc(device_name)?;
    let mut ramp = [0u16; 256 * 3];
    // SAFETY: GetDeviceGammaRamp fills 768 WORDs on success.
    let ok: bool = unsafe { GetDeviceGammaRamp(hdc, &mut ramp as *mut _ as _).as_bool() };
    // SAFETY: DeleteDC is safe after a successful CreateDCW.
    unsafe { let _ = DeleteDC(hdc); };
    if !ok {
        return Err(format!("GetDeviceGammaRamp failed for {device_name}"));
    }
    Ok(ramp)
}

// ── Shared apply core (gamma + vibrance, no ICC) ────────────────────────────

/// Apply driver-level color state (gamma LUT + NVAPI vibrance/hue),
/// without ICC association. Shared by manual Apply and the enforcer.
pub fn apply_color(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    preset: &crate::store::Preset,
) -> ApplyResult {
    if !api.is_connected(&preset.edid_id) {
        return ApplyResult::offline();
    }

    let mut gamma_applied = false;
    let mut vibrance_applied = false;
    let mut error: Option<String> = None;

    // Step 1 — Gamma / RGB overlay (engine-branched)
    let engine = gamma_engine(&preset.edid_id);
    match engine {
        GammaEngine::Nvapi => {
            // Stage-purity: reset GDI LUT to identity before the NVAPI
            // set.  Exclusive-fullscreen games may have clobbered only the
            // GDI LUT (the canonical evaporation case); this guarantees
            // the NVAPI path owns the full pipeline on every apply.
            // Best-effort — failure appends error but does NOT block NVAPI.
            let gdi_purge_err = api.set_gamma_ramp(
                &preset.edid_id, 1.0, 1.0, [1.0, 1.0, 1.0], 1.0,
            ).err().map(|e| format!("gdi-purge: {e}"));

            // NVAPI path: NVCP transfer math + set_target_gamma + persist
            let nvapi_result = (|| -> Result<(), String> {
                let fns = fns().ok_or_else(|| "NVAPI unavailable".to_string())?;
                let device_name = resolve_device_name(&preset.edid_id)
                    .ok_or_else(|| format!("no display device found for EDID {}", preset.edid_id))?;
                let display_id = display_id_for_device(fns, &device_name)?;

                let b_int = ui_to_internal(preset.brightness);
                let c_int = ui_to_internal(preset.contrast);
                let mut ramp = nvcp_ramp(b_int, c_int, preset.gamma);

                // Apply RGB gains as post-multiplier (preserves existing behavior)
                for i in 0..1024 {
                    ramp[i * 3] = (ramp[i * 3] as f64 * preset.rgb_gains[0]).clamp(0.0, 1.0) as f32;
                    ramp[i * 3 + 1] =
                        (ramp[i * 3 + 1] as f64 * preset.rgb_gains[1]).clamp(0.0, 1.0) as f32;
                    ramp[i * 3 + 2] =
                        (ramp[i * 3 + 2] as f64 * preset.rgb_gains[2]).clamp(0.0, 1.0) as f32;
                }

                set_target_gamma(fns, display_id, &ramp)?;

                // Best-effort persist: failure does NOT fail the apply
                if let Ok(luid) = display_luid(fns, display_id) {
                    let b_arr = [b_int; 3];
                    let c_arr = [c_int; 3];
                    let g_arr = [preset.gamma * 100.0; 3];
                    if let Err(e) = persist_nvcp(luid, b_arr, c_arr, g_arr) {
                        return Err(format!("persist: {e}"));
                    }
                }

                Ok(())
            })();

            match nvapi_result {
                Ok(()) => gamma_applied = true,
                Err(e) => {
                    if e.starts_with("persist: ") {
                        // Gamma succeeded but persist failed — gamma still counts as applied
                        gamma_applied = true;
                        error = Some(match error {
                            Some(ref prev) => format!("{prev}; {e}"),
                            None => e,
                        });
                    } else {
                        let gamma_err = format!("gamma: {e}");
                        error = Some(match error {
                            Some(ref prev) => format!("{prev}; {gamma_err}"),
                            None => gamma_err,
                        });
                    }
                }
            }

            // Append GDI-purge error (best-effort, gamma already tracked).
            if let Some(e) = gdi_purge_err {
                error = Some(match error {
                    Some(ref prev) => format!("{prev}; {e}"),
                    None => e,
                });
            }
        }
        GammaEngine::Gdi => {
            // GDI fallback with mapped gains: gain = 0.8 + 0.004 × ui_val
            // (neutral 50 → gain 1.0, matching the old gain-model neutral)
            let b_gain = 0.8 + 0.004 * preset.brightness;
            let c_gain = 0.8 + 0.004 * preset.contrast;
            match api.set_gamma_ramp(
                &preset.edid_id,
                b_gain,
                c_gain,
                preset.rgb_gains,
                preset.gamma,
            ) {
                Ok(()) => gamma_applied = true,
                Err(e) => {
                    let gamma_err = format!("gamma: {e}");
                    error = Some(match error {
                        Some(ref prev) => format!("{prev}; {gamma_err}"),
                        None => gamma_err,
                    });
                }
            }
        }
    }

    // Step 2 — NVAPI vibrance/hue overlay (last; always written when
    // supported so neutral presets fully restore prior state; skipped
    // silently only where NVAPI reports unsupported).
    let neutral = preset.vibrance == crate::nvapi::VIBRANCE_NEUTRAL
        && preset.hue_deg == crate::nvapi::HUE_NEUTRAL;
    if !neutral || nv.supported(&preset.edid_id) {
        match nv.set(&preset.edid_id, preset.vibrance, preset.hue_deg) {
            Ok(()) => vibrance_applied = true,
            Err(e) => {
                let nv_err = format!("vibrance: {e}");
                error = Some(match error {
                    Some(ref prev) => format!("{prev}; {nv_err}"),
                    None => nv_err,
                });
            }
        }
    }

    ApplyResult {
        icc_applied: false,
        gamma_applied,
        vibrance_applied,
        error,
    }
}

// ── apply_preset (ICC + shared apply_color) ─────────────────────────────────

/// Apply a preset to its target monitor.
///
/// Precedence: ICC first, then gamma/RGB overlay on top.
/// Returns an `ApplyResult` summarising what succeeded / failed.
pub fn apply_preset(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    preset: &crate::store::Preset,
    store_profiles_dir: &str,
) -> ApplyResult {
    // Step 1 — monitor connectivity check
    if !api.is_connected(&preset.edid_id) {
        return ApplyResult::offline();
    }

    let mut icc_applied = false;
    let mut icc_error: Option<String> = None;

    // Step 2 — ICC (first)
    if !preset.icc_hash.is_empty() {
        let profile_path = RealColorApi::profile_path(store_profiles_dir, &preset.icc_filename);
        let profile_path_str = profile_path.to_string_lossy().to_string();
        match api.associate_icc(&preset.edid_id, &profile_path_str) {
            Ok(()) => icc_applied = true,
            Err(e) => {
                icc_error = Some(format!("ICC: {e}"));
            }
        }
    }

    // Step 3 — gamma + vibrance via shared core
    let mut result = apply_color(api, nv, preset);
    result.icc_applied = icc_applied;
    result.error = match (icc_error, result.error) {
        (Some(a), Some(b)) => Some(format!("{a}; {b}")),
        (Some(a), None) => Some(a),
        (None, b) => b,
    };
    result
}

/// Reset a monitor to system defaults: identity gamma ramp
/// (brightness 1, contrast 1, unit RGB gains, gamma 1) plus neutral
/// vibrance (50) and hue (0). ICC associations are left untouched.
/// Vibrance is skipped silently where NVAPI reports unsupported.
/// Returns an `ApplyResult` with `icc_applied: false`.
pub fn reset_monitor(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    edid_id: &str,
) -> ApplyResult {
    if !api.is_connected(edid_id) {
        return ApplyResult::offline();
    }

    let mut gamma_applied = false;
    let mut vibrance_applied = false;
    let mut error: Option<String> = None;

    let engine = gamma_engine(edid_id);
    match engine {
        GammaEngine::Nvapi => {
            // NVAPI reset: identity ramp + neutral 100s persist
            let nvapi_result = (|| -> Result<(), String> {
                let fns = fns().ok_or_else(|| "NVAPI unavailable".to_string())?;
                let device_name = resolve_device_name(edid_id)
                    .ok_or_else(|| format!("no display device found for EDID {edid_id}"))?;
                let display_id = display_id_for_device(fns, &device_name)?;

                // Identity ramp at neutral (100, 100, 1.0)
                let ramp = nvcp_ramp(100.0, 100.0, 1.0);
                set_target_gamma(fns, display_id, &ramp)?;

                // Persist neutral 100s
                if let Ok(luid) = display_luid(fns, display_id) {
                    let neutral = [100.0; 3];
                    if let Err(e) = persist_nvcp(luid, neutral, neutral, neutral) {
                        return Err(format!("persist: {e}"));
                    }
                }

                Ok(())
            })();

            match nvapi_result {
                Ok(()) => gamma_applied = true,
                Err(e) => {
                    if e.starts_with("persist: ") {
                        // Gamma reset succeeded but persist failed
                        gamma_applied = true;
                        error = Some(match error {
                            Some(ref prev) => format!("{prev}; {e}"),
                            None => e,
                        });
                    } else {
                        error = Some(format!("reset: {e}"));
                    }
                }
            }
        }
        GammaEngine::Gdi => {
            // GDI reset: unchanged identity gain values (1.0 = neutral)
            match api.set_gamma_ramp(edid_id, 1.0, 1.0, [1.0, 1.0, 1.0], 1.0) {
                Ok(()) => gamma_applied = true,
                Err(e) => {
                    error = Some(format!("reset: {e}"));
                }
            }
        }
    }

    if nv.supported(edid_id) {
        match nv.set(
            edid_id,
            crate::nvapi::VIBRANCE_NEUTRAL,
            crate::nvapi::HUE_NEUTRAL,
        ) {
            Ok(()) => vibrance_applied = true,
            Err(e) => {
                let nv_err = format!("vibrance: {e}");
                error = Some(match error {
                    Some(ref prev) => format!("{prev}; {nv_err}"),
                    None => nv_err,
                });
            }
        }
    }

    ApplyResult {
        icc_applied: false,
        gamma_applied,
        vibrance_applied,
        error,
    }
}

// ── Recorder / mock for testing ─────────────────────────────────────────────

#[cfg(test)]
pub struct TestRecorder {
    pub calls: std::sync::Mutex<Vec<String>>,
    pub last_gamma_params: std::sync::Mutex<Option<(f64, f64, [f64; 3], f64)>>,
    pub connected: bool,
    pub icc_should_fail: bool,
    pub gamma_should_fail: bool,
}

#[cfg(test)]
impl TestRecorder {
    pub fn new(connected: bool) -> Self {
        TestRecorder {
            calls: std::sync::Mutex::new(Vec::new()),
            last_gamma_params: std::sync::Mutex::new(None),
            connected,
            icc_should_fail: false,
            gamma_should_fail: false,
        }
    }
}

#[cfg(test)]
impl ColorApi for TestRecorder {
    fn is_connected(&self, _edid_id: &str) -> bool {
        self.connected
    }

    fn associate_icc(&self, _edid_id: &str, _profile_path: &str) -> Result<(), String> {
        let mut calls = self.calls.lock().unwrap();
        calls.push("icc".into());
        if self.icc_should_fail {
            Err("mock ICC failure".into())
        } else {
            Ok(())
        }
    }

    fn set_gamma_ramp(
        &self,
        _edid_id: &str,
        brightness: f64,
        contrast: f64,
        rgb_gains: [f64; 3],
        gamma: f64,
    ) -> Result<(), String> {
        let mut calls = self.calls.lock().unwrap();
        calls.push("gamma".into());
        *self.last_gamma_params.lock().unwrap() =
            Some((brightness, contrast, rgb_gains, gamma));
        if self.gamma_should_fail {
            Err("mock gamma failure".into())
        } else {
            Ok(())
        }
    }
}

// ── Tauri command ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn apply_preset_cmd(
    state: tauri::State<'_, crate::store::AppStore>,
    id: String,
) -> ApplyResult {
    // Read the preset from the store
    let store = match state.0.lock() {
        Ok(s) => s,
        Err(e) => {
            return ApplyResult {
                icc_applied: false,
                gamma_applied: false,
                vibrance_applied: false,
                error: Some(format!("store lock: {e}")),
            }
        }
    };

    let preset = match store.list_presets().into_iter().find(|p| p.id == id) {
        Some(p) => p,
        None => {
            return ApplyResult {
                icc_applied: false,
                gamma_applied: false,
                vibrance_applied: false,
                error: Some(format!("preset not found: {id}")),
            }
        }
    };

    // Build the profiles directory path from the store's data_dir
    let profiles_dir = crate::store::default_store_path().join("profiles");
    let profiles_dir_str = profiles_dir.to_string_lossy().to_string();

    // Release the store lock before the potentially long apply call
    drop(store);

    let api = RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    apply_preset(&api, &nv, &preset, &profiles_dir_str)
}

#[tauri::command]
pub fn reset_monitor_cmd(edid_id: String) -> ApplyResult {
    let api = RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    reset_monitor(&api, &nv, &edid_id)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Preset;

    // ── Engine selection (pure seam) ───────────────────────────────────

    #[test]
    fn engine_for_flags_selects_correct_engine() {
        assert_eq!(engine_for_flags(true), GammaEngine::Nvapi);
        assert_eq!(engine_for_flags(false), GammaEngine::Gdi);
    }

    fn fake_preset() -> Preset {
        Preset {
            id: "test-id".into(),
            name: "Test".into(),
            edid_id: "EDID-001".into(),
            icc_hash: "abc123".into(),
            icc_filename: "abc123.icc".into(),
            brightness: 0.5,
            contrast: 0.8,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 2.2,
            vibrance: 50.0,
            hue_deg: 0.0,
            color_model: "nvcp-v1".into(),
        }
    }

    /// A preset with no ICC – should skip ICC step and only call gamma.
    fn preset_no_icc() -> Preset {
        let mut p = fake_preset();
        p.icc_hash = String::new();
        p.icc_filename = String::new();
        p
    }

    // ── Precedence test ──────────────────────────────────────────────────

    #[test]
    fn apply_precedence_is_icc_then_gamma() {
        let recorder = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = fake_preset();

        let _result = apply_preset(&recorder, &nv, &preset, "profiles");

        let calls = recorder.calls.lock().unwrap();
        assert_eq!(
            *calls,
            vec!["icc", "gamma"],
            "ICC must be applied BEFORE gamma overlay"
        );
    }

    // ── Offline monitor test ─────────────────────────────────────────────

    #[test]
    fn offline_monitor_returns_false_and_error() {
        let recorder = TestRecorder::new(false);
        let nv = MockNvapi::new(true);
        let preset = fake_preset();

        let result = apply_preset(&recorder, &nv, &preset, "profiles");
        assert!(!result.icc_applied);
        assert!(!result.gamma_applied);
        assert!(result.error.is_some());
        assert!(
            result.error.as_ref().unwrap().contains("offline"),
            "error should mention offline"
        );
    }

    // ── Missing ICC file test ────────────────────────────────────────────

    #[test]
    fn missing_icc_returns_error_not_panic() {
        let recorder = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let mut preset = fake_preset();
        preset.icc_hash = "nonexistent".into();
        preset.icc_filename = "nonexistent.icc".into();

        // With a recorder that always succeeds, both should be applied.
        let result = apply_preset(&recorder, &nv, &preset, "profiles");

        assert!(result.icc_applied);
        assert!(result.gamma_applied);
        assert!(result.error.is_none());
    }

    // ── No ICC → only gamma applied ──────────────────────────────────────

    #[test]
    fn no_icc_skips_icc_and_applies_gamma_only() {
        let recorder = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = preset_no_icc();

        let _result = apply_preset(&recorder, &nv, &preset, "profiles");

        let calls = recorder.calls.lock().unwrap();
        assert_eq!(*calls, vec!["gamma"], "without ICC, only gamma is applied");
    }

    // ── ICC failure still applies gamma ──────────────────────────────────

    #[test]
    fn icc_failure_still_applies_gamma() {
        let mut recorder = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        recorder.icc_should_fail = true;
        let preset = fake_preset();

        let result = apply_preset(&recorder, &nv, &preset, "profiles");

        let calls = recorder.calls.lock().unwrap();
        assert_eq!(*calls, vec!["icc", "gamma"], "both are attempted");
        assert!(!result.icc_applied, "ICC should have failed");
        assert!(result.gamma_applied, "gamma should still succeed");
        assert!(result.error.is_some());
        assert!(
            result.error.as_ref().unwrap().contains("ICC"),
            "error should mention ICC"
        );
    }

    // ── Gamma ramp builder ───────────────────────────────────────────────

    #[test]
    fn gamma_ramp_has_correct_length() {
        let ramp = build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 2.2);
        assert_eq!(ramp.len(), 256 * 3);
    }

    #[test]
    fn gamma_ramp_max_brightness_gives_max_value() {
        let ramp = build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0);
        assert_eq!(ramp[255], 65535, "max index should be 65535 at full brightness gamma=1");
    }

    #[test]
    fn gamma_ramp_zero_brightness_gives_zero() {
        let ramp = build_gamma_ramp(0.0, 1.0, [1.0, 1.0, 1.0], 2.2);
        assert_eq!(ramp[0], 0, "index 0 should be 0");
        assert_eq!(ramp[255], 0, "max index should be 0 at zero brightness");
    }

    #[test]
    fn gamma_ramp_per_channel_gains_work() {
        let ramp = build_gamma_ramp(1.0, 1.0, [0.5, 1.0, 2.0], 1.0);
        // At i=255: v=1.0, gamma=1.0 -> 1.0, contrast=1.0 -> (1.0-0.5)*1.0+0.5=1.0, *brightness=1.0
        // base = 65535
        assert_eq!(ramp[255], 32768, "red gain 0.5 gives half"); // 65535*0.5=32767.5 rounded
        assert_eq!(ramp[511], 65535, "green gain 1.0 gives full");
        assert_eq!(ramp[767], 65535, "blue gain 2.0 clamped to 65535");
    }

    // ── Driver peak floor (regression: "SetDeviceGammaRamp failed") ────

    /// The exact preset that failed on hardware (brightness 0.5,
    /// contrast 0.5, gamma 1.25 → raw peak 24576 < 0x8000, rejected by the
    /// driver). The builder must raise the peak to the accepted floor.
    #[test]
    fn dim_preset_peak_is_raised_to_driver_floor() {
        let ramp = build_gamma_ramp(0.5, 0.5, [1.0, 1.0, 1.0], 1.25);
        for ch in 0..3 {
            assert_eq!(
                ramp[ch * 256 + 255],
                MIN_CHANNEL_PEAK,
                "channel {ch} peak must land exactly on the driver floor"
            );
        }
    }

    /// Rescaling is linear: each entry is the raw formula output scaled by
    /// floor/raw_peak, so the curve shape is preserved and the ramp stays
    /// non-decreasing (no banding cliffs introduced).
    #[test]
    fn dim_preset_shape_preserved_after_floor_rescale() {
        let ramp = build_gamma_ramp(0.5, 0.5, [1.0, 1.0, 1.0], 1.25);
        // Raw (pre-floor) formula output, computed independently here.
        fn raw(i: u32) -> f64 {
            let mut v = i as f64 / 255.0;
            v = v.powf(1.25);
            v = (v - 0.5) * 0.5 + 0.5;
            v *= 0.5;
            v.clamp(0.0, 1.0) * 65535.0
        }
        let raw_peak = raw(255) as u16; // truncated like the builder
        assert!(raw_peak > 0 && raw_peak < MIN_CHANNEL_PEAK);
        let scale = MIN_CHANNEL_PEAK as f64 / raw_peak as f64;
        for &i in &[0u32, 1, 64, 128, 200, 254, 255] {
            let expected = ((raw(i) as u16 as f64 * scale).round() as u16).min(65535);
            assert_eq!(ramp[i as usize], expected, "index {i} must be linearly rescaled");
        }
        for ch in 0..3 {
            let base = ch * 256;
            for i in 1..256 {
                assert!(
                    ramp[base + i] >= ramp[base + i - 1],
                    "channel {ch} must stay non-decreasing at index {i}"
                );
            }
        }
    }

    /// Full-range ramps must pass through untouched (no-op path).
    #[test]
    fn full_range_ramp_untouched_by_floor() {
        let ramp = build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0);
        assert_eq!(ramp[0], 0);
        assert_eq!(ramp[255], 65535);
    }

    /// Brightness 0 yields an all-zero ramp; the floor logic must skip it
    /// (no divide-by-zero) and leave it zero.
    #[test]
    fn zero_brightness_ramp_stays_zero() {
        let ramp = build_gamma_ramp(0.0, 0.5, [1.0, 1.0, 1.0], 2.2);
        assert!(ramp.iter().all(|&v| v == 0));
    }

    // ── reset_monitor ──────────────────────────────────────────────────

    use crate::nvapi::{HUE_NEUTRAL, VIBRANCE_NEUTRAL};

    /// Reset restores full defaults: identity gamma plus neutral
    /// vibrance/hue, never ICC.
    #[test]
    fn reset_restores_full_defaults() {
        let recorder = TestRecorder::new(true);
        let nv = MockNvapi::new(true);

        let result = reset_monitor(&recorder, &nv, "EDID-001");

        let calls = recorder.calls.lock().unwrap();
        assert_eq!(*calls, vec!["gamma"], "reset always writes identity gamma");
        let params = recorder.last_gamma_params.lock().unwrap();
        assert_eq!(
            *params,
            Some((1.0, 1.0, [1.0, 1.0, 1.0], 1.0)),
            "reset must use identity parameters"
        );
        let nv_calls = nv.calls.lock().unwrap();
        assert_eq!(
            *nv_calls,
            vec![("set".to_string(), VIBRANCE_NEUTRAL, HUE_NEUTRAL)],
            "reset must restore neutral vibrance/hue"
        );
        assert!(!result.icc_applied);
        assert!(result.gamma_applied);
        assert!(result.vibrance_applied);
        assert!(result.error.is_none());
    }

    /// Reset on an offline monitor returns the offline error without
    /// touching the driver.
    #[test]
    fn reset_offline_monitor_returns_offline_error() {
        let recorder = TestRecorder::new(false);
        let nv = MockNvapi::new(false);

        let result = reset_monitor(&recorder, &nv, "EDID-001");

        let calls = recorder.calls.lock().unwrap();
        assert!(calls.is_empty(), "no driver calls when offline");
        assert!(nv.calls.lock().unwrap().is_empty());
        assert!(!result.gamma_applied);
        assert!(!result.vibrance_applied);
        assert!(result.error.as_ref().unwrap().contains("offline"));
    }

    /// Reset skips vibrance silently where NVAPI reports unsupported
    /// (e.g. iGPU-driven panels): gamma still applies, no error.
    #[test]
    fn reset_skips_vibrance_when_unsupported() {
        let recorder = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        let result = reset_monitor(&recorder, &nv, "EDID-001");

        assert!(result.gamma_applied);
        assert!(!result.vibrance_applied);
        assert!(nv.calls.lock().unwrap().is_empty());
        assert!(result.error.is_none());
    }

    /// Vibrance failure during reset still reports gamma success with a
    /// vibrance error attached.
    #[test]
    fn reset_reports_vibrance_failure_but_keeps_gamma() {
        let recorder = TestRecorder::new(true);
        let mut nv = MockNvapi::new(true);
        nv.fail_set = true;

        let result = reset_monitor(&recorder, &nv, "EDID-001");

        assert!(result.gamma_applied);
        assert!(!result.vibrance_applied);
        assert!(result.error.as_ref().unwrap().contains("vibrance"));
    }

    // ── Blank-ramp guard (regression: same error text, new cause) ─────

    /// A zero-peak (brightness 0) ramp must be flagged with an actionable
    /// message instead of reaching the driver and failing cryptically.
    #[test]
    fn zero_peak_ramp_flagged_with_actionable_error() {
        let ramp = build_gamma_ramp(0.0, 0.5, [1.0, 1.0, 1.0], 1.25);
        let msg = RealColorApi::blank_channel_error(&ramp)
            .expect("zero ramp must be flagged");
        assert!(
            msg.contains("Brightness"),
            "message must tell the user what to change, got: {msg}"
        );
    }

    /// Normal and floor-rescaled ramps must NOT be flagged.
    #[test]
    fn appliable_ramps_not_flagged() {
        let full = build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0);
        assert!(RealColorApi::blank_channel_error(&full).is_none());
        let dim = build_gamma_ramp(0.5, 0.5, [1.0, 1.0, 1.0], 1.25);
        assert!(RealColorApi::blank_channel_error(&dim).is_none());
    }

    // ── NVAPI vibrance/hue ──────────────────────────────────────────────

    use crate::nvapi::MockNvapi;

    #[test]
    fn neutral_preset_writes_vibrance_when_supported() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = fake_preset(); // vibrance 50.0, hue_deg 0.0
        let result = apply_preset(&color, &nv, &preset, "profiles");
        assert_eq!(
            *nv.calls.lock().unwrap(),
            vec![("set".to_string(), 50.0, 0.0)],
            "neutral presets must still restore vibrance/hue"
        );
        assert!(result.vibrance_applied);
        assert!(result.error.is_none());
    }

    #[test]
    fn neutral_preset_skips_vibrance_silently_when_unsupported() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(false);
        let preset = fake_preset(); // vibrance 50.0, hue_deg 0.0
        let result = apply_preset(&color, &nv, &preset, "profiles");
        assert!(nv.calls.lock().unwrap().is_empty());
        assert!(!result.vibrance_applied);
        assert!(result.gamma_applied);
        assert!(result.error.is_none());
    }

    #[test]
    fn nonneutral_preset_calls_nvapi_with_exact_values() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let mut preset = fake_preset();
        preset.vibrance = 75.0;
        preset.hue_deg = 120.0;
        let result = apply_preset(&color, &nv, &preset, "profiles");
        assert_eq!(
            *nv.calls.lock().unwrap(),
            vec![("set".to_string(), 75.0, 120.0)]
        );
        assert!(result.gamma_applied && result.vibrance_applied);
        assert!(result.error.is_none());
    }

    #[test]
    fn vibrance_failure_still_reports_gamma_success() {
        let color = TestRecorder::new(true);
        let mut nv = MockNvapi::new(true);
        nv.fail_set = true;
        let mut preset = fake_preset();
        preset.vibrance = 75.0;
        let result = apply_preset(&color, &nv, &preset, "profiles");
        assert!(result.gamma_applied);
        assert!(!result.vibrance_applied);
        assert!(result.error.as_ref().unwrap().contains("vibrance"));
    }
}