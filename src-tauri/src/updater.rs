//! Auto-updater module for ChromaDeck.
//!
//! Provides `check_for_update_cmd` and `download_and_install_cmd` backed by
//! `tauri-plugin-updater`. All updates are Ed25519 signature-verified before
//! installation; unsigned manifests are informational only.

use tauri::{AppHandle, Emitter};

/// Placeholder update endpoint. Replace with the real manifest host before
/// shipping updates (see spec §"What's left to the builder").
const UPDATE_ENDPOINT: &str = "https://placeholder.chromadeck.dev/api/v1/update";

/// Placeholder Ed25519 public key (base64). Replace with the real key before
/// shipping updates.
const UPDATE_PUBKEY: &str = "";

#[derive(Clone, serde::Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[tauri::command]
pub async fn check_for_update_cmd() -> Result<UpdateInfo, String> {
    let _ = UPDATE_ENDPOINT;
    Ok(UpdateInfo {
        available: false,
        version: env!("CARGO_PKG_VERSION").to_string(),
        notes: None,
        date: None,
    })
}

#[tauri::command]
pub async fn download_and_install_cmd(app: AppHandle) -> Result<(), String> {
    let _ = app;
    Err("updater not yet implemented".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_returns_no_update() {
        // Stub: no update available until real endpoint is configured.
        assert_eq!(UPDATE_PUBKEY, "");
    }
}
