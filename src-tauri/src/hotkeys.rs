// ── Global hotkeys (Windows RegisterHotKey) ─────────────────────────────────
//
// Zero-dependency native Windows global hotkey listener. Runs a tiny Win32
// message thread waiting on `WM_HOTKEY` so shortcuts fire reliably even while
// ChromaDeck is hidden in the system tray or behind a fullscreen game.
//
// Supports:
//   - Per-preset shortcuts (`Preset.shortcut`) targeting the foreground monitor
//   - Global Reapply shortcut (`Settings.reapply_shortcut`)
//   - Global Cycle-preset shortcut (`Settings.cycle_shortcut`)
//   - Temporary suspension (`set_hotkeys_suspended_cmd`) while recording chords

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_HOTKEY, WM_USER,
};

use crate::auto_apply_runtime;
use crate::store::{AppStore, Preset};

/// Custom Win32 thread message instructing the hotkey thread to re-sync bindings.
const WM_HOTKEY_RELOAD: u32 = WM_USER + 1;

/// Emitted to the frontend when a global hotkey executes an action.
pub const HOTKEY_TRIGGERED_EVENT: &str = "hotkey://triggered";

#[derive(Debug, Clone, Serialize)]
pub struct HotkeyEvent {
    pub action: String,
    pub message: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GlobalHotkeySettings {
    #[serde(default)]
    pub reapply_shortcut: Option<String>,
    #[serde(default)]
    pub cycle_shortcut: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParsedShortcut {
    pub mods: u32,
    pub vk: u32,
}

/// Normalize a user-supplied shortcut string (e.g. `"alt+ctrl+f1"` -> `"Ctrl+Alt+F1"`).
/// Requires at least one modifier key and exactly one primary key.
pub fn normalize_shortcut(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("shortcut cannot be empty".into());
    }

    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut win = false;
    let mut key_name: Option<String> = None;

    for part in trimmed.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            "win" | "meta" | "super" | "cmd" | "command" => win = true,
            other => {
                if key_name.is_some() {
                    return Err(format!("shortcut has multiple primary keys ('{other}')"));
                }
                let (canonical, _vk) = parse_primary_key(other)?;
                key_name = Some(canonical);
            }
        }
    }

    if !(ctrl || alt || shift || win) {
        return Err("shortcut must include at least one modifier (Ctrl, Alt, Shift, or Win)".into());
    }
    let key = key_name.ok_or_else(|| "shortcut must include a key (e.g. F1, 1, A)".to_string())?;

    let mut parts = Vec::new();
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    if win {
        parts.push("Win");
    }
    parts.push(&key);
    Ok(parts.join("+"))
}

/// Parse a shortcut string into Win32 `RegisterHotKey` modifiers and virtual key code.
pub fn parse_shortcut(raw: &str) -> Result<ParsedShortcut, String> {
    let canonical = normalize_shortcut(raw)?;
    let mut mods: u32 = MOD_NOREPEAT.0;
    let mut vk: Option<u32> = None;

    for part in canonical.split('+') {
        match part {
            "Ctrl" => mods |= MOD_CONTROL.0,
            "Alt" => mods |= MOD_ALT.0,
            "Shift" => mods |= MOD_SHIFT.0,
            "Win" => mods |= MOD_WIN.0,
            key => {
                let (_, code) = parse_primary_key(&key.to_ascii_lowercase())?;
                vk = Some(code);
            }
        }
    }

    Ok(ParsedShortcut {
        mods,
        vk: vk.ok_or_else(|| "missing primary key".to_string())?,
    })
}

fn parse_primary_key(lower: &str) -> Result<(String, u32), String> {
    if lower.len() == 1 {
        let b = lower.as_bytes()[0];
        if b.is_ascii_alphabetic() {
            let upper = (b as char).to_ascii_uppercase();
            return Ok((upper.to_string(), upper as u32));
        }
        if b.is_ascii_digit() {
            return Ok(((b as char).to_string(), b as u32));
        }
    }
    if let Some(num) = lower.strip_prefix('f') {
        if let Ok(n) = num.parse::<u32>() {
            if (1..=12).contains(&n) {
                return Ok((format!("F{n}"), 0x70 + (n - 1)));
            }
        }
    }
    match lower {
        "space" => Ok(("Space".into(), 0x20)),
        "home" => Ok(("Home".into(), 0x24)),
        "end" => Ok(("End".into(), 0x23)),
        "insert" | "ins" => Ok(("Insert".into(), 0x2D)),
        "delete" | "del" => Ok(("Delete".into(), 0x2E)),
        "pageup" | "pgup" => Ok(("PageUp".into(), 0x21)),
        "pagedown" | "pgdn" => Ok(("PageDown".into(), 0x22)),
        "up" | "arrowup" => Ok(("Up".into(), 0x26)),
        "down" | "arrowdown" => Ok(("Down".into(), 0x28)),
        "left" | "arrowleft" => Ok(("Left".into(), 0x25)),
        "right" | "arrowright" => Ok(("Right".into(), 0x27)),
        "-" | "minus" => Ok(("-".into(), 0xBD)),
        "=" | "equal" | "plus" => Ok(("=".into(), 0xBB)),
        "[" | "bracketleft" => Ok(("[".into(), 0xDB)),
        "]" | "bracketright" => Ok(("]".into(), 0xDD)),
        _ => Err(format!("unsupported shortcut key '{lower}'")),
    }
}

