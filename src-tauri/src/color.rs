// ── Per-monitor colour apply (ICC + OS gamma ramp) ──────────────────────────
// Windows-only.  Applies ICC profile first, then gamma/RGB overlay on top.
// All hardware calls abstracted behind the `ColorApi` trait for testability.

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC, HDC};
use windows::Win32::UI::ColorSystem::GetDeviceGammaRamp;

use crate::backend::BackendRegistry;
use crate::nvapi::{display_id_for_device, fns, resolve_device_name, NvColorApi};

// ── ApplyResult ─────────────────────────────────────────────────────────────

/// Result of applying a preset to a monitor.
///
/// The four fields below are the original wire contract and stay unchanged.
/// Per-adjustment detail lives in [`ApplyOutcome`], which serializes these
/// fields flat (`#[serde(flatten)]`) plus an additive `adjustments` array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyResult {
    pub icc_applied: bool,
    pub gamma_applied: bool,
    pub vibrance_applied: bool,
    pub error: Option<String>,
}

/// Status of one adjustment in an apply attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdjustmentStatus {
    /// The backend accepted the write.
    Applied,
    /// Deliberately not attempted (unsupported by the backend, or neutral so
    /// there is nothing to change).
    Skipped,
    /// Attempted and reported a failure.
    Failed,
}

/// Per-adjustment outcome, serialized inside [`ApplyOutcome::adjustments`].
///
/// `verified` is honest about read-back: apply paths report accepted writes,
/// not panel read-back (Reset is the path that verifies by read-back). A
/// backend that gains read-back verification can set it true per adjustment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdjustmentOutcome {
    pub adjustment: String,
    pub status: AdjustmentStatus,
    #[serde(default)]
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Apply response: the legacy [`ApplyResult`] shape plus additive
/// per-adjustment detail. When `adjustments` is empty it is omitted, so the
/// serialized shape is byte-for-byte the legacy contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyOutcome {
    #[serde(flatten)]
    pub result: ApplyResult,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjustments: Vec<AdjustmentOutcome>,
}

impl ApplyOutcome {
    fn legacy(result: ApplyResult) -> Self {
        ApplyOutcome {
            result,
            adjustments: Vec::new(),
        }
    }
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

    /// Read back the live OS gamma ramp (3×256 WORDs). Used to verify a
    /// reset actually landed instead of reporting blind success.
    fn read_gamma_ramp(&self, edid_id: &str) -> Result<[u16; 256 * 3], String>;
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

