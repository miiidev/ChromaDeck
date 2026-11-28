// ── Cross-vendor HDR / Windows 11 ACM detection & WM_DISPLAYCHANGE listener ─
//
// 1. Queries `QueryDisplayConfig` + `DisplayConfigGetDeviceInfo` with
//    `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO` per active path to detect both
//    HDR active and Windows 11 Auto Color Management (ACM) across NVIDIA,
//    AMD, Intel, and GDI displays uniformly.
// 2. Spawns a lightweight hidden top-level Win32 window listening for
//    `WM_DISPLAYCHANGE` and `WM_SETTINGCHANGE` (broadcast messages do NOT
//    reach `HWND_MESSAGE` message-only windows, so we use a 0×0 never-shown
//    `WS_OVERLAPPED` + `WS_EX_TOOLWINDOW` window).
//    On hot-plug or `Win+Alt+B` HDR toggle:
//      - invalidates DDC/CI and Advanced Color caches
//      - emits `monitors://changed` to the frontend for instant UI refresh
//      - when any display transitions HDR -> SDR, triggers a non-forced
//        `enforce::check_once` pass to re-assert pinned presets cleanly.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use windows::core::PCWSTR;
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
    TranslateMessage, CW_USEDEFAULT, MSG, WM_DISPLAYCHANGE, WM_SETTINGCHANGE, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};

pub const MONITORS_CHANGED_EVENT: &str = "monitors://changed";

/// Cross-vendor Windows Advanced Color state for one display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DisplayColorMode {
    pub advanced_color_supported: bool,
    /// True when Windows HDR is active (driver SDR LUTs bypassed).
    pub hdr_active: bool,
    /// True when Windows 11 Auto Color Management (ACM / WCG) is enforced on SDR.
    pub acm_active: bool,
}

/// Pure classifier over `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO`'s bitflags:
///   bit 0: `advancedColorSupported`
///   bit 1: `advancedColorEnabled`
///   bit 2: `wideColorEnforced` (Windows 11 ACM on SDR)
///   bit 3: `advancedColorForceDisabled`
pub fn classify_advanced_color_bits(value: u32) -> DisplayColorMode {
    let supported = (value & (1 << 0)) != 0;
    let enabled = (value & (1 << 1)) != 0;
    let wide_color_enforced = (value & (1 << 2)) != 0;
    let force_disabled = (value & (1 << 3)) != 0;

    if force_disabled {
        return DisplayColorMode {
            advanced_color_supported: supported,
            hdr_active: false,
            acm_active: false,
        };
    }

    DisplayColorMode {
        advanced_color_supported: supported,
        // When Windows 11 ACM is active on an SDR display, `wideColorEnforced`
        // is set alongside `advancedColorEnabled`; true HDR has `enabled && !wideColorEnforced`.
        hdr_active: enabled && !wide_color_enforced,
        acm_active: wide_color_enforced,
    }
}

/// Pure transition detector: returns every device name that was HDR in `prev`
/// and is now SDR (`!hdr_active`) in `curr`.
pub fn detect_hdr_to_sdr_returns(
    prev: &HashMap<String, DisplayColorMode>,
    curr: &HashMap<String, DisplayColorMode>,
) -> Vec<String> {
    let mut returned = Vec::new();
    for (dev, prev_mode) in prev {
        if prev_mode.hdr_active {
            if let Some(curr_mode) = curr.get(dev) {
                if !curr_mode.hdr_active {
                    returned.push(dev.clone());
                }
            }
        }
    }
    returned.sort();
    returned
}

