// ── JSON preset store + bundled ICC file management ────────────────────────
// Windows-only.  Persists presets as presets.json under %APPDATA%/ChromaDeck
// and ICC profiles as profiles/{sha256}.icc, keyed by edid_id.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

fn default_vibrance() -> f64 { 50.0 }
fn default_hue() -> f64 { 0.0 }

// ── Types ──────────────────────────────────────────────────────────────────

/// A saved colour preset for one monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub edid_id: String,
    pub icc_hash: String,
    pub icc_filename: String,
    pub brightness: f64,
    pub contrast: f64,
    pub rgb_gains: [f64; 3],
    pub gamma: f64,
    #[serde(default = "default_vibrance")]
    pub vibrance: f64, // 0–100, 50 = neutral
    #[serde(default = "default_hue")]
    pub hue_deg: f64, // 0–359 degrees
}

/// Input data for creating or updating a preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetInput {
    pub name: String,
    pub edid_id: String,
    pub icc_path: Option<String>,
    pub brightness: f64,
    pub contrast: f64,
    pub rgb_gains: [f64; 3],
    pub gamma: f64,
    pub vibrance: f64,
    pub hue_deg: f64,
}

// ── Store errors ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Json(serde_json::Error),
    NotFound(String),
    InvalidInput(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "I/O error: {e}"),
            StoreError::Json(e) => write!(f, "JSON error: {e}"),
            StoreError::NotFound(id) => write!(f, "preset not found: {id}"),
            StoreError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Json(e)
    }
}

// ── Store ──────────────────────────────────────────────────────────────────

/// Manages the preset store on disk.
pub struct Store {
    data_dir: PathBuf,
    presets_path: PathBuf,
    profiles_dir: PathBuf,
    presets: Vec<Preset>,
}

