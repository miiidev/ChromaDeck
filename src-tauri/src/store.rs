// ── JSON preset store + bundled ICC file management ────────────────────────
// Windows-only.  Persists presets as presets.json under %APPDATA%/ChromaDeck
// and ICC profiles as profiles/{sha256}.icc, keyed by edid_id.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

fn default_vibrance() -> f64 { 50.0 }
fn default_hue() -> f64 { 0.0 }
fn legacy_model() -> String { "gain-v1".into() }

/// Validate a color_tag value: must be empty or #rrggbb hex.
fn is_valid_color_tag(tag: &str) -> bool {
    if tag.is_empty() {
        return true;
    }
    if tag.len() != 7 || !tag.starts_with("#") {
        return false;
    }
    for c in tag[1..].chars() {
        if !matches!(c, '0'..='9' | 'a'..='f' | 'A'..='F') {
            return false;
        }
    }
    true
}

/// Produce a local-date timestamp string `yyyymmdd-HHMMSS` from SystemTime.
fn local_timestamp() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("SystemTime went backwards");
    let total_secs = dur.as_secs();
    let days = total_secs / 86400;
    let secs_today = total_secs % 86400;
    let h = secs_today / 3600;
    let m = (secs_today % 3600) / 60;
    let s = secs_today % 60;

    let mut y = 1970i64;
    let mut rem = days as i64;
    loop {
        let diy = if is_leap(y) { 366 } else { 365 };
        if rem < diy {
            break;
        }
        rem -= diy;
        y += 1;
    }
    let mon_lengths = [31, if is_leap(y) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u32;
    for &ml in &mon_lengths {
        if rem < ml as i64 {
            break;
        }
        rem -= ml as i64;
        mo += 1;
    }
    let d = rem + 1;
    format!("{y:04}{mo:02}{d:02}-{h:02}{m:02}{s:02}")
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ── Types ──────────────────────────────────────────────────────────────────

/// A saved colour preset (global, not tied to one monitor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
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
    #[serde(default = "legacy_model")]
    pub color_model: String,
    #[serde(default)]
    pub color_tag: Option<String>, // #rrggbb or None for untagged
}

/// Input data for creating or updating a preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetInput {
    pub name: String,
    pub icc_path: Option<String>,
    pub brightness: f64,
    pub contrast: f64,
    pub rgb_gains: [f64; 3],
    pub gamma: f64,
    pub vibrance: f64,
    pub hue_deg: f64,
    #[serde(default)]
    pub color_tag: Option<String>,
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

/// A preset as previously seeded on first run: exactly "Standard" with
/// untouched neutral values (no ICC). Only such a pristine preset is
/// removed by the standard-seed purge below — a user-tweaked or
/// user-created "Standard" keeps its pins and applied records.
fn is_neutral_standard(p: &Preset) -> bool {
    p.name == "Standard"
        && p.brightness == 50.0
        && p.contrast == 50.0
        && p.gamma == 1.0
        && p.rgb_gains == [1.0, 1.0, 1.0]
        && p.vibrance == 50.0
        && p.hue_deg == 0.0
        && p.icc_hash.is_empty()
}

/// Manages the preset store on disk.
pub struct Store {
    data_dir: PathBuf,
    presets_path: PathBuf,
    profiles_dir: PathBuf,
    presets: Vec<Preset>,
    pins_path: PathBuf,
    pinned: HashMap<String, String>, // edid_id -> preset_id
    applied_path: PathBuf,
    applied: HashMap<String, String>, // edid_id -> preset_id (last manual apply; not verified)
    names_path: PathBuf,
    monitor_names: HashMap<String, String>, // edid_id -> user alias
}