    fn read_gamma_ramp(&self, edid_id: &str) -> Result<[u16; 256 * 3], String> {
        let device_name = Self::device_name_for_edid(edid_id)
            .ok_or_else(|| format!("no display device found for EDID {edid_id}"))?;
        read_gamma_ramp(&device_name)
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

/// Expected identity ramp: exactly what `set_gamma_ramp` builds for
/// neutral parameters. Read-backs are compared against this byte-exact
/// (the OS LUT stores what was written; any deviation is real signal).
pub(crate) fn identity_ramp() -> [u16; 256 * 3] {
    build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0)
}

/// Best-effort GDI identity purge, shared by apply (before NVAPI writes)
/// and reset (NVAPI branch). Returns the error text on failure; callers
/// append it without failing (the driver write is authoritative).
pub(crate) fn gdi_identity_purge(api: &dyn ColorApi, edid_id: &str) -> Option<String> {
    api.set_gamma_ramp(edid_id, 1.0, 1.0, [1.0, 1.0, 1.0], 1.0)
        .err()
        .map(|e| format!("gdi-purge: {e}"))
}

/// Apply driver-level color state (gamma LUT + NVAPI vibrance/hue),
/// without ICC association. Shared by manual Apply and the enforcer.
pub fn apply_color(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    preset: &crate::store::Preset,
    target_edid: &str,
) -> ApplyResult {
    apply_color_with_backend(api, nv, preset, target_edid).0
}

/// Shared apply core that also names the backend that handled the display,
/// so per-adjustment reporting can use the same capability table the UI shows.
fn apply_color_with_backend(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    preset: &crate::store::Preset,
    target_edid: &str,
) -> (ApplyResult, &'static str) {
    if !api.is_connected(target_edid) {
        return (ApplyResult::offline(), "offline");
    }

    // Backend selection is delegated to the registry: the selected backend
    // (NVAPI, Intel, or GDI) owns the full driver-level pipeline for the display.
    let registry = BackendRegistry::new();
    let backend = registry.select(target_edid);
    (
        backend.apply(api, nv, preset, target_edid),
        backend.id(),
    )
}

// ── Per-adjustment reporting ────────────────────────────────────────────────

fn applied(adjustment: &str) -> AdjustmentOutcome {
    AdjustmentOutcome {
        adjustment: adjustment.to_string(),
        status: AdjustmentStatus::Applied,
        verified: false,
        reason: None,
    }
}

fn skipped(adjustment: &str, reason: impl Into<String>) -> AdjustmentOutcome {
    AdjustmentOutcome {
        adjustment: adjustment.to_string(),
        status: AdjustmentStatus::Skipped,
        verified: false,
        reason: Some(reason.into()),
    }
}

fn failed(adjustment: &str, reason: impl Into<String>) -> AdjustmentOutcome {
    AdjustmentOutcome {
        adjustment: adjustment.to_string(),
        status: AdjustmentStatus::Failed,
        verified: false,
        reason: Some(reason.into()),
    }
}

/// Outcome for a requested vibrance-family adjustment whose write flag is
/// false: prefer a real vibrance write error, then a static unsupported reason,
/// then a generic honest failure.
fn optional_write_outcome(
    backend_id: &str,
    adjustment: &str,
    vibrance_error: &Option<String>,
    result: &ApplyResult,
) -> AdjustmentOutcome {
    if let Some(reason) = vibrance_error {
        return failed(adjustment, reason.clone());
    }
    if let Some(reason) = capability_reason(backend_id, adjustment) {
        return skipped(adjustment, reason);
    }
    failed(
        adjustment,
        result
            .error
            .clone()
            .unwrap_or_else(|| "backend did not confirm this adjustment".to_string()),
    )
}

/// Look up the static capability reason for an adjustment on a backend.
/// Uses the same tables `get_display_capabilities` serves the editor.
fn capability_reason(backend_id: &str, adjustment: &str) -> Option<&'static str> {
    crate::capabilities::capabilities_for_backend(backend_id, true)
        .adjustments
        .into_iter()
        .find(|c| c.adjustment == adjustment)
        .and_then(|c| c.reason)
}

