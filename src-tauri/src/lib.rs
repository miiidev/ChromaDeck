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
        .manage(app_store)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