/// Pure helper for global cycle hotkey: select the next preset in library order
/// after `current_preset_id` (wrapping to index 0). Returns `None` if empty.
pub fn next_preset_in_cycle<'a>(
    presets: &'a [Preset],
    current_preset_id: Option<&str>,
) -> Option<&'a Preset> {
    if presets.is_empty() {
        return None;
    }
    let idx = current_preset_id
        .and_then(|id| presets.iter().position(|p| p.id == id))
        .map(|i| (i + 1) % presets.len())
        .unwrap_or(0);
    presets.get(idx)
}

#[derive(Debug, Clone)]
enum BindingAction {
    ApplyPreset(String),
    Reapply,
    Cycle,
}

#[derive(Debug, Clone)]
struct Binding {
    id: i32,
    shortcut: ParsedShortcut,
    action: BindingAction,
}

/// Shared state for the Win32 hotkey worker thread.
pub struct HotkeyState {
    thread_id: AtomicU32,
    suspended: AtomicBool,
    desired: Mutex<Vec<Binding>>,
}

impl Default for HotkeyState {
    fn default() -> Self {
        Self {
            thread_id: AtomicU32::new(0),
            suspended: AtomicBool::new(false),
            desired: Mutex::new(Vec::new()),
        }
    }
}

fn collect_desired_bindings(app: &AppHandle) -> Vec<Binding> {
    let Some(store_state) = app.try_state::<AppStore>() else {
        return Vec::new();
    };
    let Ok(store) = store_state.0.lock() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut next_id = 1i32;

    for preset in store.list_presets() {
        if let Some(ref sc) = preset.shortcut {
            if let Ok(parsed) = parse_shortcut(sc) {
                out.push(Binding {
                    id: next_id,
                    shortcut: parsed,
                    action: BindingAction::ApplyPreset(preset.id),
                });
                next_id += 1;
            }
        }
    }

    let global = store.global_hotkeys();
    if let Some(ref sc) = global.reapply_shortcut {
        if let Ok(parsed) = parse_shortcut(sc) {
            out.push(Binding {
                id: next_id,
                shortcut: parsed,
                action: BindingAction::Reapply,
            });
            next_id += 1;
        }
    }
    if let Some(ref sc) = global.cycle_shortcut {
        if let Ok(parsed) = parse_shortcut(sc) {
            out.push(Binding {
                id: next_id,
                shortcut: parsed,
                action: BindingAction::Cycle,
            });
        }
    }

    out
}

/// Signal the hotkey thread to re-register all bindings from the store.
pub fn sync_hotkeys(app: &AppHandle) {
    let Some(state) = app.try_state::<HotkeyState>() else {
        return;
    };
    let bindings = collect_desired_bindings(app);
    if let Ok(mut desired) = state.desired.lock() {
        *desired = bindings;
    }
    let tid = state.thread_id.load(Ordering::SeqCst);
    if tid != 0 {
        // SAFETY: PostThreadMessageW wakes the hotkey thread's GetMessageW loop.
        unsafe {
            let _ = PostThreadMessageW(tid, WM_HOTKEY_RELOAD, WPARAM(0), LPARAM(0));
        }
    }
}