/// Query Windows DisplayConfig for every active path's Advanced Color / HDR / ACM
/// state, keyed by uppercase GDI source device name (e.g. `\\.\DISPLAY1`).
pub fn query_all_display_color_modes() -> HashMap<String, DisplayColorMode> {
    let mut result = HashMap::new();
    unsafe {
        let mut path_count: u32 = 0;
        let mut mode_count: u32 = 0;
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count).0
            != 0
            || path_count == 0
        {
            return result;
        }

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
        .0 != 0
        {
            return result;
        }

        for path in paths.iter().take(path_count as usize) {
            let mut src_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut src_name.header) != 0 {
                continue;
            }
            let len = src_name
                .viewGdiDeviceName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(src_name.viewGdiDeviceName.len());
            let dev = String::from_utf16_lossy(&src_name.viewGdiDeviceName[..len])
                .to_ascii_uppercase();
            if dev.is_empty() {
                continue;
            }

            let mut color_info = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
                    size: std::mem::size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32,
                    adapterId: path.targetInfo.adapterId,
                    id: path.targetInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut color_info.header) != 0 {
                continue;
            }
            let bits = color_info.Anonymous.value;
            let mode = classify_advanced_color_bits(bits);
            // If cloned paths share a source device, OR active flags safely.
            result
                .entry(dev)
                .and_modify(|existing: &mut DisplayColorMode| {
                    existing.advanced_color_supported |= mode.advanced_color_supported;
                    existing.hdr_active |= mode.hdr_active;
                    existing.acm_active |= mode.acm_active;
                })
                .or_insert(mode);
        }
    }
    result
}

// ── Short-lived cache invalidated immediately on WM_DISPLAYCHANGE ───────────

const MODE_CACHE_TTL: Duration = Duration::from_secs(2);

struct ModeCache {
    fetched_at: Instant,
    modes: HashMap<String, DisplayColorMode>,
}

static MODE_CACHE: OnceLock<Mutex<Option<ModeCache>>> = OnceLock::new();

fn cached_modes() -> HashMap<String, DisplayColorMode> {
    let lock = MODE_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = lock.lock() {
        if let Some(ref c) = *guard {
            if c.fetched_at.elapsed() < MODE_CACHE_TTL {
                return c.modes.clone();
            }
        }
    }
    let fresh = query_all_display_color_modes();
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(ModeCache {
            fetched_at: Instant::now(),
            modes: fresh.clone(),
        });
    }
    fresh
}

pub fn invalidate_cache() {
    if let Some(lock) = MODE_CACHE.get() {
        if let Ok(mut guard) = lock.lock() {
            *guard = None;
        }
    }
}

/// Look up cached HDR / ACM mode for a GDI device name (e.g. `\\.\DISPLAY1`).
pub fn color_mode_for_device(device_name: &str) -> DisplayColorMode {
    let key = device_name.to_ascii_uppercase();
    cached_modes().get(&key).copied().unwrap_or_default()
}

/// Look up cached HDR / ACM mode for a ChromaDeck `edid_id`.
pub fn color_mode_for_edid(edid_id: &str) -> DisplayColorMode {
    let Some(device) = crate::nvapi::resolve_device_name(edid_id) else {
        return DisplayColorMode::default();
    };
    color_mode_for_device(&device)
}

// ── Hidden Win32 broadcast listener (WM_DISPLAYCHANGE / WM_SETTINGCHANGE) ───

struct ListenerContext {
    app: AppHandle,
    last_event: Mutex<Option<Instant>>,
    prev_modes: Mutex<HashMap<String, DisplayColorMode>>,
}

static LISTENER_CTX: OnceLock<ListenerContext> = OnceLock::new();

