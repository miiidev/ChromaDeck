// ── Opt-in DDC/CI hardware backlight brightness (VCP 0x10 via Dxva2) ───────
// Windows-only. Controls physical monitor OSD backlight brightness only when a
// preset explicitly sets `hardware_brightness: Some(0..=100)`.
//
// Invariants:
//   - Physical monitor handles are ALWAYS freed via `DestroyPhysicalMonitors`
//     inside `PhysicalMonitorsGuard::drop` (zero handle leaks).
//   - Negative DDC/CI verdicts (e.g. internal laptop eDP panels or monitors
//     with DDC/CI disabled in OSD) are cached per `edid_id` for 5 minutes so
//     40–100 ms I2C timeouts never stall repeated probes or applies.
//   - Before the first write to a monitor in a session, its pre-apply VCP 0x10
//     value is captured so Reset / Auto-apply Revert / Tray Quit restore it.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, GetNumberOfPhysicalMonitorsFromHMONITOR,
    GetPhysicalMonitorsFromHMONITOR, GetVCPFeatureAndVCPFeatureReply, SetVCPFeature,
    PHYSICAL_MONITOR,
};
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};

/// VESA MCCS VCP code for Luminance (Backlight Brightness).
pub const VCP_LUMINANCE: u8 = 0x10;

const NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(300);

/// Pure normalization from raw VCP `(current, max)` to `0..=100` percent.
pub fn vcp_to_percent(current: u32, max: u32) -> u8 {
    if max == 0 {
        return 0;
    }
    let ratio = (current.min(max) as f64 / max as f64) * 100.0;
    ratio.round().clamp(0.0, 100.0) as u8
}

/// Pure denormalization from `0..=100` percent to raw VCP `0..=max`.
pub fn percent_to_vcp(percent: u8, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    let clamped = percent.min(100) as f64;
    ((clamped / 100.0) * max as f64).round().clamp(0.0, max as f64) as u32
}

/// Live probe result returned to `PresetEditor.tsx`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DdcciProbe {
    pub supported: bool,
    pub current_percent: Option<u8>,
    pub reason: Option<String>,
}

/// Hardware seam so DDC/CI logic, negative caching, and baseline capture/restore
/// are 100% unit-tested without an external DDC/CI monitor attached.
pub trait DdcciApi: Send + Sync {
    /// Read current luminance as `(current_raw, max_raw)`.
    fn read_vcp_luminance(&self, edid_id: &str) -> Result<(u32, u32), String>;
    /// Write raw luminance value (`0..=max_raw`).
    fn write_vcp_luminance(&self, edid_id: &str, raw_value: u32) -> Result<(), String>;
}

/// RAII guard guaranteeing `DestroyPhysicalMonitors` always runs.
struct PhysicalMonitorsGuard {
    monitors: Vec<PHYSICAL_MONITOR>,
}

impl Drop for PhysicalMonitorsGuard {
    fn drop(&mut self) {
        if !self.monitors.is_empty() {
            // SAFETY: `self.monitors` was populated by `GetPhysicalMonitorsFromHMONITOR`.
            let _ = unsafe { DestroyPhysicalMonitors(&self.monitors) };
        }
    }
}

fn find_hmonitor_for_device(target_device: &str) -> Option<HMONITOR> {
    struct SearchCtx {
        target: String,
        found: Option<HMONITOR>,
    }

    unsafe extern "system" fn enum_proc(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> windows::core::BOOL {
        let ctx = &mut *(lparam.0 as *mut SearchCtx);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut info.monitorInfo).as_bool() {
            let len = info
                .szDevice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(info.szDevice.len());
            let name = String::from_utf16_lossy(&info.szDevice[..len]);
            if name.eq_ignore_ascii_case(&ctx.target) {
                ctx.found = Some(hmon);
                return windows::core::BOOL(0); // stop enumeration
            }
        }
        windows::core::BOOL(1)
    }

    let mut ctx = SearchCtx {
        target: target_device.to_string(),
        found: None,
    };
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut ctx as *mut SearchCtx as isize),
        );
    }
    ctx.found
}