impl Store {
    /// Open (or create) the store at `data_dir`.
    pub fn new(data_dir: PathBuf) -> Result<Self, StoreError> {
        let profiles_dir = data_dir.join("profiles");
        let presets_path = data_dir.join("presets.json");

        std::fs::create_dir_all(&profiles_dir)?;

        let presets = if presets_path.exists() {
            let content = std::fs::read_to_string(&presets_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Store {
            data_dir,
            presets_path,
            profiles_dir,
            presets,
        })
    }

    /// Return all stored presets.
    pub fn list_presets(&self) -> Vec<Preset> {
        self.presets.clone()
    }

    /// Create a new preset from `input`.  Returns the created `Preset`.
    pub fn create_preset(&mut self, input: PresetInput) -> Result<Preset, StoreError> {
        // Validate
        if input.name.trim().is_empty() {
            return Err(StoreError::InvalidInput("name cannot be empty".into()));
        }
        if input.edid_id.trim().is_empty() {
            return Err(StoreError::InvalidInput("edid_id cannot be empty".into()));
        }
        if !(0.0..=1.0).contains(&input.brightness) {
            return Err(StoreError::InvalidInput(
                "brightness must be in 0.0..=1.0".into(),
            ));
        }
        if !(0.0..=1.0).contains(&input.contrast) {
            return Err(StoreError::InvalidInput(
                "contrast must be in 0.0..=1.0".into(),
            ));
        }
        if !(1.0..=3.0).contains(&input.gamma) {
            return Err(StoreError::InvalidInput(
                "gamma must be in 1.0..=3.0".into(),
            ));
        }
        if !(0.0..=100.0).contains(&input.vibrance) {
            return Err(StoreError::InvalidInput(
                "vibrance must be in 0.0..=100.0".into(),
            ));
        }
        if !(0.0..=359.0).contains(&input.hue_deg) {
            return Err(StoreError::InvalidInput(
                "hue_deg must be in 0.0..=359.0".into(),
            ));
        }
        for &g in &input.rgb_gains {
            if !(0.0..=10.0).contains(&g) {
                return Err(StoreError::InvalidInput(
                    "rgb_gains values must be in 0.0..=10.0".into(),
                ));
            }
        }

        // Handle optional ICC import
        let (icc_hash, icc_filename) = if let Some(ref src) = input.icc_path {
            let hash = self.import_icc_file(src)?;
            let filename = format!("{hash}.icc");
            (hash, filename)
        } else {
            (String::new(), String::new())
        };

        let id = uuid::Uuid::new_v4().to_string();

        let preset = Preset {
            id,
            name: input.name,
            edid_id: input.edid_id,
            icc_hash,
            icc_filename,
            brightness: input.brightness,
            contrast: input.contrast,
            rgb_gains: input.rgb_gains,
            gamma: input.gamma,
            vibrance: input.vibrance,
            hue_deg: input.hue_deg,
        };

        self.presets.push(preset.clone());
        self.flush()?;
        Ok(preset)
    }

    /// Update an existing preset identified by `id`.
    pub fn update_preset(
        &mut self,
        id: &str,
        input: PresetInput,
    ) -> Result<Preset, StoreError> {
        let idx = self
            .presets
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| StoreError::NotFound(id.into()))?;

        if input.name.trim().is_empty() {
            return Err(StoreError::InvalidInput("name cannot be empty".into()));
        }
        if !(0.0..=1.0).contains(&input.brightness) {
            return Err(StoreError::InvalidInput(
                "brightness must be in 0.0..=1.0".into(),
            ));
        }
        if !(0.0..=1.0).contains(&input.contrast) {
            return Err(StoreError::InvalidInput(
                "contrast must be in 0.0..=1.0".into(),
            ));
        }
        if !(1.0..=3.0).contains(&input.gamma) {
            return Err(StoreError::InvalidInput(
                "gamma must be in 1.0..=3.0".into(),
            ));
        }
        if !(0.0..=100.0).contains(&input.vibrance) {
            return Err(StoreError::InvalidInput(
                "vibrance must be in 0.0..=100.0".into(),
            ));
        }
        if !(0.0..=359.0).contains(&input.hue_deg) {
            return Err(StoreError::InvalidInput(
                "hue_deg must be in 0.0..=359.0".into(),
            ));
        }

        // Handle optional ICC import (new path provided) or keep existing
        let (icc_hash, icc_filename) = if let Some(ref src) = input.icc_path {
            let hash = self.import_icc_file(src)?;
            let filename = format!("{hash}.icc");
            (hash, filename)
        } else {
            (
                self.presets[idx].icc_hash.clone(),
                self.presets[idx].icc_filename.clone(),
            )
        };

        let updated = Preset {
            id: self.presets[idx].id.clone(),
            name: input.name,
            edid_id: input.edid_id,
            icc_hash,
            icc_filename,
            brightness: input.brightness,
            contrast: input.contrast,
            rgb_gains: input.rgb_gains,
            gamma: input.gamma,
            vibrance: input.vibrance,
            hue_deg: input.hue_deg,
        };

        self.presets[idx] = updated.clone();
        self.flush()?;
        Ok(updated)
    }

    /// Delete a preset by `id`.  Returns the deleted preset.
    pub fn delete_preset(&mut self, id: &str) -> Result<Preset, StoreError> {
        let idx = self
            .presets
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| StoreError::NotFound(id.into()))?;

        let removed = self.presets.remove(idx);
        self.flush()?;
        Ok(removed)
    }

    /// Import an ICC profile from `src_path`, copying it into the store and
    /// returning its SHA‑256 hex hash.
    pub fn import_icc(&mut self, src_path: &str) -> Result<String, StoreError> {
        self.import_icc_file(src_path)
    }

    /// Internal: copy + hash an ICC file.
    fn import_icc_file(&self, src_path: &str) -> Result<String, StoreError> {
        let src = PathBuf::from(src_path);

        if !src.exists() {
            return Err(StoreError::InvalidInput(format!(
                "ICC file not found: {src_path}"
            )));
        }
        if src.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
            return Err(StoreError::InvalidInput(format!(
                "ICC file is empty: {src_path}"
            )));
        }

        let data = std::fs::read(&src)?;
        use sha2::{Digest, Sha256};
        let hash = hex::encode(Sha256::digest(&data));

        let dest = self.profiles_dir.join(format!("{hash}.icc"));
        if !dest.exists() {
            std::fs::copy(&src, &dest)?;
        }

        Ok(hash)
    }

    /// Persist the in-memory preset list to disk atomically.
    fn flush(&self) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(&self.presets)?;
        let tmp = self.data_dir.join("presets.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.presets_path)?;
        Ok(())
    }
}

// ── Default app data directory ─────────────────────────────────────────────

/// Return the default store path: `%APPDATA%/ChromaDeck`.
pub fn default_store_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ChromaDeck")
}