fn handle_display_broadcast() {
    let Some(ctx) = LISTENER_CTX.get() else {
        return;
    };

    // Debounce rapid bursts (e.g. multi-monitor mode switch emits several WM_DISPLAYCHANGE).
    {
        let Ok(mut last) = ctx.last_event.lock() else {
            return;
        };
        if let Some(prev_at) = *last {
            if prev_at.elapsed() < Duration::from_millis(250) {
                return;
            }
        }
        *last = Some(Instant::now());
    }

    // Invalidate caches so both the frontend refresh and enforcer see live state.
    invalidate_cache();
    crate::ddcci::invalidate_cache();

    let fresh = query_all_display_color_modes();
    if let Some(lock) = MODE_CACHE.get() {
        if let Ok(mut guard) = lock.lock() {
            *guard = Some(ModeCache {
                fetched_at: Instant::now(),
                modes: fresh.clone(),
            });
        }
    }

    let hdr_returned = match ctx.prev_modes.lock() {
        Ok(mut prev) => {
            let returns = detect_hdr_to_sdr_returns(&prev, &fresh);
            *prev = fresh;
            returns
        }
        Err(_) => Vec::new(),
    };

    let app = ctx.app.clone();
    let _ = app.emit(MONITORS_CHANGED_EVENT, ());
    crate::tray::refresh_tray(&app);

    if !hdr_returned.is_empty() {
        let enforce_handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            // Brief settle for display driver LUT pipelines after HDR -> SDR flip.
            std::thread::sleep(Duration::from_millis(400));
            let _op = crate::auto_apply_runtime::DISPLAY_OPERATION
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let Some(store_state) = enforce_handle.try_state::<crate::store::AppStore>() else {
                return;
            };
            let (pins, presets) = match store_state.0.lock() {
                Ok(store) => (store.list_pins(), store.list_presets()),
                Err(_) => return,
            };
            if pins.is_empty() {
                return;
            }
            let api = crate::color::RealColorApi;
            let nv = crate::nvapi::RealNvapi;
            let reader = crate::enforce::RealStateReader;
            let _ = crate::enforce::check_once(&api, &nv, &reader, &pins, &presets, false);
            let _ = enforce_handle.emit(MONITORS_CHANGED_EVENT, ());
        });
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_DISPLAYCHANGE || msg == WM_SETTINGCHANGE {
        handle_display_broadcast();
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Spawn the lightweight background Win32 window listening for display hot-plug
/// (`WM_DISPLAYCHANGE`) and system setting / HDR changes (`WM_SETTINGCHANGE`).
pub fn start_display_event_listener(app: AppHandle) {
    let initial = query_all_display_color_modes();
    let _ = LISTENER_CTX.set(ListenerContext {
        app,
        last_event: Mutex::new(None),
        prev_modes: Mutex::new(initial),
    });

    let _ = std::thread::Builder::new()
        .name("chromadeck-display-events".into())
        .spawn(|| unsafe {
            let class_name: Vec<u16> = "ChromaDeckDisplayEventWindow\0".encode_utf16().collect();
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinstance.into(),
                lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);

            // Top-level invisible window (never ShowWindow'd) so Windows delivers
            // HWND_BROADCAST messages (WM_DISPLAYCHANGE / WM_SETTINGCHANGE).
            let Ok(hwnd) = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                PCWSTR::from_raw(class_name.as_ptr()),
                PCWSTR::null(),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                None,
                None,
                Some(hinstance.into()),
                None,
            ) else {
                return;
            };
            if hwnd.is_invalid() {
                return;
            }

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_advanced_color_distinguishes_sdr_hdr_acm_and_force_disabled() {
        // Pure SDR
        assert_eq!(
            classify_advanced_color_bits(0),
            DisplayColorMode {
                advanced_color_supported: false,
                hdr_active: false,
                acm_active: false,
            }
        );

        // HDR active: supported + enabled, wideColorEnforced = 0
        assert_eq!(
            classify_advanced_color_bits(0b0011),
            DisplayColorMode {
                advanced_color_supported: true,
                hdr_active: true,
                acm_active: false,
            }
        );

        // Windows 11 ACM on SDR: supported + enabled + wideColorEnforced
        assert_eq!(
            classify_advanced_color_bits(0b0111),
            DisplayColorMode {
                advanced_color_supported: true,
                hdr_active: false,
                acm_active: true,
            }
        );

        // Force disabled overrides enabled/ACM
        assert_eq!(
            classify_advanced_color_bits(0b1111),
            DisplayColorMode {
                advanced_color_supported: true,
                hdr_active: false,
                acm_active: false,
            }
        );
    }

    #[test]
    fn detect_hdr_to_sdr_returns_flags_only_returning_displays() {
        let mut prev = HashMap::new();
        prev.insert(
            r"\\.\DISPLAY1".into(),
            DisplayColorMode {
                advanced_color_supported: true,
                hdr_active: true,
                acm_active: false,
            },
        );
        prev.insert(
            r"\\.\DISPLAY2".into(),
            DisplayColorMode {
                advanced_color_supported: true,
                hdr_active: false,
                acm_active: true,
            },
        );

        let mut curr = HashMap::new();
        curr.insert(
            r"\\.\DISPLAY1".into(),
            DisplayColorMode {
                advanced_color_supported: true,
                hdr_active: false,
                acm_active: false,
            },
        );
        curr.insert(
            r"\\.\DISPLAY2".into(),
            DisplayColorMode {
                advanced_color_supported: true,
                hdr_active: true,
                acm_active: false,
            },
        );

        assert_eq!(
            detect_hdr_to_sdr_returns(&prev, &curr),
            vec![r"\\.\DISPLAY1".to_string()]
        );
    }

    #[test]
    #[ignore = "Requires live Windows display hardware; run with -- --ignored --nocapture"]
    fn live_native_display_config_hdr_acm_and_broadcast_window() {
        let modes = query_all_display_color_modes();
        println!("=== Live DisplayConfig Advanced Color Modes ({}) ===", modes.len());
        for (dev, mode) in &modes {
            println!("  {} -> {:?}", dev, mode);
        }
        assert!(
            !modes.is_empty(),
            "QueryDisplayConfig returned 0 active displays"
        );

        let monitors = crate::monitor::list_monitors();
        assert!(!monitors.is_empty(), "list_monitors returned 0 displays");
        for m in &monitors {
            let caps = crate::capabilities::get_display_capabilities(m.edid_id.clone());
            println!(
                "  Monitor {} ({}): hdr={} acm={} backend={} ({})",
                m.model, m.device_name, m.hdr_active, m.acm_active, caps.backend_id, caps.backend_label
            );
            if m.hdr_active {
                assert!(
                    caps.adjustments
                        .iter()
                        .all(|a| matches!(a.status, crate::capabilities::CapabilityStatus::Unsupported)),
                    "HDR display must report all adjustments unsupported"
                );
            }
        }

        // Verify real Win32 hidden broadcast window creation & WM_DISPLAYCHANGE / WM_SETTINGCHANGE delivery.
        use std::sync::atomic::{AtomicUsize, Ordering};
        static HITS: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "system" fn test_wndproc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            if msg == WM_DISPLAYCHANGE || msg == WM_SETTINGCHANGE {
                HITS.fetch_add(1, Ordering::SeqCst);
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        let class_name: Vec<u16> = "ChromaDeckDisplayEventsSelfTest\0".encode_utf16().collect();
        unsafe {
            let hinst = GetModuleHandleW(None).unwrap_or_default();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(test_wndproc),
                hInstance: hinst.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            let atom = RegisterClassW(&wc);
            assert_ne!(atom, 0, "RegisterClassW failed");

            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(class_name.as_ptr()),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(hinst.into()),
                None,
            )
            .expect("CreateWindowExW failed");

            HITS.store(0, Ordering::SeqCst);
            let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                hwnd,
                WM_DISPLAYCHANGE,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
            let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                hwnd,
                WM_SETTINGCHANGE,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            let _ = windows::Win32::UI::WindowsAndMessaging::UnregisterClassW(
                PCWSTR(class_name.as_ptr()),
                Some(hinst.into()),
            );
        }
        assert_eq!(HITS.load(Ordering::SeqCst), 2, "expected both WM_DISPLAYCHANGE and WM_SETTINGCHANGE");
        println!("=== Win32 broadcast window self-test passed (2/2 messages delivered) ===");
    }
}