fn physical_monitors_for_edid(edid_id: &str) -> Result<PhysicalMonitorsGuard, String> {
    let device = crate::nvapi::resolve_device_name(edid_id)
        .ok_or_else(|| format!("monitor not connected: {edid_id}"))?;
    let hmon = find_hmonitor_for_device(&device)
        .ok_or_else(|| format!("no HMONITOR found for {device}"))?;

    let mut count: u32 = 0;
    unsafe {
        GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count)
            .map_err(|e| format!("GetNumberOfPhysicalMonitorsFromHMONITOR failed: {e}"))?;
    }
    if count == 0 {
        return Err("Internal/non-DDC panel (0 physical monitors)".into());
    }
    let mut monitors = vec![PHYSICAL_MONITOR::default(); count as usize];
    unsafe {
        GetPhysicalMonitorsFromHMONITOR(hmon, &mut monitors)
            .map_err(|e| format!("GetPhysicalMonitorsFromHMONITOR failed: {e}"))?;
    }
    Ok(PhysicalMonitorsGuard { monitors })
}

/// Real Win32 `Dxva2` DDC/CI implementation.
pub struct RealDdcci;

impl DdcciApi for RealDdcci {
    fn read_vcp_luminance(&self, edid_id: &str) -> Result<(u32, u32), String> {
        let guard = physical_monitors_for_edid(edid_id)?;
        let handle = guard.monitors[0].hPhysicalMonitor;
        let mut current: u32 = 0;
        let mut max: u32 = 0;
        // SAFETY: `handle` is valid for the lifetime of `guard`.
        let ok = unsafe {
            GetVCPFeatureAndVCPFeatureReply(handle, VCP_LUMINANCE, None, &mut current, Some(&mut max))
        };
        if ok == 0 {
            return Err("DDC/CI unsupported (internal laptop panel or DDC/CI disabled in OSD)".into());
        }
        if max == 0 {
            return Err("DDC/CI reported max luminance 0".into());
        }
        Ok((current, max))
    }

    fn write_vcp_luminance(&self, edid_id: &str, raw_value: u32) -> Result<(), String> {
        let guard = physical_monitors_for_edid(edid_id)?;
        let handle = guard.monitors[0].hPhysicalMonitor;
        // SAFETY: `handle` is valid for the lifetime of `guard`.
        let ok = unsafe { SetVCPFeature(handle, VCP_LUMINANCE, raw_value) };
        if ok == 0 {
            return Err("DDC/CI SetVCPFeature(0x10) failed".into());
        }
        Ok(())
    }
}

// ── Session state: negative cache + pre-apply baseline capture ──────────────

#[derive(Default)]
pub struct DdcciSession {
    negative_cache: HashMap<String, (Instant, String)>,
    /// Captured pre-apply `(raw_value, max_value)` per `edid_id`.
    baselines: HashMap<String, (u32, u32)>,
    /// Cached `max_value` per `edid_id` once a monitor responds cleanly.
    max_values: HashMap<String, u32>,
}

impl DdcciSession {
    pub fn invalidate_cache(&mut self) {
        self.negative_cache.clear();
        self.max_values.clear();
    }

    fn cached_negative(&self, edid_id: &str, ttl: Duration) -> Option<String> {
        self.negative_cache.get(edid_id).and_then(|(since, reason)| {
            if since.elapsed() < ttl {
                Some(reason.clone())
            } else {
                None
            }
        })
    }

    pub fn probe(&mut self, api: &dyn DdcciApi, edid_id: &str) -> DdcciProbe {
        if let Some(reason) = self.cached_negative(edid_id, NEGATIVE_CACHE_TTL) {
            return DdcciProbe {
                supported: false,
                current_percent: None,
                reason: Some(reason),
            };
        }
        match api.read_vcp_luminance(edid_id) {
            Ok((cur, max)) => {
                self.negative_cache.remove(edid_id);
                self.max_values.insert(edid_id.to_string(), max);
                DdcciProbe {
                    supported: true,
                    current_percent: Some(vcp_to_percent(cur, max)),
                    reason: None,
                }
            }
            Err(reason) => {
                self.negative_cache
                    .insert(edid_id.to_string(), (Instant::now(), reason.clone()));
                DdcciProbe {
                    supported: false,
                    current_percent: None,
                    reason: Some(reason),
                }
            }
        }
    }

    /// Apply opt-in hardware brightness (`0..=100`). Captures pre-apply VCP
    /// luminance on first touch so restore returns the display to its prior OSD
    /// backlight level.
    pub fn apply_brightness(
        &mut self,
        api: &dyn DdcciApi,
        edid_id: &str,
        percent: u8,
    ) -> Result<(), String> {
        if let Some(reason) = self.cached_negative(edid_id, NEGATIVE_CACHE_TTL) {
            return Err(reason);
        }

        // Capture baseline once before our first write to this monitor.
        let max = if let Some(&cached_max) = self.max_values.get(edid_id) {
            if !self.baselines.contains_key(edid_id) {
                if let Ok((cur, m)) = api.read_vcp_luminance(edid_id) {
                    self.baselines.insert(edid_id.to_string(), (cur, m));
                    self.max_values.insert(edid_id.to_string(), m);
                }
            }
            cached_max
        } else {
            match api.read_vcp_luminance(edid_id) {
                Ok((cur, m)) => {
                    self.baselines
                        .entry(edid_id.to_string())
                        .or_insert((cur, m));
                    self.max_values.insert(edid_id.to_string(), m);
                    m
                }
                Err(reason) => {
                    self.negative_cache
                        .insert(edid_id.to_string(), (Instant::now(), reason.clone()));
                    return Err(reason);
                }
            }
        };

        let raw = percent_to_vcp(percent, max);
        api.write_vcp_luminance(edid_id, raw)
    }