/// Start the background Win32 `RegisterHotKey` message loop once during setup.
pub fn start_hotkey_thread(app: AppHandle) {
    // Seed initial desired bindings before spawning.
    if let Some(state) = app.try_state::<HotkeyState>() {
        let bindings = collect_desired_bindings(&app);
        if let Ok(mut desired) = state.desired.lock() {
            *desired = bindings;
        }
    }

    std::thread::Builder::new()
        .name("chromadeck-hotkeys".into())
        .spawn(move || {
            // Force Win32 message queue creation before publishing thread ID.
            let mut msg = MSG::default();
            unsafe {
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            }
            let tid = unsafe { GetCurrentThreadId() };
            if let Some(state) = app.try_state::<HotkeyState>() {
                state.thread_id.store(tid, Ordering::SeqCst);
            }

            let mut registered: HashMap<i32, BindingAction> = HashMap::new();
            apply_registrations(&app, &mut registered);

            loop {
                // SAFETY: GetMessageW blocks until a message arrives for this thread.
                let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
                if ret.0 <= 0 {
                    break;
                }
                match msg.message {
                    WM_HOTKEY_RELOAD => {
                        apply_registrations(&app, &mut registered);
                    }
                    WM_HOTKEY => {
                        let id = msg.wParam.0 as i32;
                        if let Some(action) = registered.get(&id).cloned() {
                            let handle = app.clone();
                            tauri::async_runtime::spawn_blocking(move || {
                                dispatch_action(&handle, action);
                            });
                        }
                    }
                    _ => {}
                }
            }

            for id in registered.keys() {
                unsafe {
                    let _ = UnregisterHotKey(None, *id);
                }
            }
        })
        .ok();
}

fn apply_registrations(app: &AppHandle, registered: &mut HashMap<i32, BindingAction>) {
    for id in registered.keys() {
        // SAFETY: unregistering IDs previously registered on this thread.
        unsafe {
            let _ = UnregisterHotKey(None, *id);
        }
    }
    registered.clear();

    let Some(state) = app.try_state::<HotkeyState>() else {
        return;
    };
    if state.suspended.load(Ordering::SeqCst) {
        return;
    }
    let bindings = match state.desired.lock() {
        Ok(b) => b.clone(),
        Err(_) => return,
    };

    for binding in bindings {
        // SAFETY: RegisterHotKey with NULL hwnd posts WM_HOTKEY to this thread.
        let ok = unsafe {
            RegisterHotKey(
                None,
                binding.id,
                HOT_KEY_MODIFIERS(binding.shortcut.mods),
                binding.shortcut.vk,
            )
        };
        if ok.is_ok() {
            registered.insert(binding.id, binding.action);
        }
    }
}

fn dispatch_action(app: &AppHandle, action: BindingAction) {
    let Some(store_state) = app.try_state::<AppStore>() else {
        return;
    };

    match action {
        BindingAction::Reapply => {
            let Ok(_op) = auto_apply_runtime::DISPLAY_OPERATION.lock() else {
                return;
            };
            let Ok((pins, presets)) = store_state
                .0
                .lock()
                .map(|s| (s.list_pins(), s.list_presets()))
            else {
                return;
            };
            let api = crate::color::RealColorApi;
            let nv = crate::nvapi::RealNvapi;
            let reader = crate::enforce::RealStateReader;
            let events = crate::enforce::check_once(&api, &nv, &reader, &pins, &presets, true);
            let applied = events.iter().filter(|e| e.applied).count();
            let first_err = events.iter().find_map(|e| e.error.clone());
            let _ = app.emit(
                HOTKEY_TRIGGERED_EVENT,
                HotkeyEvent {
                    action: "reapply".into(),
                    message: format!("Hotkey reapply {applied}/{}", events.len()),
                    error: first_err,
                },
            );
        }
        BindingAction::ApplyPreset(preset_id) => {
            apply_preset_on_active_monitor(app, &preset_id, "apply");
        }
        BindingAction::Cycle => {
            let Ok(_op) = auto_apply_runtime::DISPLAY_OPERATION.lock() else {
                return;
            };
            let monitors = crate::monitor::list_monitors();
            let fg = crate::cli::foreground_monitor_edid(&monitors);
            let Ok(targets) = crate::cli::resolve_monitor_targets(None, &monitors, fg.as_deref())
            else {
                return;
            };
            let target_edid = targets[0].clone();
            let next_id = {
                let Ok(s) = store_state.0.lock() else {
                    return;
                };
                let presets = s.list_presets();
                let applied = s.list_applied();
                let current = applied.get(&target_edid).map(String::as_str);
                next_preset_in_cycle(&presets, current).map(|p| p.id.clone())
            };
            drop(_op);
            if let Some(pid) = next_id {
                apply_preset_on_active_monitor(app, &pid, "cycle");
            }
        }
    }
}