// ── Tauri commands ────────────────────────────────────────────────────────

use std::sync::Mutex;

/// Wrapper for Tauri-managed state (Mutex for thread safety).
pub struct AppStore(pub Mutex<Store>);

#[tauri::command]
pub fn list_presets_cmd(state: tauri::State<'_, AppStore>) -> Result<Vec<Preset>, String> {
    let store = state.0.lock().map_err(|e| e.to_string())?;
    Ok(store.list_presets())
}

#[tauri::command]
pub fn create_preset_cmd(
    state: tauri::State<'_, AppStore>,
    input: PresetInput,
) -> Result<Preset, String> {
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    store.create_preset(input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_preset_cmd(
    state: tauri::State<'_, AppStore>,
    id: String,
    input: PresetInput,
) -> Result<Preset, String> {
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    store.update_preset(&id, input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_preset_cmd(
    state: tauri::State<'_, AppStore>,
    id: String,
) -> Result<Preset, String> {
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    store.delete_preset(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_icc_cmd(
    state: tauri::State<'_, AppStore>,
    src_path: String,
) -> Result<String, String> {
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    store.import_icc(&src_path).map_err(|e| e.to_string())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a Store backed by a unique temp directory.
    fn test_store() -> Store {
        let dir = std::env::temp_dir().join(format!(
            "chromadeck_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Store::new(dir).unwrap()
    }

    /// Helper to create a minimal valid PresetInput.
    fn minimal_input() -> PresetInput {
        PresetInput {
            name: "Test Preset".into(),
            edid_id: "EDID-001".into(),
            icc_path: None,
            brightness: 0.5,
            contrast: 0.8,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 2.2,
            vibrance: 50.0,
            hue_deg: 0.0,
        }
    }

    // ── create_and_list_preset ─────────────────────────────────────────────

    #[test]
    fn create_and_list_preset() {
        let mut store = test_store();

        let preset = store
            .create_preset(minimal_input())
            .expect("create should succeed");
        assert_eq!(preset.name, "Test Preset");
        assert_eq!(preset.edid_id, "EDID-001");
        assert!(!preset.id.is_empty(), "preset must have a UUID id");
        assert!(preset.icc_hash.is_empty(), "no ICC imported");
        assert!(preset.icc_filename.is_empty());

        let list = store.list_presets();
        assert_eq!(list.len(), 1, "should have exactly 1 preset");
        assert_eq!(list[0].name, "Test Preset");
    }

    // ── create_preset validates required fields ────────────────────────────

    #[test]
    fn create_preset_rejects_empty_name() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.name = String::new();
        let err = store.create_preset(input).unwrap_err();
        assert!(
            err.to_string().contains("name"),
            "error should mention name: {}",
            err
        );
    }

    #[test]
    fn create_preset_rejects_bad_brightness() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.brightness = 1.5;
        let err = store.create_preset(input).unwrap_err();
        assert!(err.to_string().contains("brightness"));
    }

    #[test]
    fn create_preset_rejects_bad_gamma() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.gamma = 0.5;
        let err = store.create_preset(input).unwrap_err();
        assert!(err.to_string().contains("gamma"));
    }

    // ── update_preset ──────────────────────────────────────────────────────

    #[test]
    fn update_preset_changes_fields() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();

        let updated = store
            .update_preset(
                &preset.id,
                PresetInput {
                    name: "Updated".into(),
                    edid_id: preset.edid_id.clone(),
                    icc_path: None,
                    brightness: 0.9,
                    contrast: 0.3,
                    rgb_gains: [0.8, 0.9, 1.0],
                    gamma: 2.0,
                    vibrance: 50.0,
                    hue_deg: 0.0,
                },
            )
            .unwrap();

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.id, preset.id);
        assert_eq!(updated.brightness, 0.9);
    }

    #[test]
    fn update_preset_not_found() {
        let mut store = test_store();
        let err = store
            .update_preset("nonexistent", minimal_input())
            .unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
    }

    // ── delete_preset ──────────────────────────────────────────────────────

    #[test]
    fn delete_preset_removes_it() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        assert_eq!(store.list_presets().len(), 1);

        let deleted = store.delete_preset(&preset.id).unwrap();
        assert_eq!(deleted.id, preset.id);
        assert_eq!(store.list_presets().len(), 0);
    }

    #[test]
    fn delete_preset_not_found() {
        let mut store = test_store();
        let err = store.delete_preset("nonexistent").unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
    }

    // ── import_icc copy + hash ─────────────────────────────────────────────

    #[test]
    fn import_icc_copies_and_hashes() {
        let dir = std::env::temp_dir().join(format!(
            "chromadeck_icc_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = Store::new(dir.clone()).unwrap();

        // Create a fake ICC file outside the store
        let src = dir.join("source.icc");
        let content = b"fake_icc_profile_data_123";
        std::fs::write(&src, content).unwrap();

        let hash = store
            .import_icc(src.to_str().unwrap())
            .expect("import should succeed");

        // SHA-256 of the content
        use sha2::{Digest, Sha256};
        let expected_hash = hex::encode(Sha256::digest(content));
        assert_eq!(hash, expected_hash);

        // File should exist in profiles/
        let dest = dir.join("profiles").join(format!("{hash}.icc"));
        assert!(dest.exists(), "ICC must be copied into profiles dir");
        assert_eq!(std::fs::read(&dest).unwrap(), content);
    }

    #[test]
    fn import_icc_rejects_nonexistent() {
        let mut store = test_store();
        let err = store
            .import_icc("C:\\nonexistent\\file.icc")
            .unwrap_err();
        assert!(matches!(err, StoreError::InvalidInput(_)));
    }

    #[test]
    fn import_icc_rejects_empty_file() {
        let mut store = test_store();
        let empty_path = store.data_dir.join("empty.icc");
        std::fs::write(&empty_path, b"").unwrap();
        let err = store
            .import_icc(empty_path.to_str().unwrap())
            .unwrap_err();
        assert!(matches!(err, StoreError::InvalidInput(_)));
    }

    // ── Persistence (export / import round-trip) ───────────────────────────

    #[test]
    fn presist_presets_across_reload() {
        let dir = std::env::temp_dir().join(format!(
            "chromadeck_persist_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        // First session
        {
            let mut store = Store::new(dir.clone()).unwrap();
            let _p1 = store.create_preset(minimal_input()).unwrap();
            let _p2 = store
                .create_preset(PresetInput {
                    name: "Second".into(),
                    edid_id: "EDID-002".into(),
                    icc_path: None,
                    brightness: 0.3,
                    contrast: 0.6,
                    rgb_gains: [0.5, 0.5, 0.5],
                    gamma: 2.5,
                    vibrance: 50.0,
                    hue_deg: 0.0,
                })
                .unwrap();
            assert_eq!(store.list_presets().len(), 2);
        }

        // Second session — reload from disk
        {
            let store = Store::new(dir.clone()).unwrap();
            let list = store.list_presets();
            assert_eq!(list.len(), 2);
            assert_eq!(list[0].name, "Test Preset");
            assert_eq!(list[1].name, "Second");
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── nvapi fields: vibrance + hue_deg ─────────────────────────────────────

    #[test]
    fn old_json_without_nvapi_fields_gets_neutral_defaults() {
        let json = r#"{"id":"x","name":"Old","edid_id":"E","icc_hash":"","icc_filename":"","brightness":0.5,"contrast":0.5,"rgb_gains":[1.0,1.0,1.0],"gamma":2.2}"#;
        let preset: Preset = serde_json::from_str(json).unwrap();
        assert_eq!(preset.vibrance, 50.0);
        assert_eq!(preset.hue_deg, 0.0);
    }

    #[test]
    fn create_preset_rejects_bad_vibrance() {
        let mut input = minimal_input();
        input.vibrance = 101.0;
        let err = test_store().create_preset(input).unwrap_err();
        assert!(err.to_string().contains("vibrance"));
    }

    #[test]
    fn create_preset_rejects_bad_hue() {
        let mut input = minimal_input();
        input.hue_deg = 360.0;
        let err = test_store().create_preset(input).unwrap_err();
        assert!(err.to_string().contains("hue_deg"));
    }

    #[test]
    fn create_preset_carries_vibrance_and_hue() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.vibrance = 75.0;
        input.hue_deg = 120.0;
        let preset = store.create_preset(input).unwrap();
        assert_eq!(preset.vibrance, 75.0);
        assert_eq!(preset.hue_deg, 120.0);
    }
}