    /// Restore pre-apply hardware brightness if we ever modified this monitor.
    /// Returns `Ok(true)` if restored, `Ok(false)` if this monitor was never touched.
    pub fn restore_brightness(
        &mut self,
        api: &dyn DdcciApi,
        edid_id: &str,
    ) -> Result<bool, String> {
        let Some((orig_raw, _max)) = self.baselines.remove(edid_id) else {
            return Ok(false);
        };
        api.write_vcp_luminance(edid_id, orig_raw)?;
        Ok(true)
    }

    /// Restore every monitor whose hardware brightness was modified in this session.
    pub fn restore_all(&mut self, api: &dyn DdcciApi) {
        let keys: Vec<String> = self.baselines.keys().cloned().collect();
        for edid in keys {
            let _ = self.restore_brightness(api, &edid);
        }
    }
}

static GLOBAL_SESSION: OnceLock<Mutex<DdcciSession>> = OnceLock::new();

fn global_session() -> &'static Mutex<DdcciSession> {
    GLOBAL_SESSION.get_or_init(|| Mutex::new(DdcciSession::default()))
}

/// Clear cached negative DDC/CI verdicts (called on `WM_DISPLAYCHANGE`).
pub fn invalidate_cache() {
    if let Ok(mut s) = global_session().lock() {
        s.invalidate_cache();
    }
}

pub fn apply_hardware_brightness(edid_id: &str, percent: u8) -> Result<(), String> {
    let mut s = global_session()
        .lock()
        .map_err(|e| format!("ddcci session lock: {e}"))?;
    s.apply_brightness(&RealDdcci, edid_id, percent)
}

pub fn restore_hardware_brightness(edid_id: &str) -> Result<bool, String> {
    let mut s = global_session()
        .lock()
        .map_err(|e| format!("ddcci session lock: {e}"))?;
    s.restore_brightness(&RealDdcci, edid_id)
}

pub fn restore_all_hardware_brightness() {
    if let Ok(mut s) = global_session().lock() {
        s.restore_all(&RealDdcci);
    }
}