/// Derive the per-adjustment report for one backend apply.
///
/// Honest rules:
/// - the gamma family (gamma, brightness, contrast, rgb_gains) is one pipeline
///   write on every backend: applied together or failed together;
/// - vibrance/hue are only reported when the preset asks for a non-neutral
///   value (neutral skips are not failures) — a failure reported by the
///   vibrance write is a `Failed`, an unsupported backend is a `Skipped`;
/// - saturation is `Applied` only on Intel (the one backend with a desktop
///   saturation transform), otherwise reported `Skipped` with the reason;
/// - applied writes are `verified: false`: the apply path does not read the
///   panel back (Reset and Probe are the verified paths).
pub fn adjustment_outcomes(
    backend_id: &str,
    preset: &crate::store::Preset,
    result: &ApplyResult,
) -> Vec<AdjustmentOutcome> {
    use crate::backend::{
        ADJUSTMENT_BRIGHTNESS, ADJUSTMENT_CONTRAST, ADJUSTMENT_GAMMA, ADJUSTMENT_HUE,
        ADJUSTMENT_RGB_GAINS, ADJUSTMENT_SATURATION, ADJUSTMENT_VIBRANCE,
    };
    use crate::nvapi::{HUE_NEUTRAL, VIBRANCE_NEUTRAL};

    let mut out = Vec::new();

    // Gamma family — delivered as one ramp/pipeline write.
    let gamma_failure = if result.gamma_applied {
        None
    } else {
        Some(
            result
                .error
                .clone()
                .unwrap_or_else(|| "backend did not confirm this adjustment".to_string()),
        )
    };
    for adjustment in [
        ADJUSTMENT_GAMMA,
        ADJUSTMENT_BRIGHTNESS,
        ADJUSTMENT_CONTRAST,
        ADJUSTMENT_RGB_GAINS,
    ] {
        out.push(match &gamma_failure {
            None => applied(adjustment),
            Some(reason) => failed(adjustment, reason.clone()),
        });
    }

    // Saturation — no backend except Intel applies the desktop saturation
    // transform. Neutral saturation requests nothing, so stay silent.
    const SATURATION_NEUTRAL: f64 = 50.0;
    if preset.saturation != SATURATION_NEUTRAL {
        if backend_id == "intel" {
            out.push(match &gamma_failure {
                None => applied(ADJUSTMENT_SATURATION),
                Some(reason) => failed(ADJUSTMENT_SATURATION, reason.clone()),
            });
        } else {
            out.push(skipped(
                ADJUSTMENT_SATURATION,
                capability_reason(backend_id, ADJUSTMENT_SATURATION)
                    .unwrap_or("no desktop saturation control"),
            ));
        }
    }

    // Vibrance / hue — only when requested (non-neutral).
    let vibrance_requested = preset.vibrance != VIBRANCE_NEUTRAL;
    let hue_requested = preset.hue_deg != HUE_NEUTRAL;

    // GDI reaches vibrance through the NVAPI overlay: a failure there is a
    // real attempted failure, not an unsupported skip, so prefer the write
    // error when it names vibrance.
    let vibrance_error = result
        .error
        .clone()
        .filter(|e| e.to_lowercase().contains("vibrance"));

    if vibrance_requested {
        out.push(if result.vibrance_applied {
            applied(ADJUSTMENT_VIBRANCE)
        } else {
            optional_write_outcome(
                backend_id,
                ADJUSTMENT_VIBRANCE,
                &vibrance_error,
                result,
            )
        });
    }

    if hue_requested {
        // Intel delivers hue inside the pixtx colour matrix, written as part
        // of the same pipeline SET as the gamma family (tracked by
        // `gamma_applied`) — `vibrance_applied` is always false on Intel and
        // must not be used to judge hue. NVIDIA/GDI deliver hue through the
        // DVC vibrance write.
        if backend_id == "intel" {
            out.push(match &gamma_failure {
                None => applied(ADJUSTMENT_HUE),
                Some(reason) => failed(ADJUSTMENT_HUE, reason.clone()),
            });
        } else {
            out.push(if result.vibrance_applied {
                applied(ADJUSTMENT_HUE)
            } else {
                optional_write_outcome(backend_id, ADJUSTMENT_HUE, &vibrance_error, result)
            });
        }
    }

    out
}

// ── apply_preset (ICC + shared apply_color) ─────────────────────────────────

/// Apply a preset to a target monitor.
///
/// Precedence: ICC first, then gamma/RGB overlay on top.
/// Returns an `ApplyResult` summarising what succeeded / failed.
// The command layer uses `apply_preset_detailed` (legacy fields + additive
// per-adjustment detail); this legacy projection stays for tests and internal
// callers that only need the original four fields.
#[allow(dead_code)]
pub fn apply_preset(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    preset: &crate::store::Preset,
    target_edid: &str,
    store_profiles_dir: &str,
) -> ApplyResult {
    apply_preset_detailed(api, nv, preset, target_edid, store_profiles_dir).result
}

/// Same apply flow as [`apply_preset`], plus the additive per-adjustment
/// report. The legacy fields on `ApplyOutcome::result` behave exactly as
/// before; the command layer returns this shape to the UI.
pub fn apply_preset_detailed(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    preset: &crate::store::Preset,
    target_edid: &str,
    store_profiles_dir: &str,
) -> ApplyOutcome {
    // Step 1 — monitor connectivity check
    if !api.is_connected(target_edid) {
        return ApplyOutcome::legacy(ApplyResult::offline());
    }

    let mut icc_applied = false;
    let mut icc_error: Option<String> = None;

    // Step 2 — ICC (first)
    if !preset.icc_hash.is_empty() {
        let profile_path = RealColorApi::profile_path(store_profiles_dir, &preset.icc_filename);
        let profile_path_str = profile_path.to_string_lossy().to_string();
        match api.associate_icc(target_edid, &profile_path_str) {
            Ok(()) => icc_applied = true,
            Err(e) => {
                icc_error = Some(format!("ICC: {e}"));
            }
        }
    }

    // Step 3 — gamma + vibrance via shared core
    let (mut result, backend_id) = apply_color_with_backend(api, nv, preset, target_edid);
    result.icc_applied = icc_applied;
    let merged_error = match (icc_error.clone(), result.error.clone()) {
        (Some(a), Some(b)) => Some(format!("{a}; {b}")),
        (Some(a), None) => Some(a),
        (None, b) => b,
    };
    result.error = merged_error;

    // Step 4 — additive per-adjustment report. ICC comes first (it is the
    // first step); the rest follow the capability-table order.
    let mut adjustments = Vec::new();
    if !preset.icc_hash.is_empty() {
        adjustments.push(if icc_applied {
            applied("icc")
        } else {
            failed(
                "icc",
                icc_error
                    .clone()
                    .or_else(|| result.error.clone())
                    .unwrap_or_else(|| "ICC association failed".to_string()),
            )
        });
    }
    adjustments.extend(adjustment_outcomes(backend_id, preset, &result));

    ApplyOutcome {
        result,
        adjustments,
    }
}

