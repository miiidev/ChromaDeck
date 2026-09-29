mod color;
mod enforce;
mod monitor;
mod nvapi;
mod store;

use color::{apply_preset_cmd, reset_monitor_cmd};
use enforce::reapply_now_cmd;
use monitor::list_monitors_cmd;
use nvapi::vibrance_supported_cmd;
use store::{
    create_preset_cmd, delete_preset_cmd, import_icc_cmd, list_pins_cmd, list_presets_cmd,
    pin_preset_cmd, unpin_monitor_cmd, update_preset_cmd, AppStore,
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

// ── System tray ──────────────────────────────────────────────────────────────

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "tray-show", "Show", true, None::<&str>)?;
    let reapply = MenuItem::with_id(app, "tray-reapply", "Reapply now", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&show, &reapply, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("ChromaDeck")
        .icon(app.default_window_icon().cloned().unwrap())
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-quit" => app.exit(0),
            "tray-show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "tray-reapply" => {
                use tauri::Manager;
                if let Some(store) = app.try_state::<AppStore>() {
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
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;
    Ok(())
}

// ── Application entry point ──────────────────────────────────────────────────

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let store_path = store::default_store_path();
    let app_store = AppStore(std::sync::Mutex::new(
        store::Store::new(store_path).expect("failed to initialise preset store"),
    ));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .manage(app_store)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("ChromaDeck")
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            greet,
            list_monitors_cmd,
            list_presets_cmd,
            create_preset_cmd,
            update_preset_cmd,
            delete_preset_cmd,
            import_icc_cmd,
            apply_preset_cmd,
            vibrance_supported_cmd,
            reset_monitor_cmd,
            pin_preset_cmd,
            unpin_monitor_cmd,
            list_pins_cmd,
            reapply_now_cmd,
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                crate::enforce::enforce_loop(handle).await;
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