#[tauri::command]
pub fn probe_ddcci_cmd(edid_id: String) -> DdcciProbe {
    match global_session().lock() {
        Ok(mut s) => s.probe(&RealDdcci, &edid_id),
        Err(e) => DdcciProbe {
            supported: false,
            current_percent: None,
            reason: Some(format!("ddcci lock error: {e}")),
        },
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockDdcci {
        supported: bool,
        max: u32,
        current: Mutex<u32>,
        read_calls: AtomicUsize,
        write_calls: AtomicUsize,
    }

    impl MockDdcci {
        fn new(supported: bool, initial: u32, max: u32) -> Self {
            Self {
                supported,
                max,
                current: Mutex::new(initial),
                read_calls: AtomicUsize::new(0),
                write_calls: AtomicUsize::new(0),
            }
        }
    }

    impl DdcciApi for MockDdcci {
        fn read_vcp_luminance(&self, _edid_id: &str) -> Result<(u32, u32), String> {
            self.read_calls.fetch_add(1, Ordering::SeqCst);
            if !self.supported {
                return Err("Internal/non-DDC panel".into());
            }
            Ok((*self.current.lock().unwrap(), self.max))
        }

        fn write_vcp_luminance(&self, _edid_id: &str, raw_value: u32) -> Result<(), String> {
            self.write_calls.fetch_add(1, Ordering::SeqCst);
            if !self.supported {
                return Err("Internal/non-DDC panel".into());
            }
            *self.current.lock().unwrap() = raw_value;
            Ok(())
        }
    }

    #[test]
    fn vcp_normalization_round_trips_for_non_100_max() {
        assert_eq!(vcp_to_percent(0, 255), 0);
        assert_eq!(vcp_to_percent(255, 255), 100);
        assert_eq!(vcp_to_percent(128, 255), 50);
        assert_eq!(percent_to_vcp(50, 255), 128);
        assert_eq!(percent_to_vcp(100, 100), 100);
        assert_eq!(percent_to_vcp(120, 100), 100); // clamped
    }

    #[test]
    fn apply_captures_baseline_once_and_restore_returns_original() {
        let mock = MockDdcci::new(true, 64, 100);
        let mut session = DdcciSession::default();

        session.apply_brightness(&mock, "MON1", 25).unwrap();
        assert_eq!(*mock.current.lock().unwrap(), 25);

        // Second apply must keep original 64 baseline, not overwrite with 25.
        session.apply_brightness(&mock, "MON1", 90).unwrap();
        assert_eq!(*mock.current.lock().unwrap(), 90);

        let restored = session.restore_brightness(&mock, "MON1").unwrap();
        assert!(restored);
        assert_eq!(*mock.current.lock().unwrap(), 64);

        // Second restore is a clean no-op.
        assert!(!session.restore_brightness(&mock, "MON1").unwrap());
    }

    #[test]
    fn negative_cache_avoids_repeated_i2c_timeouts_until_invalidated() {
        let mock = MockDdcci::new(false, 0, 100);
        let mut session = DdcciSession::default();

        let p1 = session.probe(&mock, "EDP1");
        assert!(!p1.supported);
        assert_eq!(mock.read_calls.load(Ordering::SeqCst), 1);

        // Second probe hits negative cache without touching hardware.
        let p2 = session.probe(&mock, "EDP1");
        assert!(!p2.supported);
        assert_eq!(mock.read_calls.load(Ordering::SeqCst), 1);

        // Invalidation (e.g. WM_DISPLAYCHANGE) re-enables probing.
        session.invalidate_cache();
        let _ = session.probe(&mock, "EDP1");
        assert_eq!(mock.read_calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    #[ignore = "Requires live Windows display hardware; run with -- --ignored --nocapture"]
    fn live_native_ddcci_probe_and_safe_roundtrip() {
        let monitors = crate::monitor::list_monitors();
        assert!(!monitors.is_empty(), "expected at least one connected display");
        println!("=== Live Native DDC/CI Probe ({} monitor(s)) ===", monitors.len());

        let mut session = DdcciSession::default();
        let mut any_ddcci = false;

        for m in &monitors {
            let t0 = Instant::now();
            let raw = RealDdcci.read_vcp_luminance(&m.edid_id);
            let elapsed_ms = t0.elapsed().as_millis();
            println!(
                "Monitor: {} ({}) [{}] hdr={} acm={} -> raw={:?} ({} ms)",
                m.model, m.device_name, m.edid_id, m.hdr_active, m.acm_active, raw, elapsed_ms
            );

            let probe = session.probe(&RealDdcci, &m.edid_id);
            println!("  DdcciSession::probe -> {:?}", probe);

            if let Ok((orig_raw, max_val)) = raw {
                any_ddcci = true;
                let orig_pct = vcp_to_percent(orig_raw, max_val);
                let target_pct = if orig_pct >= 5 { orig_pct - 2 } else { orig_pct + 2 };
                let expected_target_raw = percent_to_vcp(target_pct, max_val);

                println!(
                    "  Testing safe DDC/CI write: orig={} ({}%), target={} ({}%)",
                    orig_raw, orig_pct, expected_target_raw, target_pct
                );
                session
                    .apply_brightness(&RealDdcci, &m.edid_id, target_pct)
                    .expect("apply_brightness failed on DDC/CI-capable monitor");
                std::thread::sleep(Duration::from_millis(250));
                let after_apply = RealDdcci.read_vcp_luminance(&m.edid_id);
                println!("  Readback after apply -> {:?}", after_apply);

                let restored = session
                    .restore_brightness(&RealDdcci, &m.edid_id)
                    .expect("restore_brightness failed");
                assert!(restored, "expected baseline to be restored");
                std::thread::sleep(Duration::from_millis(250));
                let after_restore = RealDdcci.read_vcp_luminance(&m.edid_id);
                println!("  Readback after restore -> {:?}", after_restore);

                if let Ok((applied_raw, _)) = after_apply {
                    assert_eq!(
                        applied_raw, expected_target_raw,
                        "hardware OSD luminance did not move to target"
                    );
                }
                if let Ok((restored_raw, _)) = after_restore {
                    assert_eq!(
                        restored_raw, orig_raw,
                        "hardware OSD luminance did not restore to exact baseline"
                    );
                }
            }
        }

        println!("=== Done (any_ddcci_supported={}) ===", any_ddcci);
    }
}