/// Restore a set of displays to system defaults through the backend
/// abstraction. Used when an applied/pinned preset is removed: every display
/// that referenced it gets the same reset semantics as `reset_monitor`.
/// Returns `(edid_id, result)` pairs in input order.
pub fn restore_monitors(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    edid_ids: &[String],
) -> Vec<(String, ApplyResult)> {
    edid_ids
        .iter()
        .map(|edid| (edid.clone(), reset_monitor(api, nv, edid)))
        .collect()
}

/// Reset a monitor to system defaults: identity gamma ramp
/// (brightness 1, contrast 1, unit RGB gains, gamma 1) plus neutral
/// vibrance (50) and hue (0). ICC associations are left untouched.
/// Vibrance is attempted even where NVAPI reports unsupported (best
/// effort, those failures swallowed); supported failures are reported.
/// Every write is verified by read-back: a panel that doesn't reflect
/// the reset becomes an actionable error instead of false success.
/// Returns an `ApplyResult` with `icc_applied: false`.
pub fn reset_monitor(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    edid_id: &str,
) -> ApplyResult {
    if !api.is_connected(edid_id) {
        return ApplyResult::offline();
    }

    // Backend selection is delegated to the registry: the selected backend
    // (NVAPI or GDI) owns the full reset path (gamma + vibrance + read-back).
    BackendRegistry::new()
        .select(edid_id)
        .restore(api, nv, edid_id)
}

// ── Recorder / mock for testing ─────────────────────────────────────────────

#[cfg(test)]
pub struct TestRecorder {
    pub calls: std::sync::Mutex<Vec<String>>,
    pub last_gamma_params: std::sync::Mutex<Option<(f64, f64, [f64; 3], f64)>>,
    pub connected: bool,
    pub icc_should_fail: bool,
    pub gamma_should_fail: bool,
    /// Ramp returned by read_gamma_ramp. Defaults to identity so existing
    /// tests (which assert clean resets) keep passing; tests for the
    /// read-back check overwrite it with a deviating ramp.
    pub ramp_to_return: std::sync::Mutex<[u16; 256 * 3]>,
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
            ramp_to_return: std::sync::Mutex::new(build_gamma_ramp(
                1.0, 1.0, [1.0, 1.0, 1.0], 1.0,
            )),
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

    fn read_gamma_ramp(&self, _edid_id: &str) -> Result<[u16; 256 * 3], String> {
        Ok(*self.ramp_to_return.lock().unwrap())
    }
}

// ── Tauri command ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn apply_preset_cmd(
    state: tauri::State<'_, crate::store::AppStore>,
    id: String,
    target_edid: String,
) -> ApplyOutcome {
    // Read the preset from the store
    let store = match state.0.lock() {
        Ok(s) => s,
        Err(e) => {
            return ApplyOutcome::legacy(ApplyResult {
                icc_applied: false,
                gamma_applied: false,
                vibrance_applied: false,
                error: Some(format!("store lock: {e}")),
            })
        }
    };

    let preset = match store.list_presets().into_iter().find(|p| p.id == id) {
        Some(p) => p,
        None => {
            return ApplyOutcome::legacy(ApplyResult {
                icc_applied: false,
                gamma_applied: false,
                vibrance_applied: false,
                error: Some(format!("preset not found: {id}")),
            })
        }
    };

    // Build the profiles directory path from the store's data_dir
    let profiles_dir = crate::store::default_store_path().join("profiles");
    let profiles_dir_str = profiles_dir.to_string_lossy().to_string();

    // Release the store lock before the potentially long apply call
    drop(store);

    let api = RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let outcome = apply_preset_detailed(&api, &nv, &preset, &target_edid, &profiles_dir_str);

    // Record applied state on success (reacquire store lock)
    if outcome.result.error.is_none() {
        if let Ok(mut store) = state.0.lock() {
            let _ = store.record_applied(&target_edid, &id);
        }
    }

    outcome
}

