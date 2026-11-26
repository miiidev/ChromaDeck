// ── Dynamic system tray menu (per-monitor preset submenus) ──────────────────
//
// Builds and refreshes the ChromaDeck system tray menu so users can switch
// presets or reset any connected display in two clicks without opening the
// main window. Menu item IDs use `::` as delimiter because EDID keys may
// contain `:` (e.g. `EDID:06AF-F7AD-00000000`) and `|` (port discriminator).

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::auto_apply_runtime;
use crate::store::AppStore;

pub const TRAY_ID: &str = "main-tray";
/// Emitted whenever a background surface (tray, CLI, global hotkey) mutates
/// applied state or pins so an open window refreshes cleanly.
pub const STORE_CHANGED_EVENT: &str = "store://changed";

const APPLY_PREFIX: &str = "tray-apply::";
const RESET_PREFIX: &str = "tray-reset::";

pub fn encode_apply_id(edid_id: &str, preset_id: &str) -> String {
    format!("{APPLY_PREFIX}{edid_id}::{preset_id}")
}

pub fn parse_apply_id(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix(APPLY_PREFIX)?.split_once("::")
}

pub fn encode_reset_id(edid_id: &str) -> String {
    format!("{RESET_PREFIX}{edid_id}")
}

pub fn parse_reset_id(id: &str) -> Option<&str> {
    id.strip_prefix(RESET_PREFIX)
}

fn display_label(m: &crate::monitor::Monitor) -> String {
    if !m.alias.is_empty() {
        m.alias.clone()
    } else if !m.model.is_empty() {
        m.model.clone()
    } else if !m.device_name.is_empty() {
        m.device_name.clone()
    } else {
        m.edid_id.clone()
    }
}

fn active_preset_for<'a>(
    edid_id: &str,
    pins: &'a std::collections::HashMap<String, String>,
    applied: &'a std::collections::HashMap<String, String>,
) -> Option<&'a str> {
    pins.get(edid_id)
        .or_else(|| {
            pins.iter()
                .find(|(k, _)| crate::monitor::key_match(k, edid_id))
                .map(|(_, v)| v)
        })
        .or_else(|| applied.get(edid_id))
        .or_else(|| {
            applied
                .iter()
                .find(|(k, _)| crate::monitor::key_match(k, edid_id))
                .map(|(_, v)| v)
        })
        .map(String::as_str)
}

/// Construct a fresh tray menu reflecting the current connected monitors,
/// presets, pins, applied state, and auto-apply toggle.
pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let (presets, pins, applied, names, auto_checked) = app
        .try_state::<AppStore>()
        .and_then(|s| {
            s.0.lock().ok().map(|store| {
                (
                    store.list_presets(),
                    store.list_pins(),
                    store.list_applied(),
                    store.list_monitor_names(),
                    store.auto_apply_enabled(),
                )
            })
        })
        .unwrap_or_default();

    let mut monitors = crate::monitor::list_monitors();
    for m in &mut monitors {
        if let Some(alias) = names.get(&m.edid_id).or_else(|| {
            names
                .iter()
                .find(|(k, _)| crate::monitor::key_match(k, &m.edid_id))
                .map(|(_, v)| v)
        }) {
            m.alias = alias.clone();
        }
    }
    let connected: Vec<_> = monitors.into_iter().filter(|m| m.connected).collect();

    let menu = Menu::new(app)?;
    let show = MenuItem::with_id(app, "tray-show", "Show ChromaDeck", true, None::<&str>)?;
    menu.append(&show)?;

    if !connected.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        for m in &connected {
            let sub = Submenu::new(app, display_label(m), true)?;
            let current_id = active_preset_for(&m.edid_id, &pins, &applied);
            if presets.is_empty() {
                let empty =
                    MenuItem::with_id(app, format!("tray-empty::{}", m.edid_id), "No presets yet", false, None::<&str>)?;
                sub.append(&empty)?;
            } else {
                for preset in &presets {
                    let is_active = current_id == Some(preset.id.as_str());
                    let label = match &preset.shortcut {
                        Some(sc) if !sc.is_empty() => format!("{}  ({})", preset.name, sc),
                        _ => preset.name.clone(),
                    };
                    let item = CheckMenuItem::with_id(
                        app,
                        encode_apply_id(&m.edid_id, &preset.id),
                        label,
                        true,
                        is_active,
                        None::<&str>,
                    )?;
                    sub.append(&item)?;
                }
            }
            sub.append(&PredefinedMenuItem::separator(app)?)?;
            let reset_item = MenuItem::with_id(
                app,
                encode_reset_id(&m.edid_id),
                "Reset to default",
                true,
                None::<&str>,
            )?;
            sub.append(&reset_item)?;
            menu.append(&sub)?;
        }
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let reapply = MenuItem::with_id(app, "tray-reapply", "Reapply now", true, None::<&str>)?;
    menu.append(&reapply)?;

    let auto_apply = CheckMenuItem::with_id(
        app,
        auto_apply_runtime::TRAY_CHECK_ID,
        "Auto-apply",
        true,
        auto_checked,
        None::<&str>,
    )?;
    if let Some(state) = app.try_state::<auto_apply_runtime::AutoState>() {
        if let Ok(mut item) = state.check_item.lock() {
            *item = Some(auto_apply.clone());
        }
    }
    menu.append(&auto_apply)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit", true, None::<&str>)?;
    menu.append(&quit)?;

    Ok(menu)
}