impl Store {
    /// Open (or create) the store at `data_dir`.
    pub fn new(data_dir: PathBuf) -> Result<Self, StoreError> {
        let profiles_dir = data_dir.join("profiles");
        let presets_path = data_dir.join("presets.json");

        std::fs::create_dir_all(&profiles_dir)?;

        let presets: Vec<Preset> = if presets_path.exists() {
            let content = std::fs::read_to_string(&presets_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        let pins_path = data_dir.join("pins.json");
        let pinned: HashMap<String, String> = if pins_path.exists() {
            let content = std::fs::read_to_string(&pins_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        let names_path = data_dir.join("monitor_names.json");
        let monitor_names: HashMap<String, String> = if names_path.exists() {
            let content = std::fs::read_to_string(&names_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        let applied_path = data_dir.join("applied.json");
        // ── Stale-record honesty ─────────────────────────────────────────────
        // The applied map is a session-recency hint: it records the last
        // manually applied preset per monitor from the same process lifetime.
        // Unlike pins, it is NOT verified against hardware — an unpinned record
        // can over-claim if something external (game, HDR toggle, reboot) clears
        // the gamma LUT or NVAPI registers. Hardware capture-and-compare is a
        // separate future build; pins + the enforce loop remain the definitive
        // source of truth for guaranteed state.
        let applied: HashMap<String, String> = if applied_path.exists() {
            let content = std::fs::read_to_string(&applied_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        // ── Legacy migration: NVCP-scale preset upgrade ─────────────────────
        let mut store = Store {
            data_dir: data_dir.clone(),
            presets_path: presets_path.clone(),
            profiles_dir,
            presets,
            pins_path,
            pinned,
            applied_path,
            applied,
            names_path,
            monitor_names,
        };

        // ── Monitor-enumeration generation wipe (one-shot, user-approved)
        // First run under real-monitor identities: back up legacy data,
        // then start empty. Sentinel makes it exactly-once.
        if !data_dir.join(".monitor-enumeration-v2").exists() {
            if !store.presets.is_empty() {
                let bak = format!("presets.json.bak-{}", local_timestamp());
                let _ = std::fs::copy(&store.presets_path, store.data_dir.join(&bak));
                store.presets = Vec::new();
                store.flush()?;
            }
            if !store.pinned.is_empty() {
                let bak = format!("pins.json.bak-{}", local_timestamp());
                let _ = std::fs::copy(&store.pins_path, store.data_dir.join(&bak));
                store.pinned = HashMap::new();
                store.flush_pins()?;
            }
            let _ = std::fs::write(data_dir.join(".monitor-enumeration-v2"), b"");
        }

        let needs_migration = store.presets.iter().any(|p| p.color_model == "gain-v1");
        if needs_migration {
            // Backup with local-timestamped filename
            let bak_name = format!("presets.json.bak-{}", local_timestamp());
            let bak_path = data_dir.join(&bak_name);
            let _ = std::fs::copy(&presets_path, &bak_path);
            // Migrate
            for p in &mut store.presets {
                if p.color_model == "gain-v1" {
                    p.brightness = 50.0;
                    p.contrast = 50.0;
                    p.color_model = "nvcp-v1".into();
                }
            }
            store.flush()?;
        }

        // ── Global-presets migration: strip edid_id from stored presets ─────
        // serde silently ignores the edid_id field in old data, so this
        // migration re-serialises every preset to drop the field from disk.
        if !data_dir.join(".global-presets-v1").exists() {
            if !store.presets.is_empty() {
                let bak = format!("presets.json.bak-{}", local_timestamp());
                let _ = std::fs::copy(&store.presets_path, store.data_dir.join(&bak));
                store.flush()?;
            }
            // Also clean pin ownership guard — pins are now just edid→preset
            // without cross-checking preset.edid_id (handled in pin_preset).
            let _ = std::fs::write(data_dir.join(".global-presets-v1"), b"");
        }

        // ── Seeded-Standard purge (one-shot) ───────────────────────────
        // Early builds created a neutral "Standard" preset on first launch
        // and (for a time) auto-pinned/applied it, so it stayed IN USE on
        // every connected monitor. Standard is no longer seeded: a fresh
        // library starts empty and the user authors every preset. This
        // repairs existing installs: any still-pristine neutral Standard is
        // removed (delete_preset cascades pins + applied records + flushes).
        // Anything the user renamed, retuned, or created themselves is
        // untouched. Sentinel makes it exactly-once; an obsolete
        // .standard-unpin-v1 file left on disk is harmless.
        if !data_dir.join(".standard-seed-purge-v1").exists() {
            let standard_ids: Vec<String> = store
                .presets
                .iter()
                .filter(|p| is_neutral_standard(p))
                .map(|p| p.id.clone())
                .collect();
            for id in &standard_ids {
                let _ = store.delete_preset(id)?;
            }
            let _ = std::fs::write(data_dir.join(".standard-seed-purge-v1"), b"");
        }

        Ok(store)
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
        if !(0.0..=100.0).contains(&input.brightness) {
            return Err(StoreError::InvalidInput(
                "brightness must be in 0.0..=100.0".into(),
            ));
        }
        if !(0.0..=100.0).contains(&input.contrast) {
            return Err(StoreError::InvalidInput(
                "contrast must be in 0.0..=100.0".into(),
            ));
        }
        if !(0.3..=2.8).contains(&input.gamma) {
            return Err(StoreError::InvalidInput(
                "gamma must be in 0.3..=2.8".into(),
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

        // Validate color_tag
        if let Some(ref tag) = input.color_tag {
            if !is_valid_color_tag(tag) {
                return Err(StoreError::InvalidInput(
                    "color_tag must be a hex color like #rrggbb or empty".into(),
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
            icc_hash,
            icc_filename,
            brightness: input.brightness,
            contrast: input.contrast,
            rgb_gains: input.rgb_gains,
            gamma: input.gamma,
            vibrance: input.vibrance,
            hue_deg: input.hue_deg,
            color_model: "nvcp-v1".into(),
            color_tag: if let Some(tag) = input.color_tag {
                if tag.is_empty() { None } else { Some(tag.into()) }
            } else { None },
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
        if !(0.0..=100.0).contains(&input.brightness) {
            return Err(StoreError::InvalidInput(
                "brightness must be in 0.0..=100.0".into(),
            ));
        }
        if !(0.0..=100.0).contains(&input.contrast) {
            return Err(StoreError::InvalidInput(
                "contrast must be in 0.0..=100.0".into(),
            ));
        }
        if !(0.3..=2.8).contains(&input.gamma) {
            return Err(StoreError::InvalidInput(
                "gamma must be in 0.3..=2.8".into(),
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

        // Validate color_tag
        if let Some(ref tag) = input.color_tag {
            if !is_valid_color_tag(tag) {
                return Err(StoreError::InvalidInput(
                    "color_tag must be a hex color like #rrggbb or empty".into(),
                ));
            }
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
            icc_hash,
            icc_filename,
            brightness: input.brightness,
            contrast: input.contrast,
            rgb_gains: input.rgb_gains,
            gamma: input.gamma,
            vibrance: input.vibrance,
            hue_deg: input.hue_deg,
            color_model: self.presets[idx].color_model.clone(),
            color_tag: if let Some(tag) = input.color_tag {
                if tag.is_empty() { None } else { Some(tag.into()) }
            } else { None },
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
        let pinned_gone = self.pinned.values().any(|v| v == id);
        if pinned_gone {
            self.pinned.retain(|_, v| v != id);
        }
        let applied_gone = self.applied.values().any(|v| v == id);
        if applied_gone {
            self.applied.retain(|_, v| v != id);
        }
        self.flush()?;
        if pinned_gone {
            self.flush_pins()?;
        }
        if applied_gone {
            self.flush_applied()?;
        }
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

    /// Pin a preset to a monitor. Returns error when preset does not exist.
    pub fn pin_preset(&mut self, edid_id: &str, preset_id: &str) -> Result<(), StoreError> {
        let _ = self
            .presets
            .iter()
            .find(|p| p.id == preset_id)
            .ok_or_else(|| StoreError::NotFound(preset_id.into()))?;
        self.pinned.insert(edid_id.into(), preset_id.into());
        self.flush_pins()
    }

    /// Remove the pin for a monitor (no-op when absent).
    pub fn unpin_monitor(&mut self, edid_id: &str) {
        if self.pinned.remove(edid_id).is_some() {
            let _ = self.flush_pins();
        }
    }

    /// Return a copy of the current pin map (edid_id -> preset_id).
    pub fn list_pins(&self) -> HashMap<String, String> {
        self.pinned.clone()
    }

    /// Set (or clear, when the trimmed alias is empty) a monitor's display
    /// name. Rejects aliases longer than 64 characters.
    pub fn set_monitor_name(&mut self, edid_id: &str, alias: &str) -> Result<(), StoreError> {
        let trimmed = alias.trim();
        if trimmed.chars().count() > 64 {
            return Err(StoreError::InvalidInput(
                "monitor name must be 64 characters or fewer".into(),
            ));
        }
        if trimmed.is_empty() {
            if self.monitor_names.remove(edid_id).is_some() {
                let _ = self.flush_names();
            }
            return Ok(());
        }
        self.monitor_names.insert(edid_id.into(), trimmed.into());
        self.flush_names()
    }

    /// Return a copy of the monitor alias map (edid_id -> alias).
    pub fn list_monitor_names(&self) -> HashMap<String, String> {
        self.monitor_names.clone()
    }

    /// Record a manual preset application for a monitor.
    /// Flushes to applied.json atomically.
    pub fn record_applied(&mut self, edid_id: &str, preset_id: &str) -> Result<(), StoreError> {
        self.applied.insert(edid_id.into(), preset_id.into());
        self.flush_applied()
    }

    /// Clear the applied record for a monitor (e.g. after reset).
    /// No-op when no record exists. Flushes only on change.
    pub fn clear_applied(&mut self, edid_id: &str) {
        if self.applied.remove(edid_id).is_some() {
            let _ = self.flush_applied();
        }
    }

    /// Return a copy of the applied map (edid_id -> preset_id).
    pub fn list_applied(&self) -> HashMap<String, String> {
        self.applied.clone()
    }

    /// Atomically persist the applied map to `applied.json`.
    fn flush_applied(&self) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(&self.applied)?;
        let tmp = self.data_dir.join("applied.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.applied_path)?;
        Ok(())
    }

    /// Atomically persist the alias map to `monitor_names.json`.
    fn flush_names(&self) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(&self.monitor_names)?;
        let tmp = self.data_dir.join("monitor_names.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.names_path)?;
        Ok(())
    }

    /// Atomically persist the pin map to `pins.json`.
    fn flush_pins(&self) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(&self.pinned)?;
        let tmp = self.data_dir.join("pins.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.pins_path)?;
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

#[tauri::command]
pub fn pin_preset_cmd(
    state: tauri::State<'_, AppStore>,
    edid_id: String,
    preset_id: String,
) -> Result<(), String> {
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    store.pin_preset(&edid_id, &preset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn unpin_monitor_cmd(state: tauri::State<'_, AppStore>, edid_id: String) {
    if let Ok(mut store) = state.0.lock() {
        store.unpin_monitor(&edid_id);
    }
}

#[tauri::command]
pub fn list_pins_cmd(
    state: tauri::State<'_, AppStore>,
) -> Result<HashMap<String, String>, String> {
    let store = state.0.lock().map_err(|e| e.to_string())?;
    Ok(store.list_pins())
}

#[tauri::command]
pub fn list_applied_cmd(
    state: tauri::State<'_, AppStore>,
) -> Result<HashMap<String, String>, String> {
    let store = state.0.lock().map_err(|e| e.to_string())?;
    Ok(store.list_applied())
}

#[tauri::command]
pub fn set_monitor_name_cmd(
    state: tauri::State<'_, AppStore>,
    edid_id: String,
    alias: String,
) -> Result<(), String> {
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    store.set_monitor_name(&edid_id, &alias).map_err(|e| e.to_string())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a Store backed by a unique temp directory.
    fn test_store() -> Store {
        test_store_at(test_store_dir_unique())
    }

    /// Create a Store at a specific directory (cleaning any leftovers first).
    /// Pre-writes an empty presets.json so Store::new starts from a known
    /// empty library.
    fn test_store_at(dir: PathBuf) -> Store {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("presets.json"), b"[]").unwrap();
        Store::new(dir).unwrap()
    }

    /// Shared temp directory for persistence tests (fixed name, cleaned).
    fn test_store_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("chromadeck_test_shared");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Helper to create a minimal valid PresetInput (NVCP-scale values).
    fn minimal_input() -> PresetInput {
        PresetInput {
            name: "Test Preset".into(),
            icc_path: None,
            brightness: 55.0,
            contrast: 60.0,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 2.2,
            vibrance: 50.0,
            hue_deg: 0.0,
            color_tag: None,
        }
    }

    /// Create a Store backed by a unique temp directory (returns the dir).
    /// Uniqueness is a process-wide atomic counter, not the wall clock:
    /// Windows clock granularity is coarse, so two tests starting in the
    /// same tick would otherwise share a directory and race.
    static TEST_DIR_COUNTER: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);

    fn test_store_dir_unique() -> PathBuf {
        let n = TEST_DIR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "chromadeck_test_{}_{}_{}",
            std::process::id(),
            n,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    // ── create_and_list_preset ─────────────────────────────────────────────

    #[test]
    fn create_and_list_preset() {
        let mut store = test_store();

        let preset = store
            .create_preset(minimal_input())
            .expect("create should succeed");
        assert_eq!(preset.name, "Test Preset");
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
        input.brightness = 101.0;
        let err = store.create_preset(input).unwrap_err();
        assert!(err.to_string().contains("brightness"));
    }

    #[test]
    fn create_preset_rejects_bad_gamma() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.gamma = 0.2;
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
                    icc_path: None,
                    brightness: 55.0,
                    contrast: 60.0,
                    rgb_gains: [0.8, 0.9, 1.0],
                    gamma: 2.0,
                    vibrance: 50.0,
                    hue_deg: 0.0,
                    color_tag: None,
                },
            )
            .unwrap();

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.id, preset.id);
        assert_eq!(updated.brightness, 55.0);
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
        let dir = test_store_dir_unique();
        let _ = std::fs::remove_dir_all(&dir);

        // First session: fresh dir starts empty, creates 2 = 2 total
        {
            let mut store = Store::new(dir.clone()).unwrap();
            let _p1 = store.create_preset(minimal_input()).unwrap();
            let _p2 = store
                .create_preset(PresetInput {
                    name: "Second".into(),
                    icc_path: None,
                    brightness: 30.0,
                    contrast: 60.0,
                    rgb_gains: [0.5, 0.5, 0.5],
                    gamma: 2.5,
                    vibrance: 50.0,
                    hue_deg: 0.0,
                    color_tag: None,
                })
                .unwrap();
            assert_eq!(store.list_presets().len(), 2);
        }

        // Second session — reload from disk (2 presets)
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
        let json = r#"{"id":"x","name":"Old","icc_hash":"","icc_filename":"","brightness":0.5,"contrast":0.5,"rgb_gains":[1.0,1.0,1.0],"gamma":2.2}"#;
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

    // ── color_tag ──────────────────────────────────────────────────────────

    #[test]
    fn color_tag_defaults_to_none() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        assert!(preset.color_tag.is_none(), "untagged preset has no color_tag");
    }

    #[test]
    fn color_tag_store_and_roundtrip() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.color_tag = Some("#06b6d4".into());
        let preset = store.create_preset(input).unwrap();
        assert_eq!(preset.color_tag, Some("#06b6d4".to_string()));
    }

    #[test]
    fn color_tag_empty_string_normalized_to_none() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.color_tag = Some("".into());
        let preset = store.create_preset(input).unwrap();
        assert!(preset.color_tag.is_none(), "empty string color_tag becomes None");
    }

    #[test]
    fn color_tag_rejects_bad_format() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.color_tag = Some("not-a-color".into());
        let err = store.create_preset(input).unwrap_err();
        assert!(err.to_string().contains("color_tag"), "error mentions color_tag");
    }

    #[test]
    fn color_tag_rejects_short_hex() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.color_tag = Some("#fff".into());
        let err = store.create_preset(input).unwrap_err();
        assert!(err.to_string().contains("color_tag"));
    }

    #[test]
    fn color_tag_loaded_from_old_json_without_field() {
        let json = r#"{"id":"x","name":"Old","icc_hash":"","icc_filename":"","brightness":55.0,"contrast":60.0,"rgb_gains":[1.0,1.0,1.0],"gamma":2.2,"vibrance":50.0,"hue_deg":0.0,"color_model":"nvcp-v1"}"#;
        let preset: Preset = serde_json::from_str(json).unwrap();
        assert!(preset.color_tag.is_none(), "old JSON without color_tag loads as None");
    }

    #[test]
    fn color_tag_update_changes_value() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        assert!(preset.color_tag.is_none());

        let updated = store
            .update_preset(
                &preset.id,
                PresetInput {
                    name: "Tagged".into(),
                    icc_path: None,
                    brightness: 55.0,
                    contrast: 60.0,
                    rgb_gains: [1.0, 1.0, 1.0],
                    gamma: 2.2,
                    vibrance: 50.0,
                    hue_deg: 0.0,
                    color_tag: Some("#ef4444".into()),
                },
            )
            .unwrap();
        assert_eq!(updated.color_tag, Some("#ef4444".to_string()));
    }

    #[test]
    fn color_tag_update_clears_tag() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.color_tag = Some("#06b6d4".into());
        let preset = store.create_preset(input).unwrap();
        assert_eq!(preset.color_tag, Some("#06b6d4".to_string()));

        let updated = store
            .update_preset(
                &preset.id,
                PresetInput {
                    name: "Cleared".into(),
                    icc_path: None,
                    brightness: 55.0,
                    contrast: 60.0,
                    rgb_gains: [1.0, 1.0, 1.0],
                    gamma: 2.2,
                    vibrance: 50.0,
                    hue_deg: 0.0,
                    color_tag: Some("".into()),
                },
            )
            .unwrap();
        assert!(updated.color_tag.is_none(), "clearing color_tag produces None");
    }

    #[test]
    fn color_tag_update_rejects_bad_value() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();

        let err = store
            .update_preset(
                &preset.id,
                PresetInput {
                    name: "Bad".into(),
                    icc_path: None,
                    brightness: 55.0,
                    contrast: 60.0,
                    rgb_gains: [1.0, 1.0, 1.0],
                    gamma: 2.2,
                    vibrance: 50.0,
                    hue_deg: 0.0,
                    color_tag: Some("invalid".into()),
                },
            )
            .unwrap_err();
        assert!(err.to_string().contains("color_tag"));
    }

    // ── color_model / migration ──────────────────────────────────────────────

    #[test]
    fn legacy_file_migrates_brightness_contrast_to_neutral_with_backup() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("presets.json"),
            r#"[{"id":"a","name":"Old","edid_id":"E","icc_hash":"","icc_filename":"","brightness":0.55,"contrast":0.5,"rgb_gains":[1.0,1.0,1.0],"gamma":1.25,"vibrance":100.0,"hue_deg":0.0}]"#,
        )
        .unwrap();
        // Sentinel present → skip the v2 wipe so migration can be tested.
        std::fs::write(dir.join(".monitor-enumeration-v2"), b"").unwrap();
        let store = Store::new(dir.clone()).unwrap();
        let p = &store.list_presets()[0];
        assert_eq!((p.brightness, p.contrast), (50.0, 50.0));
        assert_eq!((p.gamma, p.vibrance), (1.25, 100.0)); // untouched
        assert!(std::fs::read_dir(&dir).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("presets.json.bak-")));
    }

    #[test]
    fn create_rejects_brightness_above_100() {
        let mut input = minimal_input();
        input.brightness = 101.0;
        let err = test_store().create_preset(input).unwrap_err();
        assert!(err.to_string().contains("brightness"));
    }

    #[test]
    fn create_stamps_nvcp_model() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        assert_eq!(preset.color_model, "nvcp-v1");
    }

    // ── pin/unpin pins ──────────────────────────────────────────────────────

    #[test]
    fn pin_and_unpin_roundtrip() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.pin_preset("EDID-001", &preset.id).unwrap();
        assert_eq!(store.list_pins().get("EDID-001"), Some(&preset.id));
        store.unpin_monitor("EDID-001");
        assert!(store.list_pins().is_empty());
    }

    #[test]
    fn pin_rejects_unknown_preset() {
        let mut store = test_store();
        let err = store.pin_preset("EDID-001", "nope").unwrap_err();
        assert!(err.to_string().contains("preset not found"));
    }

    #[test]
    fn pin_allows_cross_monitor() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap(); // global preset
        // Cross-monitor pin is now allowed since presets are global
        store.pin_preset("EDID-999", &preset.id).unwrap();
        assert_eq!(store.list_pins().get("EDID-999"), Some(&preset.id));
    }

    #[test]
    fn delete_cascades_pin() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.pin_preset("EDID-001", &preset.id).unwrap();
        store.delete_preset(&preset.id).unwrap();
        assert!(store.list_pins().is_empty());
    }

    #[test]
    fn pins_persist_across_reopen() {
        let dir = test_store_dir();
        let mut store = Store::new(dir.clone()).unwrap();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.pin_preset("EDID-001", &preset.id).unwrap();
        drop(store);
        let reopened = Store::new(dir).unwrap();
        assert_eq!(reopened.list_pins().get("EDID-001"), Some(&preset.id));
    }

    // ── applied record (persistent last-manual-apply hint) ──────────────

    #[test]
    fn applied_record_flush_and_reload() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.record_applied("EDID-001", &preset.id).unwrap();
        assert_eq!(
            store.list_applied().get("EDID-001"),
            Some(&preset.id),
        );
        let list = store.list_applied();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn applied_clear_removes_entry() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.record_applied("EDID-001", &preset.id).unwrap();
        assert!(!store.list_applied().is_empty());
        store.clear_applied("EDID-001");
        assert!(store.list_applied().is_empty());
    }

    #[test]
    fn applied_persists_across_reopen() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("presets.json"), b"[]").unwrap();
        let mut store = Store::new(dir.clone()).unwrap();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.record_applied("EDID-001", &preset.id).unwrap();
        drop(store);
        let reopened = Store::new(dir).unwrap();
        assert_eq!(
            reopened.list_applied().get("EDID-001"),
            Some(&preset.id),
        );
    }

    #[test]
    fn delete_cascades_applied() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.record_applied("EDID-001", &preset.id).unwrap();
        store.delete_preset(&preset.id).unwrap();
        assert!(store.list_applied().is_empty());
    }

    #[test]
    fn malformed_applied_defaults_empty() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("applied.json"),
            br#"this is not json at all"#,
        )
        .unwrap();
        std::fs::write(dir.join("presets.json"), b"[]").unwrap();
        std::fs::write(dir.join(".monitor-enumeration-v2"), b"").unwrap();
        let store = Store::new(dir).unwrap();
        assert!(store.list_applied().is_empty());
    }

    // ── monitor-enumeration wipe (backup + sentinel) ──────────────────────

    #[test]
    fn first_run_backs_up_and_wipes_legacy_data() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("presets.json"),
            r#"[{"id":"a","name":"Old","edid_id":"PCI\\VEN_10DE&DEV_1","icc_hash":"","icc_filename":"","brightness":55.0,"contrast":60.0,"rgb_gains":[1.0,1.0,1.0],"gamma":1.0,"vibrance":100.0,"hue_deg":0.0,"color_model":"nvcp-v1"}]"#,
        )
        .unwrap();
        std::fs::write(dir.join("pins.json"), r#"{"PCI\\VEN_X":"a"}"#).unwrap();
        let store = Store::new(dir.clone()).unwrap();
        assert!(store.list_presets().is_empty());
        assert!(store.list_pins().is_empty());
        let mut bak_presets = false;
        let mut bak_pins = false;
        for e in std::fs::read_dir(&dir).unwrap() {
            let n = e.unwrap().file_name().to_string_lossy().into_owned();
            if n.starts_with("presets.json.bak-") {
                bak_presets = true;
            }
            if n.starts_with("pins.json.bak-") {
                bak_pins = true;
            }
        }
        assert!(bak_presets && bak_pins, "both backups must exist");
        assert!(dir.join(".monitor-enumeration-v2").exists());
        // Second open: sentinel respected, no duplicate wipe activity.
        let store2 = Store::new(dir).unwrap();
        assert!(store2.list_presets().is_empty());
    }

    // ── monitor display names ─────────────────────────────────────────────

    #[test]
    fn monitor_name_set_get_and_clear() {
        let mut store = test_store();
        assert!(store.list_monitor_names().is_empty());
        store.set_monitor_name("EDID-001", "Main").unwrap();
        assert_eq!(
            store.list_monitor_names().get("EDID-001"),
            Some(&"Main".to_string())
        );
        // Empty (or whitespace) clears back to default.
        store.set_monitor_name("EDID-001", "   ").unwrap();
        assert!(store.list_monitor_names().is_empty());
    }

    #[test]
    fn monitor_name_rejects_overlong_alias() {
        let mut store = test_store();
        let long = "x".repeat(65);
        let err = store.set_monitor_name("EDID-001", &long).unwrap_err();
        assert!(err.to_string().contains("64 characters"));
        assert!(store.list_monitor_names().is_empty());
    }

    #[test]
    fn monitor_names_persist_across_reopen() {
        let dir = test_store_dir_unique();
        let mut store = test_store_at(dir.clone());
        store.set_monitor_name("EDID-001", "Main").unwrap();
        drop(store);
        let reopened = Store::new(dir).unwrap();
        assert_eq!(
            reopened.list_monitor_names().get("EDID-001"),
            Some(&"Main".to_string())
        );
    }

    // ── Fresh store: no seeding ──────────────────────────────────────

    #[test]
    fn fresh_store_is_empty() {
        let dir = test_store_dir_unique();
        let store = Store::new(dir).unwrap();
        assert!(
            store.list_presets().is_empty(),
            "no presets are seeded on first run"
        );
        assert!(store.list_pins().is_empty(), "fresh store pins nothing");
        assert!(
            store.list_applied().is_empty(),
            "fresh store has nothing in use"
        );
    }

    #[test]
    fn existing_presets_untouched_by_purge() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("presets.json"),
            r#"[{"id":"a","name":"Existing","icc_hash":"","icc_filename":"","brightness":55.0,"contrast":60.0,"rgb_gains":[1.0,1.0,1.0],"gamma":2.2,"vibrance":50.0,"hue_deg":0.0,"color_model":"nvcp-v1"}]"#,
        )
        .unwrap();
        std::fs::write(dir.join("applied.json"), r#"{"EDID-A":"a"}"#).unwrap();
        // Skip monitor-enumeration wipe so the pre-existing preset survives.
        std::fs::write(dir.join(".monitor-enumeration-v2"), b"").unwrap();
        let store = Store::new(dir.clone()).unwrap();
        let list = store.list_presets();
        assert_eq!(list.len(), 1, "non-Standard presets survive the purge");
        assert_eq!(list[0].name, "Existing");
        assert_eq!(
            store.list_applied().get("EDID-A"),
            Some(&"a".to_string()),
            "applied records for surviving presets are untouched"
        );
        assert!(
            dir.join(".standard-seed-purge-v1").exists(),
            "purge sentinel written"
        );
    }

    // ── Seeded-Standard purge ────────────────────────────────────────

    fn write_standard_purge_fixture(dir: &std::path::Path, standard_json: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("presets.json"),
            format!(
                r#"[{standard_json},{{"id":"custom-1","name":"Custom","icc_hash":"","icc_filename":"","brightness":55.0,"contrast":60.0,"rgb_gains":[1.0,1.0,1.0],"gamma":2.2,"vibrance":50.0,"hue_deg":0.0,"color_model":"nvcp-v1"}}]"#,
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("pins.json"),
            r#"{"EDID-A":"std-1","EDID-B":"custom-1"}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("applied.json"),
            r#"{"EDID-A":"std-1","EDID-C":"custom-1"}"#,
        )
        .unwrap();
        // Skip monitor-enumeration wipe so fixtures survive.
        std::fs::write(dir.join(".monitor-enumeration-v2"), b"").unwrap();
    }

    const NEUTRAL_STANDARD_JSON: &str = r#"{"id":"std-1","name":"Standard","icc_hash":"","icc_filename":"","brightness":50.0,"contrast":50.0,"rgb_gains":[1.0,1.0,1.0],"gamma":1.0,"vibrance":50.0,"hue_deg":0.0,"color_model":"nvcp-v1"}"#;

    #[test]
    fn standard_seed_purge_removes_pristine_standard_everywhere() {
        let dir = test_store_dir_unique();
        write_standard_purge_fixture(&dir, NEUTRAL_STANDARD_JSON);
        let store = Store::new(dir.clone()).unwrap();
        let list = store.list_presets();
        assert_eq!(list.len(), 1, "pristine Standard is deleted");
        assert_eq!(list[0].name, "Custom");
        assert!(
            store.list_pins().get("EDID-A").is_none(),
            "Standard pin is purged"
        );
        assert_eq!(
            store.list_pins().get("EDID-B"),
            Some(&"custom-1".to_string()),
            "unrelated pin is untouched"
        );
        assert!(
            store.list_applied().get("EDID-A").is_none(),
            "Standard applied record is purged"
        );
        assert_eq!(
            store.list_applied().get("EDID-C"),
            Some(&"custom-1".to_string()),
            "unrelated applied record is untouched"
        );
        assert!(
            dir.join(".standard-seed-purge-v1").exists(),
            "sentinel written exactly-once"
        );
    }

    #[test]
    fn standard_seed_purge_keeps_tweaked_standard() {
        let dir = test_store_dir_unique();
        let tweaked = NEUTRAL_STANDARD_JSON.replace("\"brightness\":50.0", "\"brightness\":55.0");
        write_standard_purge_fixture(&dir, &tweaked);
        let store = Store::new(dir).unwrap();
        assert_eq!(
            store.list_presets().len(),
            2,
            "user-tweaked Standard survives the purge"
        );
        assert_eq!(
            store.list_pins().get("EDID-A"),
            Some(&"std-1".to_string()),
            "user-tweaked Standard keeps its pin"
        );
        assert_eq!(
            store.list_applied().get("EDID-A"),
            Some(&"std-1".to_string()),
            "user-tweaked Standard keeps its applied record"
        );
    }
}
