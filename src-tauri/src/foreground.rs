// ── Foreground window probe (Windows-only) ────────────────────────────────
//
// Answers two questions once per watcher tick:
//   1. Which executable owns the window the user is looking at?
//   2. Which display is that window on (`\\.\DISPLAYn`)?
//
// Both answers are derived from Win32 with no caching, so a window dragged
// between monitors resolves to its new display on the next tick. The public
// surface is deliberately tiny; all policy lives in `auto_apply`.

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::auto_apply::normalize_program_name;

/// The window currently in front of the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedWindow {
    /// Lowercased executable base name, e.g. `chrome.exe`.
    pub program: String,
    /// `\\.\DISPLAYn` for the monitor holding the window; `None` when the
    /// window cannot be located on any display.
    pub device_name: Option<String>,
}

/// Reduce a full image path to the stored program identity. Kept separate
/// from the Win32 calls so the path rules are unit-testable.
pub fn program_from_path(path: &str) -> Option<String> {
    normalize_program_name(path)
}

/// Read the current foreground window, the process that owns it, and the
/// display it sits on. Returns `None` when there is no foreground window, the
/// owning process cannot be opened, or its image path is unreadable.
pub fn foreground_window() -> Option<ObservedWindow> {
    // SAFETY: GetForegroundWindow takes no arguments and may return a null
    // HWND when the desktop has no foreground window (lock screen, secure
    // desktop). We treat that as "nothing observed".
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }

    let program = process_name_for_window(hwnd)?;
    Some(ObservedWindow {
        program,
        device_name: device_name_for_window(hwnd),
    })
}

/// Executable base name of the current process, used to ignore ChromaDeck's
/// own windows (opening the app must never trigger a revert).
pub fn self_program() -> Option<String> {
    std::env::current_exe()
        .ok()
        .and_then(|path| program_from_path(&path.to_string_lossy()))
}

fn process_name_for_window(hwnd: HWND) -> Option<String> {
    let mut pid: u32 = 0;
    // SAFETY: the out-pointer is valid for the duration of the call.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }

    // SAFETY: OpenProcess with a limited-information handle; the returned
    // handle is closed on every path below.
    let process: HANDLE =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: buf is a live, writable buffer of `len` UTF-16 units and `len`
    // points to that capacity.
    let queried = unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
    };
    // SAFETY: `process` is a handle we own from OpenProcess; closing it once
    // here covers both the success and failure paths.
    unsafe {
        let _ = CloseHandle(process);
    }
    queried.ok()?;

    let path = String::from_utf16_lossy(&buf[..len as usize]);
    program_from_path(&path)
}

fn device_name_for_window(hwnd: HWND) -> Option<String> {
    // SAFETY: MonitorFromWindow returns either a valid HMONITOR or NULL; the
    // nearest-monitor fallback keeps a dragged window resolvable.
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_invalid() {
        return None;
    }

    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: cbSize is set as GetMonitorInfoW requires, and the struct is
    // large enough for the extended (szDevice) form.
    let ok = unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) };
    if !ok.as_bool() {
        return None;
    }

    let name: String = info
        .szDevice
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8 as char)
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_executable_base_name_from_full_path() {
        assert_eq!(
            program_from_path(r"C:\Games\Game.EXE"),
            Some("game.exe".into())
        );
        assert_eq!(
            program_from_path(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
            Some("chrome.exe".into())
        );
        assert_eq!(program_from_path(""), None);
    }
}