/// Rebuild the live system tray menu (best-effort, safe to call from any thread).
pub fn refresh_tray(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Create the initial tray icon and wire its menu & click handlers.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("ChromaDeck");
    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }
    tray.on_menu_event(|app, event| {
        let id = event.id.as_ref().to_string();
        match id.as_str() {
            "tray-quit" => {
                crate::enforce::shutdown_sweep(app);
                app.exit(0);
            }
            "tray-show" => {
                crate::show_main_window(app);
            }
            auto_apply_runtime::TRAY_CHECK_ID => {
                let current = app
                    .try_state::<AppStore>()
                    .and_then(|s| s.0.lock().ok().map(|s| s.auto_apply_enabled()))
                    .unwrap_or(false);
                let _ = auto_apply_runtime::set_auto_apply(app, !current);
            }
            "tray-reapply" => {
                let handle = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let Ok(_op) = auto_apply_runtime::DISPLAY_OPERATION.lock() else {
                        return;
                    };
                    if let Some(store) = handle.try_state::<AppStore>() {
                        if let Ok(s) = store.0.lock() {
                            let (pins, presets) = (s.list_pins(), s.list_presets());
                            drop(s);
                            let api = crate::color::RealColorApi;
                            let nv = crate::nvapi::RealNvapi;
                            let reader = crate::enforce::RealStateReader;
                            let _ = crate::enforce::check_once(
                                &api, &nv, &reader, &pins, &presets, true,
                            );
                        }
                    }
                });
            }
            _ => {
                if let Some((edid_id, preset_id)) = parse_apply_id(&id) {
                    let edid = edid_id.to_string();
                    let preset_id = preset_id.to_string();
                    let handle = app.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        apply_from_tray(&handle, &edid, &preset_id);
                    });
                } else if let Some(edid_id) = parse_reset_id(&id) {
                    let edid = edid_id.to_string();
                    let handle = app.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        reset_from_tray(&handle, &edid);
                    });
                }
            }
        }
    })
    .on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            crate::show_main_window(tray.app_handle());
        }
    })
    .build(app)?;
    Ok(())
}

fn apply_from_tray(app: &AppHandle, edid_id: &str, preset_id: &str) {
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
        let dir = store.data_dir().join("profiles");
        (p, dir.to_string_lossy().into_owned())
    };

    auto_apply_runtime::relinquish_manual(app, edid_id);
    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let outcome = crate::color::apply_preset_detailed(&api, &nv, &preset, edid_id, &profiles_dir);
    if outcome.result.error.is_none() {
        if let Ok(mut store) = store_state.0.lock() {
            let _ = store.record_applied(edid_id, preset_id);
        }
    }
    refresh_tray(app);
    let _ = app.emit(STORE_CHANGED_EVENT, ());
}

fn reset_from_tray(app: &AppHandle, edid_id: &str) {
    let Ok(_op) = auto_apply_runtime::DISPLAY_OPERATION.lock() else {
        return;
    };
    auto_apply_runtime::relinquish_manual(app, edid_id);
    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let result = crate::color::reset_monitor(&api, &nv, edid_id);
    if result.error.is_none() {
        if let Some(store_state) = app.try_state::<AppStore>() {
            if let Ok(mut store) = store_state.0.lock() {
                let _ = store.clear_applied(edid_id);
                let _ = store.unpin_monitor(edid_id);
            }
        }
    }
    refresh_tray(app);
    let _ = app.emit(STORE_CHANGED_EVENT, ());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_roundtrip_edids_with_colons_and_pipes() {
        let edid = r"EDID:06AF-F7AD-00000000|MON\1";
        let preset_id = "123e4567-e89b-12d3-a456-426614174000";
        let apply_id = encode_apply_id(edid, preset_id);
        assert_eq!(parse_apply_id(&apply_id), Some((edid, preset_id)));

        let reset_id = encode_reset_id(edid);
        assert_eq!(parse_reset_id(&reset_id), Some(edid));
    }
}