/// Live hardware readout for one monitor — answers "did the last write
/// actually land?" without changing anything.
#[derive(Debug, Clone, Serialize)]
pub struct MonitorProbe {
    /// Legacy coarse engine classification ("nvapi" | "gdi").
    pub engine: String,
    /// Real backend that owns this display ("nvapi" | "intel" | "gdi"),
    /// additive so older consumers can keep reading `engine`.
    pub backend_id: String,
    pub vibrance_supported: bool,
    pub vibrance: Option<f64>,
    pub hue: Option<f64>,
    /// Peak of the live colour ramp/LUT (0..1). For Intel this is the pixtx
    /// 1D LUT, not the unrelated OS GDI ramp.
    pub ramp_peak: Option<f64>,
    /// Whether that ramp/LUT is the linear identity. For Intel this is the
    /// pixtx 1D LUT.
    pub ramp_identity: Option<bool>,
}

#[tauri::command]
pub fn probe_monitor_cmd(edid_id: String) -> Result<MonitorProbe, String> {
    let api = RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let registry = BackendRegistry::new();
    let backend = registry.select(&edid_id);
    let backend_id = backend.id();
    let engine = match gamma_engine(&edid_id) {
        GammaEngine::Nvapi => "nvapi",
        GammaEngine::Gdi => "gdi",
    };
    // Vibrance/hue are NVAPI controls; Intel reports unsupported (false) and
    // GDI probes NVAPI best-effort exactly as before.
    let supported = backend_id != "intel" && nv.supported(&edid_id);
    let (vibrance, hue) = if supported {
        match nv.levels(&edid_id) {
            Ok((v, h)) => (Some(v), Some(h)),
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };
    // Intel colour is driven by the pixtx 1D LUT, so reading the OS GDI ramp
    // would report "identity" while the panel is tinted. Probe the live pixtx
    // LUT instead.
    let (ramp_peak, ramp_identity) = if backend_id == "intel" {
        match crate::backend::intel::current_1dlut_summary(&edid_id) {
            Ok((peak, identity)) => (Some(peak), Some(identity)),
            Err(_) => (None, None),
        }
    } else {
        match api.read_gamma_ramp(&edid_id) {
            Ok(ramp) => {
                let peak = ramp.iter().max().copied().unwrap_or(0) as f64 / 65535.0;
                (Some(peak), Some(ramp == identity_ramp()))
            }
            Err(_) => (None, None),
        }
    };
    Ok(MonitorProbe {
        engine: engine.into(),
        backend_id: backend_id.into(),
        vibrance_supported: supported,
        vibrance,
        hue,
        ramp_peak,
        ramp_identity,
    })
}

#[tauri::command]
pub fn reset_monitor_cmd(
    state: tauri::State<'_, crate::store::AppStore>,
    edid_id: String,
) -> ApplyResult {
    let api = RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let result = reset_monitor(&api, &nv, &edid_id);

    // Clear applied record on success
    if result.error.is_none() {
        if let Ok(mut store) = state.0.lock() {
            store.clear_applied(&edid_id);
        }
    }

    result
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
            icc_hash: "abc123".into(),
            icc_filename: "abc123.icc".into(),
            brightness: 0.5,
            contrast: 0.8,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 2.2,
            vibrance: 50.0,
            saturation: 50.0,
            hue_deg: 0.0,
            color_model: "nvcp-v1".into(),
            color_tag: None,
            tag_shape: None,
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

        let _result = apply_preset(&recorder, &nv, &preset, "EDID-001", "profiles");

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

        let result = apply_preset(&recorder, &nv, &preset, "EDID-001", "profiles");
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
        let result = apply_preset(&recorder, &nv, &preset, "EDID-001", "profiles");

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

        let _result = apply_preset(&recorder, &nv, &preset, "EDID-001", "profiles");

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

        let result = apply_preset(&recorder, &nv, &preset, "EDID-001", "profiles");

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

    /// Reset attempts a best-effort neutral vibrance write even where
    /// NVAPI reports unsupported (e.g. iGPU-driven panels): the attempt
    /// is recorded, failures are swallowed, gamma still applies, no error.
    /// (A write-works-but-probe-fails display must not get stuck.)
    #[test]
    fn reset_attempts_neutral_vibrance_best_effort_when_unsupported() {
        let recorder = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        let result = reset_monitor(&recorder, &nv, "EDID-001");

        assert!(result.gamma_applied);
        assert!(!result.vibrance_applied);
        assert_eq!(
            *nv.calls.lock().unwrap(),
            vec![("set".to_string(), VIBRANCE_NEUTRAL, HUE_NEUTRAL)],
            "reset must attempt the neutral write even when unsupported"
        );
        assert!(result.error.is_none());
    }

    /// Best-effort failure on an unsupported display stays silent: the
    /// panel was never addressable, so there is nothing stuck to report.
    #[test]
    fn reset_swallows_best_effort_vibrance_failure_when_unsupported() {
        let recorder = TestRecorder::new(true);
        let mut nv = MockNvapi::new(false);
        nv.fail_set = true;

        let result = reset_monitor(&recorder, &nv, "EDID-001");

        assert!(result.gamma_applied);
        assert!(!result.vibrance_applied);
        assert!(result.error.is_none());
    }

    /// A panel that still reports non-neutral vibrance after a successful
    /// neutral write fails verification instead of false success.
    #[test]
    fn reset_flags_vibrance_readback_mismatch() {
        let recorder = TestRecorder::new(true);
        let mut nv = MockNvapi::new(true);
        nv.levels = (100.0, 0.0);

        let result = reset_monitor(&recorder, &nv, "EDID-001");

        assert!(result.gamma_applied);
        assert!(result.vibrance_applied);
        assert!(result.error.as_ref().unwrap().contains("still reports"));
    }

    /// A GDI ramp that doesn't read back as identity fails verification
    /// (write accepted, panel unchanged) instead of false success.
    #[test]
    fn reset_flags_gamma_ramp_readback_mismatch() {
        let recorder = TestRecorder::new(true);
        *recorder.ramp_to_return.lock().unwrap() =
            build_gamma_ramp(0.5, 0.5, [1.0, 1.0, 1.0], 1.25);
        let nv = MockNvapi::new(false);

        let result = reset_monitor(&recorder, &nv, "EDID-001");

        assert!(result.gamma_applied, "write itself succeeded");
        assert!(result.error.as_ref().unwrap().contains("read-back"));
    }

    /// The shared GDI identity purge writes neutral parameters.
    #[test]
    fn gdi_identity_purge_writes_identity() {
        let recorder = TestRecorder::new(true);

        let err = gdi_identity_purge(&recorder, "EDID-001");

        assert!(err.is_none());
        assert_eq!(*recorder.calls.lock().unwrap(), vec!["gamma"]);
        assert_eq!(
            *recorder.last_gamma_params.lock().unwrap(),
            Some((1.0, 1.0, [1.0, 1.0, 1.0], 1.0))
        );
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
        let result = apply_preset(&color, &nv, &preset, "EDID-001", "profiles");
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
        let result = apply_preset(&color, &nv, &preset, "EDID-001", "profiles");
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
        let result = apply_preset(&color, &nv, &preset, "EDID-001", "profiles");
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
        let result = apply_preset(&color, &nv, &preset, "EDID-001", "profiles");
        assert!(result.gamma_applied);
        assert!(!result.vibrance_applied);
        assert!(result.error.as_ref().unwrap().contains("vibrance"));
    }

    // ── Additive ApplyOutcome wire contract (Phase F) ───────────────────

    fn legacy_result() -> ApplyResult {
        ApplyResult {
            icc_applied: false,
            gamma_applied: true,
            vibrance_applied: false,
            error: None,
        }
    }

    #[test]
    fn legacy_apply_outcome_serializes_without_adjustments_key() {
        let outcome = ApplyOutcome::legacy(legacy_result());
        let json = serde_json::to_value(&outcome).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "icc_applied": false,
                "gamma_applied": true,
                "vibrance_applied": false,
                "error": null
            }),
            "empty per-adjustment detail must not change the legacy wire shape"
        );
    }

    #[test]
    fn legacy_apply_result_still_serializes_and_deserializes_unchanged() {
        let result = legacy_result();
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "icc_applied": false,
                "gamma_applied": true,
                "vibrance_applied": false,
                "error": null
            })
        );
        let back: ApplyResult = serde_json::from_value(json).unwrap();
        assert_eq!(back, result);
    }

    #[test]
    fn legacy_apply_json_deserializes_with_empty_adjustments() {
        let legacy = r#"{"icc_applied":true,"gamma_applied":false,"vibrance_applied":false,"error":"gamma: nope"}"#;
        let outcome: ApplyOutcome = serde_json::from_str(legacy).unwrap();
        assert!(outcome.adjustments.is_empty());
        assert_eq!(outcome.result.icc_applied, true);
        assert_eq!(outcome.result.gamma_applied, false);
        assert_eq!(outcome.result.error.as_deref(), Some("gamma: nope"));
    }

    #[test]
    fn adjustment_details_serialize_additively() {
        let outcome = ApplyOutcome {
            result: legacy_result(),
            adjustments: vec![
                applied("gamma"),
                skipped("saturation", "no desktop saturation control"),
            ],
        };
        let json = serde_json::to_value(&outcome).unwrap();
        // Legacy fields remain at the top level.
        assert_eq!(json["gamma_applied"], true);
        assert_eq!(json["error"], serde_json::Value::Null);
        // Additive detail is a self-describing array with lowercase status.
        assert_eq!(json["adjustments"][0]["adjustment"], "gamma");
        assert_eq!(json["adjustments"][0]["status"], "applied");
        assert_eq!(json["adjustments"][0]["verified"], false);
        assert_eq!(json["adjustments"][1]["status"], "skipped");
        assert_eq!(
            json["adjustments"][1]["reason"],
            "no desktop saturation control"
        );

        // And it round-trips.
        let back: ApplyOutcome = serde_json::from_value(json).unwrap();
        assert_eq!(back, outcome);
    }

    // ── Per-adjustment derivation ───────────────────────────────────────

    fn nvcp_preset() -> Preset {
        Preset {
            id: "p1".into(),
            name: "NVCP".into(),
            icc_hash: String::new(),
            icc_filename: String::new(),
            brightness: 55.0,
            contrast: 60.0,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 1.0,
            vibrance: 50.0,
            saturation: 50.0,
            hue_deg: 0.0,
            color_model: "nvcp-v1".into(),
            color_tag: None,
            tag_shape: None,
        }
    }

    #[test]
    fn adjustment_outcomes_omit_neutral_optional_adjustments() {
        let outs = adjustment_outcomes("gdi", &nvcp_preset(), &legacy_result());
        let names: Vec<&str> = outs.iter().map(|o| o.adjustment.as_str()).collect();
        assert_eq!(names, vec!["gamma", "brightness", "contrast", "rgb_gains"]);
        assert!(outs.iter().all(|o| o.status == AdjustmentStatus::Applied));
        assert!(outs.iter().all(|o| !o.verified), "apply is not read-back verified");
    }

    #[test]
    fn adjustment_outcomes_report_unsupported_saturation_as_skipped() {
        let mut preset = nvcp_preset();
        preset.saturation = 70.0;
        let outs = adjustment_outcomes("nvapi", &preset, &legacy_result());
        let sat = outs.iter().find(|o| o.adjustment == "saturation").unwrap();
        assert_eq!(sat.status, AdjustmentStatus::Skipped);
        assert!(
            sat.reason.as_deref().unwrap_or("").contains("saturation"),
            "reason must name the missing control: {:?}",
            sat.reason
        );
    }

    #[test]
    fn adjustment_outcomes_report_gamma_failure_for_the_whole_family() {
        let result = ApplyResult {
            icc_applied: false,
            gamma_applied: false,
            vibrance_applied: false,
            error: Some("gamma: driver rejected".into()),
        };
        let outs = adjustment_outcomes("nvapi", &nvcp_preset(), &result);
        for name in ["gamma", "brightness", "contrast", "rgb_gains"] {
            let out = outs.iter().find(|o| o.adjustment == name).unwrap();
            assert_eq!(out.status, AdjustmentStatus::Failed, "{name}");
            assert_eq!(out.reason.as_deref(), Some("gamma: driver rejected"));
        }
    }

    #[test]
    fn adjustment_outcomes_report_vibrance_failure_honestly() {
        let mut preset = nvcp_preset();
        preset.vibrance = 75.0;
        let result = ApplyResult {
            icc_applied: false,
            gamma_applied: true,
            vibrance_applied: false,
            error: Some("vibrance: DVC write failed".into()),
        };
        let outs = adjustment_outcomes("nvapi", &preset, &result);
        let vib = outs.iter().find(|o| o.adjustment == "vibrance").unwrap();
        assert_eq!(vib.status, AdjustmentStatus::Failed);
        assert_eq!(vib.reason.as_deref(), Some("vibrance: DVC write failed"));
    }

    #[test]
    fn adjustment_outcomes_intel_reports_saturation_applied_and_vibrance_skipped() {
        let mut preset = nvcp_preset();
        preset.saturation = 70.0;
        preset.vibrance = 75.0;
        let outs = adjustment_outcomes("intel", &preset, &legacy_result());
        let sat = outs.iter().find(|o| o.adjustment == "saturation").unwrap();
        assert_eq!(sat.status, AdjustmentStatus::Applied);
        let vib = outs.iter().find(|o| o.adjustment == "vibrance").unwrap();
        assert_eq!(vib.status, AdjustmentStatus::Skipped);
        assert!(vib.reason.as_deref().unwrap_or("").to_lowercase().contains("vibrance"));
    }

    #[test]
    fn adjustment_outcomes_intel_reports_hue_applied_without_vibrance_flag() {
        // Intel delivers hue in the pixtx matrix, not through the NVAPI DVC
        // write. Its apply always returns vibrance_applied=false, so hue must
        // be judged by the pipeline write (gamma_applied), not that flag.
        let mut preset = nvcp_preset();
        preset.hue_deg = 30.0;
        let outs = adjustment_outcomes("intel", &preset, &legacy_result());
        let hue = outs.iter().find(|o| o.adjustment == "hue").unwrap();
        assert_eq!(hue.status, AdjustmentStatus::Applied);
        assert!(hue.reason.is_none());
    }

    #[test]
    fn adjustment_outcomes_intel_reports_hue_failed_with_pipeline_failure() {
        let mut preset = nvcp_preset();
        preset.hue_deg = 30.0;
        let result = ApplyResult {
            icc_applied: false,
            gamma_applied: false,
            vibrance_applied: false,
            error: Some("intel: pixtx SET failed".into()),
        };
        let outs = adjustment_outcomes("intel", &preset, &result);
        let hue = outs.iter().find(|o| o.adjustment == "hue").unwrap();
        assert_eq!(hue.status, AdjustmentStatus::Failed);
        assert_eq!(hue.reason.as_deref(), Some("intel: pixtx SET failed"));
    }

    #[test]
    fn apply_preset_detailed_reports_icc_first_and_keeps_legacy_fields() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = fake_preset(); // has ICC
        let outcome =
            apply_preset_detailed(&color, &nv, &preset, "EDID-001", "profiles");

        assert!(outcome.result.icc_applied);
        assert!(outcome.result.gamma_applied);
        assert!(outcome.result.error.is_none());
        assert_eq!(outcome.adjustments[0].adjustment, "icc");
        assert_eq!(outcome.adjustments[0].status, AdjustmentStatus::Applied);
        assert!(outcome
            .adjustments
            .iter()
            .any(|o| o.adjustment == "gamma" && o.status == AdjustmentStatus::Applied));
    }

    #[test]
    fn removal_restore_uses_the_backend_reset_path() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let results = restore_monitors(
            &color,
            &nv,
            &["EDID-001".to_string(), "EDID-002".to_string()],
        );
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, r)| r.gamma_applied));
        // GDI reset path writes the identity ramp for each display.
        assert_eq!(
            *color.last_gamma_params.lock().unwrap(),
            Some((1.0, 1.0, [1.0, 1.0, 1.0], 1.0))
        );
        assert_eq!(color.calls.lock().unwrap().iter().filter(|c| *c == "gamma").count(), 2);
    }
}

#[cfg(test)]
mod nvcp_tests {
    // Placeholder for future NVCP-specific tests
}