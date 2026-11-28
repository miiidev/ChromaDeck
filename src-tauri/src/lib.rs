mod auto_apply;
mod auto_apply_runtime;
mod backend;
mod bundle;
mod capabilities;
mod cli;
mod color;
mod ddcci;
mod diagnostics;
mod display_events;
mod enforce;
mod foreground;
mod hotkeys;
mod identify;
mod monitor;
mod nvapi;
mod nvgamma;
mod schedule;
mod store;
mod tray;
mod updater;

use capabilities::get_display_capabilities;
use color::{
    apply_preset_cmd, preview_preset_cmd, probe_monitor_cmd, reset_all_monitors_cmd,
    reset_monitor_cmd, restore_preview_cmd,
};
use enforce::reapply_now_cmd;
use identify::{identify_info_cmd, identify_monitors_cmd};
use monitor::list_monitors_cmd;
use nvapi::vibrance_supported_cmd;
use nvgamma::capture_nvcp_cmd;
use store::{
    create_preset_cmd, delete_preset_cmd, get_auto_apply_enabled_cmd, import_icc_cmd,
    list_applied_cmd, list_pins_cmd, list_presets_cmd, pin_preset_cmd, set_auto_apply_enabled_cmd,
    set_monitor_name_cmd, unpin_monitor_cmd, update_preset_cmd, AppStore,
};
use tauri::{Emitter, Manager, WindowEvent};
use updater::{check_for_update_cmd, download_and_install_cmd};

/// Show + focus the main window and notify the frontend so it replays its
/// entrance animation. The main window is hidden (not destroyed) on close,
/// so without the emit the staged load animation would only play once per
/// app lifetime instead of on every open.
pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("window-shown", ());
    }
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
        store::Store::new(store_path.clone()).expect("failed to initialise preset store"),
    ));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let parsed = cli::parse_cli_args(&argv);
            if let Some(err) = parsed.error {
                eprintln!("ChromaDeck CLI error: {err}");
                return;
            }
            if let Some(cmd) = parsed.command {
                let handle = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Err(err) = cli::execute_cli(&handle, cmd) {
                        eprintln!("ChromaDeck CLI failed: {err}");
                    }
                });
                return;
            }
            if !parsed.headless {
                show_main_window(app);
            }
        }))
        .manage(app_store)
        .manage(auto_apply_runtime::AutoState::new(store_path))
        .manage(hotkeys::HotkeyState::default())
        .manage(schedule::ScheduleState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("ChromaDeck")
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            check_for_update_cmd,
            download_and_install_cmd,
            list_monitors_cmd,
            identify_monitors_cmd,
            identify_info_cmd,
            list_presets_cmd,
            create_preset_cmd,
            update_preset_cmd,
            delete_preset_cmd,
            import_icc_cmd,
            apply_preset_cmd,
            preview_preset_cmd,
            restore_preview_cmd,
            vibrance_supported_cmd,
            reset_monitor_cmd,
            reset_all_monitors_cmd,
            probe_monitor_cmd,
            pin_preset_cmd,
            unpin_monitor_cmd,
            list_pins_cmd,
            list_applied_cmd,
            get_auto_apply_enabled_cmd,
            set_auto_apply_enabled_cmd,
            auto_apply_runtime::list_auto_applied_cmd,
            foreground::list_running_programs_cmd,
            hotkeys::set_hotkeys_suspended_cmd,
            hotkeys::get_global_hotkeys_cmd,
            hotkeys::set_global_hotkeys_cmd,
            bundle::export_presets_cmd,
            bundle::import_presets_cmd,
            schedule::list_schedules_cmd,
            schedule::save_schedules_cmd,
            diagnostics::export_diagnostics_cmd,
            ddcci::probe_ddcci_cmd,
            reapply_now_cmd,
            capture_nvcp_cmd,
            set_monitor_name_cmd,
            get_display_capabilities,
        ])
        .setup(|app| {
            tray::build_tray(app.handle())?;
            hotkeys::start_hotkey_thread(app.handle().clone());
            display_events::start_display_event_listener(app.handle().clone());

            // Cold-start CLI handling (`--headless` hides main window to tray;
            // explicit CLI actions run without popping the UI).
            let startup_args: Vec<String> = std::env::args().collect();
            let parsed = cli::parse_cli_args(&startup_args);
            if parsed.headless || parsed.command.is_some() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            if let Some(cmd) = parsed.command {
                let cli_handle = app.handle().clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Err(err) = cli::execute_cli(&cli_handle, cmd) {
                        eprintln!("ChromaDeck CLI failed: {err}");
                    }
                });
            }

            let handle = app.handle().clone();
            let auto_handle = app.handle().clone();
            let schedule_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Reassert pinned/applied presets once through the backend
                // registry before the drift loop takes over. Runs off the main
                // thread; disconnected displays are skipped.
                crate::enforce::startup_reapply(&handle);
                crate::enforce::enforce_loop(handle).await;
            });
            // Foreground watcher runs on its own task so a slow colour write
            // never delays drift enforcement.
            tauri::async_runtime::spawn(async move {
                crate::auto_apply_runtime::auto_apply_loop(auto_handle).await;
            });
            // Time-of-day schedule watcher runs independently every 30 s.
            tauri::async_runtime::spawn(async move {
                crate::schedule::schedule_loop(schedule_handle).await;
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                // Let identify- overlay windows close normally; prevent
                // close + hide the main window (system-tray style).
                if label.starts_with("identify-") {
                    return; // allow normal close / destroy
                }
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
