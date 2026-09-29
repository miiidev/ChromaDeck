// ── Per-monitor colour apply (ICC + OS gamma ramp) ──────────────────────────
// Windows-only.  Applies ICC profile first, then gamma/RGB overlay on top.
// All hardware calls abstracted behind the `ColorApi` trait for testability.

use serde::Serialize;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC, HDC};

// ── ApplyResult ─────────────────────────────────────────────────────────────

/// Result of applying a preset to a monitor.
#[derive(Debug, Clone, Serialize)]
pub struct ApplyResult {
    pub icc_applied: bool,
    pub gamma_applied: bool,
    pub error: Option<String>,
}

#[allow(dead_code)]
impl ApplyResult {
    pub fn offline() -> Self {
        ApplyResult {
            icc_applied: false,
            gamma_applied: false,
            error: Some("monitor is offline".into()),
        }
    }

    pub fn success() -> Self {
        ApplyResult {
            icc_applied: true,
            gamma_applied: true,
            error: None,
        }
    }
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
        let wide_device: Vec<u16> = OsString::from(&device_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: CreateDCW for a display device returns a handle to the monitor's DC.
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

        // Build the gamma ramp.
        let ramp = build_gamma_ramp(brightness, contrast, rgb_gains, gamma);

        // SAFETY: SetDeviceGammaRamp writes the ramp into the display driver.
        let ok: bool = unsafe { SetDeviceGammaRamp(hdc, &ramp as *const _ as _).as_bool() };

        // Cleanup DC.
        // SAFETY: DeleteDC is safe after a successful CreateDCW.
        unsafe { let _ = DeleteDC(hdc); };

        if !ok {
            Err(format!("SetDeviceGammaRamp failed for {device_name}"))
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
fn build_gamma_ramp(
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

    ramp
}

// ── apply_preset (core logic) ───────────────────────────────────────────────

/// Apply a preset to its target monitor.
///
/// Precedence: ICC first, then gamma/RGB overlay on top.
/// Returns an `ApplyResult` summarising what succeeded / failed.
pub fn apply_preset(
    api: &dyn ColorApi,
    preset: &crate::store::Preset,
    store_profiles_dir: &str,
) -> ApplyResult {
    // Step 1 — monitor connectivity check
    if !api.is_connected(&preset.edid_id) {
        return ApplyResult::offline();
    }

    let mut icc_applied = false;
    let mut gamma_applied = false;
    let mut error: Option<String> = None;

    // Step 2 — ICC (first)
    if !preset.icc_hash.is_empty() {
        let profile_path = RealColorApi::profile_path(store_profiles_dir, &preset.icc_filename);
        let profile_path_str = profile_path.to_string_lossy().to_string();
        match api.associate_icc(&preset.edid_id, &profile_path_str) {
            Ok(()) => icc_applied = true,
            Err(e) => {
                error = Some(format!("ICC: {e}"));
            }
        }
    }

    // Step 3 — Gamma / RGB overlay (second, on top)
    match api.set_gamma_ramp(
        &preset.edid_id,
        preset.brightness,
        preset.contrast,
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

    ApplyResult {
        icc_applied,
        gamma_applied,
        error,
    }
}

// ── Recorder / mock for testing ─────────────────────────────────────────────

#[cfg(test)]
pub struct TestRecorder {
    pub calls: std::sync::Mutex<Vec<String>>,
    pub connected: bool,
    pub icc_should_fail: bool,
    pub gamma_should_fail: bool,
}

#[cfg(test)]
impl TestRecorder {
    pub fn new(connected: bool) -> Self {
        TestRecorder {
            calls: std::sync::Mutex::new(Vec::new()),
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
        _brightness: f64,
        _contrast: f64,
        _rgb_gains: [f64; 3],
        _gamma: f64,
    ) -> Result<(), String> {
        let mut calls = self.calls.lock().unwrap();
        calls.push("gamma".into());
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
    apply_preset(&api, &preset, &profiles_dir_str)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Preset;

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
        let preset = fake_preset();

        let _result = apply_preset(&recorder, &preset, "profiles");

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
        let preset = fake_preset();

        let result = apply_preset(&recorder, &preset, "profiles");
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
        let mut preset = fake_preset();
        preset.icc_hash = "nonexistent".into();
        preset.icc_filename = "nonexistent.icc".into();

        // With a recorder that always succeeds, both should be applied.
        let result = apply_preset(&recorder, &preset, "profiles");

        assert!(result.icc_applied);
        assert!(result.gamma_applied);
        assert!(result.error.is_none());
    }

    // ── No ICC → only gamma applied ──────────────────────────────────────

    #[test]
    fn no_icc_skips_icc_and_applies_gamma_only() {
        let recorder = TestRecorder::new(true);
        let preset = preset_no_icc();

        let _result = apply_preset(&recorder, &preset, "profiles");

        let calls = recorder.calls.lock().unwrap();
        assert_eq!(*calls, vec!["gamma"], "without ICC, only gamma is applied");
    }

    // ── ICC failure still applies gamma ──────────────────────────────────

    #[test]
    fn icc_failure_still_applies_gamma() {
        let mut recorder = TestRecorder::new(true);
        recorder.icc_should_fail = true;
        let preset = fake_preset();

        let result = apply_preset(&recorder, &preset, "profiles");

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
}