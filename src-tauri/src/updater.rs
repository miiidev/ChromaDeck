//! Auto-updater module for ChromaDeck.
//!
//! Provides `check_for_update_cmd` and `download_and_install_cmd` backed by
//! `tauri-plugin-updater`. All updates are Ed25519 signature-verified before
//! installation; unsigned manifests are informational only.

use semver::Version;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// Placeholder update endpoint. Replace with the real manifest host before
/// shipping updates (see spec §"What's left to the builder").
#[allow(dead_code)]
const UPDATE_ENDPOINT: &str = "https://placeholder.chromadeck.dev/api/v1/update";

#[derive(Clone, serde::Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

/// Returns `true` when `remote` is a newer version than `current`.
pub fn is_update_available(current: &Version, remote: &Version) -> bool {
    remote > current
}

#[tauri::command]
pub async fn check_for_update_cmd(app: AppHandle) -> Result<UpdateInfo, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateInfo {
            available: true,
            version: update.version,
            notes: update.body,
            date: update.date.map(|d| d.to_string()),
        }),
        Ok(None) => Ok(UpdateInfo {
            available: false,
            version: env!("CARGO_PKG_VERSION").to_string(),
            notes: None,
            date: None,
        }),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn download_and_install_cmd(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => return Err("no update available".to_string()),
        Err(e) => return Err(e.to_string()),
    };

    let mut downloaded: u64 = 0;
    let mut total: Option<u64> = None;
    let app_clone = app.clone();

    let bytes = update
        .download(
            |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                total = total.or(content_length);
                let percent = content_length.map(|t| {
                    (downloaded as f64 / t as f64 * 100.0) as u32
                });
                let _ = app_clone.emit(
                    "update://progress",
                    serde_json::json!({
                        "downloaded": downloaded,
                        "total": total,
                        "percent": percent,
                    }),
                );
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit(
        "update://progress",
        serde_json::json!({
            "downloaded": downloaded,
            "total": total,
            "percent": 100u32,
        }),
    );

    update.install(bytes).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_available_when_remote_newer() {
        let current = Version::parse("0.8.4").unwrap();
        let remote = Version::parse("0.9.0").unwrap();
        assert!(is_update_available(&current, &remote));
    }

    #[test]
    fn no_update_when_versions_equal() {
        let current = Version::parse("0.8.4").unwrap();
        let remote = Version::parse("0.8.4").unwrap();
        assert!(!is_update_available(&current, &remote));
    }

    #[test]
    fn no_update_when_remote_older() {
        let current = Version::parse("0.9.0").unwrap();
        let remote = Version::parse("0.8.4").unwrap();
        assert!(!is_update_available(&current, &remote));
    }

    #[test]
    fn prerelease_is_not_newer_than_release() {
        let current = Version::parse("0.9.0").unwrap();
        let remote = Version::parse("0.9.0-rc.1").unwrap();
        assert!(!is_update_available(&current, &remote));
    }
}