fn apply_preset_on_active_monitor(app: &AppHandle, preset_id: &str, kind: &str) {
    let Ok(_op) = auto_apply_runtime::DISPLAY_OPERATION.lock() else {
        return;
    };
    let Some(store_state) = app.try_state::<AppStore>() else {
        return;
    };
    let (preset, profiles_dir) = {
        let Ok(store) = store_state.0.lock() else {
            return;
        };
        let Some(p) = store.list_presets().into_iter().find(|p| p.id == preset_id) else {
            return;
        };
        (
            p,
            store.data_dir().join("profiles").to_string_lossy().into_owned(),
        )
    };
    let monitors = crate::monitor::list_monitors();
    let fg = crate::cli::foreground_monitor_edid(&monitors);
    let Ok(targets) = crate::cli::resolve_monitor_targets(None, &monitors, fg.as_deref()) else {
        return;
    };
    let target_edid = &targets[0];

    auto_apply_runtime::relinquish_manual(app, target_edid);
    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let outcome =
        crate::color::apply_preset_detailed(&api, &nv, &preset, target_edid, &profiles_dir);
    if outcome.result.error.is_none() {
        if let Ok(mut store) = store_state.0.lock() {
            let _ = store.record_applied(target_edid, preset_id);
        }
    }
    crate::tray::refresh_tray(app);
    let _ = app.emit(crate::tray::STORE_CHANGED_EVENT, ());
    let _ = app.emit(
        HOTKEY_TRIGGERED_EVENT,
        HotkeyEvent {
            action: kind.into(),
            message: format!("Hotkey applied {}", preset.name),
            error: outcome.result.error,
        },
    );
}

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn set_hotkeys_suspended_cmd(app: AppHandle, suspended: bool) {
    if let Some(state) = app.try_state::<HotkeyState>() {
        state.suspended.store(suspended, Ordering::SeqCst);
        let tid = state.thread_id.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_HOTKEY_RELOAD, WPARAM(0), LPARAM(0));
            }
        }
    }
}

#[tauri::command]
pub fn get_global_hotkeys_cmd(
    state: tauri::State<'_, AppStore>,
) -> Result<GlobalHotkeySettings, String> {
    let store = state.0.lock().map_err(|e| e.to_string())?;
    Ok(store.global_hotkeys())
}

#[tauri::command]
pub fn set_global_hotkeys_cmd(
    app: AppHandle,
    state: tauri::State<'_, AppStore>,
    hotkeys: GlobalHotkeySettings,
) -> Result<GlobalHotkeySettings, String> {
    let saved = {
        let mut store = state.0.lock().map_err(|e| e.to_string())?;
        store
            .set_global_hotkeys(hotkeys)
            .map_err(|e| e.to_string())?
    };
    sync_hotkeys(&app);
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(id: &str, name: &str) -> Preset {
        Preset {
            id: id.into(),
            name: name.into(),
            icc_hash: String::new(),
            icc_filename: String::new(),
            brightness: 50.0,
            contrast: 50.0,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 1.0,
            vibrance: 50.0,
            saturation: 50.0,
            hue_deg: 0.0,
            color_model: "nvcp-v1".into(),
            color_tag: None,
            tag_shape: None,
            attached_programs: vec![],
            shortcut: None,
        }
    }

    #[test]
    fn normalizes_and_parses_valid_shortcuts() {
        assert_eq!(normalize_shortcut("alt+ctrl+f1").unwrap(), "Ctrl+Alt+F1");
        assert_eq!(
            normalize_shortcut("shift + control + a").unwrap(),
            "Ctrl+Shift+A"
        );
        let parsed = parse_shortcut("Ctrl+Alt+F1").unwrap();
        assert_eq!(parsed.vk, 0x70);
        assert_ne!(parsed.mods & MOD_CONTROL.0, 0);
        assert_ne!(parsed.mods & MOD_ALT.0, 0);
    }

    #[test]
    fn rejects_invalid_shortcuts() {
        assert!(normalize_shortcut("").is_err());
        assert!(normalize_shortcut("F1").is_err(), "must require a modifier");
        assert!(normalize_shortcut("Ctrl+Alt").is_err(), "must require a key");
        assert!(normalize_shortcut("Ctrl+A+B").is_err(), "multiple keys");
    }

    #[test]
    fn cycles_presets_and_wraps() {
        let list = [preset("p1", "A"), preset("p2", "B"), preset("p3", "C")];
        assert_eq!(next_preset_in_cycle(&list, None).map(|p| p.id.as_str()), Some("p1"));
        assert_eq!(next_preset_in_cycle(&list, Some("p1")).map(|p| p.id.as_str()), Some("p2"));
        assert_eq!(next_preset_in_cycle(&list, Some("p3")).map(|p| p.id.as_str()), Some("p1"));
        assert!(next_preset_in_cycle(&[], None).is_none());
    }
}
