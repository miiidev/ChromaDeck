// ── JSON preset store + bundled ICC file management ────────────────────────
// Windows-only.  Persists presets as presets.json under %APPDATA%/ChromaDeck
// and ICC profiles as profiles/{sha256}.icc, keyed by edid_id.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

fn default_vibrance() -> f64 {
    50.0
}
fn default_saturation() -> f64 {
    50.0
}
fn default_hue() -> f64 {
    0.0
}
fn legacy_model() -> String {
    "gain-v1".into()
}

/// Validate a color_tag value: must be empty or #rrggbb hex.
fn is_valid_color_tag(tag: &str) -> bool {
    if tag.is_empty() {
        return true;
    }
    if tag.len() != 7 || !tag.starts_with("#") {
        return false;
    }
    for c in tag[1..].chars() {
        if !c.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

/// Tag marker shapes accepted by the store. An empty value means "pill".
const TAG_SHAPES: [&str; 8] = [
    "pill", "round", "square", "bar", "diamond", "hexagon", "triangle", "ring",
];

/// Validate a tag_shape value: must be empty or one of TAG_SHAPES.
fn is_valid_tag_shape(shape: &str) -> bool {
    shape.is_empty() || TAG_SHAPES.contains(&shape)
}

/// Normalise an optional tag_shape: empty and "pill" are the default and
/// collapse to None so old and new files stay equivalent.
fn normalise_tag_shape(shape: Option<String>) -> Option<String> {
    match shape {
        Some(s) if s.is_empty() || s == "pill" => None,
        other => other,
    }
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
    let mon_lengths = [
        31,
        if is_leap(y) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
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
    #[serde(default = "default_saturation")]
    pub saturation: f64, // 0–100, 50 = neutral (legacy JSON migrates to 50)
    #[serde(default = "default_hue")]
    pub hue_deg: f64, // 0–359 degrees
    #[serde(default = "legacy_model")]
    pub color_model: String,
    #[serde(default)]
    pub color_tag: Option<String>, // #rrggbb or None for untagged
    #[serde(default)]
    pub tag_shape: Option<String>, // pill | round | square | bar; None = pill
    /// Executable base names (lowercased) that auto-apply this preset when
    /// they become the foreground window. One preset may map many programs.
    #[serde(default)]
    pub attached_programs: Vec<String>,
    /// Optional global hotkey chord (canonical form e.g. `"Ctrl+Alt+F1"`).
    #[serde(default)]
    pub shortcut: Option<String>,
}

/// Input data for creating or updating a preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetInput {
    pub name: String,
    pub icc_path: Option<String>,
    #[serde(default)]
    pub clear_icc: bool,
    pub brightness: f64,
    pub contrast: f64,
    pub rgb_gains: [f64; 3],
    pub gamma: f64,
    pub vibrance: f64,
    #[serde(default = "default_saturation")]
    pub saturation: f64, // 0–100, 50 = neutral
    pub hue_deg: f64,
    #[serde(default)]
    pub color_tag: Option<String>,
    #[serde(default)]
    pub tag_shape: Option<String>,
    #[serde(default)]
    pub attached_programs: Vec<String>,
    #[serde(default)]
    pub shortcut: Option<String>,
}

/// Persisted user settings (`settings.json`). Additive fields default so a
/// file written by an older build still loads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Settings {
    #[serde(default)]
    auto_apply_enabled: bool,
    #[serde(default)]
    reapply_shortcut: Option<String>,
    #[serde(default)]
    cycle_shortcut: Option<String>,
    #[serde(default)]
    schedules: Vec<crate::schedule::ScheduleRule>,
}

// ── Store errors ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Json(serde_json::Error),
    NotFound(String),
    InvalidInput(String),
    /// The store opened degraded because a data file could not be parsed.
    /// The file was backed up and every write is refused until it is
    /// restored or removed, so an unreadable library is never replaced.
    LoadFault(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "I/O error: {e}"),
            StoreError::Json(e) => write!(f, "JSON error: {e}"),
            StoreError::NotFound(id) => write!(f, "preset not found: {id}"),
            StoreError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            StoreError::LoadFault(msg) => write!(f, "write refused: {msg}"),
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
        && p.saturation == 50.0
        && p.hue_deg == 0.0
        && p.icc_hash.is_empty()
}

/// Files that failed to parse at load time. Non-empty means the store is
/// read-only: an unreadable file must never be replaced by a later flush.
#[derive(Debug, Default)]
struct LoadFaults {
    messages: Vec<String>,
}

impl LoadFaults {
    fn any(&self) -> bool {
        !self.messages.is_empty()
    }

    fn message(&self) -> Option<&str> {
        self.messages.first().map(String::as_str)
    }
}

/// Read and parse a JSON file. A missing file is `Ok(None)`. A file that
/// exists but cannot be parsed is copied to `<name>.bak-<timestamp>-corrupt`
/// and recorded in `faults`; the caller gets `None` so the app can still
/// open, but `Store` refuses all writes so the bad file is never overwritten.
fn read_json_or_backup<T: DeserializeOwned>(
    path: &Path,
    faults: &mut LoadFaults,
) -> Result<Option<T>, StoreError> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    match serde_json::from_str(&content) {
        Ok(value) => Ok(Some(value)),
        Err(err) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "data.json".into());
            let bak_name = format!("{name}.bak-{}-corrupt", local_timestamp());
            let bak_path = path.with_file_name(&bak_name);
            let backup = match std::fs::copy(path, &bak_path) {
                Ok(_) => format!("backed up as {bak_name}"),
                Err(e) => format!("backup failed: {e}"),
            };
            let message = format!("{name} could not be parsed ({err}); {backup}");
            eprintln!("ChromaDeck: {message}. Writes are blocked so the file is preserved.");
            faults.messages.push(message);
            Ok(None)
        }
    }
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
    settings_path: PathBuf,
    auto_apply_enabled: bool, // persisted master switch; default false
    reapply_shortcut: Option<String>,
    cycle_shortcut: Option<String>,
    schedules: Vec<crate::schedule::ScheduleRule>,
    faults: LoadFaults,
}

impl Store {
    /// Open (or create) the store at `data_dir`.
    pub fn new(data_dir: PathBuf) -> Result<Self, StoreError> {
        let profiles_dir = data_dir.join("profiles");
        let presets_path = data_dir.join("presets.json");

        std::fs::create_dir_all(&profiles_dir)?;

        // A parse failure is reported and backed up, never treated as an
        // empty collection: the next flush would otherwise overwrite the
        // user's library. The store opens read-only until the file is fixed.
        let mut faults = LoadFaults::default();
        let presets: Vec<Preset> =
            read_json_or_backup(&presets_path, &mut faults)?.unwrap_or_default();

        let pins_path = data_dir.join("pins.json");
        let pinned: HashMap<String, String> =
            read_json_or_backup(&pins_path, &mut faults)?.unwrap_or_default();

        let names_path = data_dir.join("monitor_names.json");
        let monitor_names: HashMap<String, String> =
            read_json_or_backup(&names_path, &mut faults)?.unwrap_or_default();

        let settings_path = data_dir.join("settings.json");
        let settings: Settings =
            read_json_or_backup(&settings_path, &mut faults)?.unwrap_or_default();

        let applied_path = data_dir.join("applied.json");
        // ── Stale-record honesty ─────────────────────────────────────────────
        // The applied map is a recency hint: it records the last manually
        // applied preset per monitor. Unlike pins, it is NOT continuously
        // verified against hardware — an unpinned record can over-claim if
        // something external (game, HDR toggle, reboot) clears the gamma LUT
        // or NVAPI registers. Startup reasserts it once through the backend
        // registry (Phase F); pins + the enforce loop remain the definitive
        // source of truth for guaranteed state.
        let applied: HashMap<String, String> =
            read_json_or_backup(&applied_path, &mut faults)?.unwrap_or_default();

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
            settings_path,
            auto_apply_enabled: settings.auto_apply_enabled,
            reapply_shortcut: settings.reapply_shortcut,
            cycle_shortcut: settings.cycle_shortcut,
            schedules: settings.schedules,
            faults,
        };

        // A corrupt file leaves the store read-only. Do not run any migration
        // or sentinel write over data we could not parse — that is exactly how
        // an unreadable library could be replaced by an empty one.
        if store.faults.any() {
            return Ok(store);
        }

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

    /// Refuse every write while any backing file is unreadable. This is what
    /// keeps a corrupt `presets.json` from being replaced by an empty library.
    fn ensure_writable(&self) -> Result<(), StoreError> {
        match self.faults.message() {
            Some(msg) => Err(StoreError::LoadFault(format!(
                "{msg}; restore or remove it, then restart ChromaDeck"
            ))),
            None => Ok(()),
        }
    }

    /// Return all stored presets.
    pub fn list_presets(&self) -> Vec<Preset> {
        self.presets.clone()
    }

    /// Normalise a submitted program list and enforce global uniqueness:
    /// one executable may map to only one preset at a time. `self_id` is the
    /// preset being updated (`None` when creating) so its own entries do not
    /// collide with themselves.
    fn normalize_program_list(
        &self,
        input: &[String],
        self_id: Option<&str>,
    ) -> Result<Vec<String>, StoreError> {
        let mut out: Vec<String> = Vec::new();
        for raw in input {
            let Some(name) = crate::auto_apply::normalize_program_name(raw) else {
                continue; // drop empty / malformed entries instead of blocking the save
            };
            if out.iter().any(|existing| existing == &name) {
                continue;
            }
            if let Some(owner) = self.presets.iter().find(|preset| {
                Some(preset.id.as_str()) != self_id
                    && preset
                        .attached_programs
                        .iter()
                        .any(|mapped| mapped == &name)
            }) {
                return Err(StoreError::InvalidInput(format!(
                    "{name} is already mapped to \"{}\"",
                    owner.name
                )));
            }
            out.push(name);
        }
        Ok(out)
    }

    /// Normalize and validate an optional preset shortcut, checking uniqueness
    /// across other presets and global hotkeys.
    fn normalize_and_validate_shortcut(
        &self,
        raw: Option<&str>,
        exclude_preset_id: Option<&str>,
    ) -> Result<Option<String>, StoreError> {
        let Some(trimmed) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        let canonical =
            crate::hotkeys::normalize_shortcut(trimmed).map_err(StoreError::InvalidInput)?;

        if let Some(owner) = self.presets.iter().find(|p| {
            exclude_preset_id.is_none_or(|id| p.id != id)
                && p.shortcut.as_deref() == Some(canonical.as_str())
        }) {
            return Err(StoreError::InvalidInput(format!(
                "{canonical} is already bound to \"{}\"",
                owner.name
            )));
        }
        if self.reapply_shortcut.as_deref() == Some(canonical.as_str()) {
            return Err(StoreError::InvalidInput(format!(
                "{canonical} is already bound to global Reapply"
            )));
        }
        if self.cycle_shortcut.as_deref() == Some(canonical.as_str()) {
            return Err(StoreError::InvalidInput(format!(
                "{canonical} is already bound to global Cycle preset"
            )));
        }
        Ok(Some(canonical))
    }

    /// Current persisted global hotkey settings.
    pub fn global_hotkeys(&self) -> crate::hotkeys::GlobalHotkeySettings {
        crate::hotkeys::GlobalHotkeySettings {
            reapply_shortcut: self.reapply_shortcut.clone(),
            cycle_shortcut: self.cycle_shortcut.clone(),
        }
    }

    /// Validate and persist global hotkey bindings (`reapply_shortcut`, `cycle_shortcut`).
    pub fn set_global_hotkeys(
        &mut self,
        input: crate::hotkeys::GlobalHotkeySettings,
    ) -> Result<crate::hotkeys::GlobalHotkeySettings, StoreError> {
        self.ensure_writable()?;
        let normalize = |raw: Option<&str>| -> Result<Option<String>, StoreError> {
            match raw.map(str::trim).filter(|s| !s.is_empty()) {
                None => Ok(None),
                Some(s) => crate::hotkeys::normalize_shortcut(s)
                    .map(Some)
                    .map_err(StoreError::InvalidInput),
            }
        };
        let reapply = normalize(input.reapply_shortcut.as_deref())?;
        let cycle = normalize(input.cycle_shortcut.as_deref())?;

        if reapply.is_some() && reapply == cycle {
            return Err(StoreError::InvalidInput(
                "Reapply and Cycle cannot share the same shortcut".into(),
            ));
        }
        for candidate in [&reapply, &cycle].into_iter().flatten() {
            if let Some(owner) = self
                .presets
                .iter()
                .find(|p| p.shortcut.as_deref() == Some(candidate.as_str()))
            {
                return Err(StoreError::InvalidInput(format!(
                    "{candidate} is already bound to \"{}\"",
                    owner.name
                )));
            }
        }

        let prev_reapply = std::mem::replace(&mut self.reapply_shortcut, reapply);
        let prev_cycle = std::mem::replace(&mut self.cycle_shortcut, cycle);
        if let Err(err) = self.flush_settings() {
            self.reapply_shortcut = prev_reapply;
            self.cycle_shortcut = prev_cycle;
            return Err(err);
        }
        Ok(self.global_hotkeys())
    }

    /// Create a new preset from `input`.  Returns the created `Preset`.
    pub fn create_preset(&mut self, input: PresetInput) -> Result<Preset, StoreError> {
        self.ensure_writable()?;
        // Validate
        if input.name.trim().is_empty() {
            return Err(StoreError::InvalidInput("name cannot be empty".into()));
        }
        let attached_programs = self.normalize_program_list(&input.attached_programs, None)?;
        let shortcut = self.normalize_and_validate_shortcut(input.shortcut.as_deref(), None)?;
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
        if !(0.0..=100.0).contains(&input.saturation) {
            return Err(StoreError::InvalidInput(
                "saturation must be in 0.0..=100.0".into(),
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
        // Validate tag_shape
        if let Some(ref shape) = input.tag_shape {
            if !is_valid_tag_shape(shape) {
                return Err(StoreError::InvalidInput(
                    "tag_shape must be pill, round, square, bar, diamond, hexagon, triangle, ring or empty".into(),
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
            saturation: input.saturation,
            hue_deg: input.hue_deg,
            color_model: "nvcp-v1".into(),
            color_tag: input.color_tag.filter(|tag| !tag.is_empty()),
            tag_shape: normalise_tag_shape(input.tag_shape),
            attached_programs,
            shortcut,
        };

        self.presets.push(preset.clone());
        if let Err(error) = self.flush() {
            self.presets.pop();
            return Err(error);
        }
        Ok(preset)
    }

    /// Update an existing preset identified by `id`.
    pub fn update_preset(&mut self, id: &str, input: PresetInput) -> Result<Preset, StoreError> {
        self.ensure_writable()?;
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
        if !(0.0..=100.0).contains(&input.saturation) {
            return Err(StoreError::InvalidInput(
                "saturation must be in 0.0..=100.0".into(),
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

        let attached_programs = self.normalize_program_list(&input.attached_programs, Some(id))?;
        let shortcut = self.normalize_and_validate_shortcut(input.shortcut.as_deref(), Some(id))?;

        // Validate color_tag
        if let Some(ref tag) = input.color_tag {
            if !is_valid_color_tag(tag) {
                return Err(StoreError::InvalidInput(
                    "color_tag must be a hex color like #rrggbb or empty".into(),
                ));
            }
        }
        // Validate tag_shape
        if let Some(ref shape) = input.tag_shape {
            if !is_valid_tag_shape(shape) {
                return Err(StoreError::InvalidInput(
                    "tag_shape must be pill, round, square, bar, diamond, hexagon, triangle, ring or empty".into(),
                ));
            }
        }

        let previous_icc_hash = self.presets[idx].icc_hash.clone();

        // Handle optional ICC import (new path provided), explicit clear, or keep existing
        let (icc_hash, icc_filename) = if let Some(ref src) = input.icc_path {
            if src.trim().is_empty() {
                (String::new(), String::new())
            } else {
                let hash = self.import_icc_file(src)?;
                let filename = format!("{hash}.icc");
                (hash, filename)
            }
        } else if input.clear_icc {
            (String::new(), String::new())
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
            saturation: input.saturation,
            hue_deg: input.hue_deg,
            color_model: self.presets[idx].color_model.clone(),
            color_tag: input.color_tag.filter(|tag| !tag.is_empty()),
            tag_shape: normalise_tag_shape(input.tag_shape),
            attached_programs,
            shortcut,
        };

        let previous = std::mem::replace(&mut self.presets[idx], updated.clone());
        if let Err(error) = self.flush() {
            self.presets[idx] = previous;
            return Err(error);
        }
        if !previous_icc_hash.is_empty() && previous_icc_hash != updated.icc_hash {
            self.remove_orphan_icc(&previous_icc_hash);
        }
        Ok(updated)
    }

    /// Delete a preset by `id`.  Returns the deleted preset.
    pub fn delete_preset(&mut self, id: &str) -> Result<Preset, StoreError> {
        self.ensure_writable()?;
        let idx = self
            .presets
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| StoreError::NotFound(id.into()))?;

        let previous_presets = self.presets.clone();
        let previous_pins = self.pinned.clone();
        let previous_applied = self.applied.clone();
        let previous_schedules = self.schedules.clone();
        let removed = self.presets.remove(idx);
        let pinned_gone = self.pinned.values().any(|v| v == id);
        if pinned_gone {
            self.pinned.retain(|_, v| v != id);
        }
        let applied_gone = self.applied.values().any(|v| v == id);
        if applied_gone {
            self.applied.retain(|_, v| v != id);
        }
        let schedules_gone = self.schedules.iter().any(|s| s.preset_id == id);
        if schedules_gone {
            self.schedules.retain(|s| s.preset_id != id);
        }
        let persisted = (|| {
            if pinned_gone {
                self.flush_pins()?;
            }
            if applied_gone {
                self.flush_applied()?;
            }
            if schedules_gone {
                self.flush_settings()?;
            }
            self.flush()
        })();
        if let Err(error) = persisted {
            self.presets = previous_presets;
            self.pinned = previous_pins;
            self.applied = previous_applied;
            self.schedules = previous_schedules;
            let rollback = self
                .flush_pins()
                .and_then(|_| self.flush_applied())
                .and_then(|_| self.flush_settings())
                .and_then(|_| self.flush());
            if let Err(rollback_error) = rollback {
                return Err(StoreError::InvalidInput(format!(
                    "delete failed: {error}; rollback failed: {rollback_error}"
                )));
            }
            return Err(error);
        }
        if !removed.icc_hash.is_empty() {
            self.remove_orphan_icc(&removed.icc_hash);
        }
        Ok(removed)
    }

    /// Best-effort cleanup of a bundled ICC file once no preset references its hash.
    fn remove_orphan_icc(&self, hash: &str) {
        if hash.is_empty() {
            return;
        }
        let still_referenced = self.presets.iter().any(|p| p.icc_hash == hash);
        if !still_referenced {
            let path = self.profiles_dir.join(format!("{hash}.icc"));
            let _ = std::fs::remove_file(path);
        }
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
        self.ensure_writable()?;
        let json = serde_json::to_string_pretty(&self.presets)?;
        let tmp = self.data_dir.join("presets.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.presets_path)?;
        Ok(())
    }

    /// Pin a preset to a monitor. Returns error when preset does not exist.
    pub fn pin_preset(&mut self, edid_id: &str, preset_id: &str) -> Result<(), StoreError> {
        self.ensure_writable()?;
        let _ = self
            .presets
            .iter()
            .find(|p| p.id == preset_id)
            .ok_or_else(|| StoreError::NotFound(preset_id.into()))?;
        let previous = self.pinned.clone();
        self.pinned.insert(edid_id.into(), preset_id.into());
        if let Err(error) = self.flush_pins() {
            self.pinned = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Remove the pin for a monitor (no-op when absent).
    pub fn unpin_monitor(&mut self, edid_id: &str) -> Result<(), StoreError> {
        self.ensure_writable()?;
        if let Some(previous) = self.pinned.remove(edid_id) {
            if let Err(error) = self.flush_pins() {
                self.pinned.insert(edid_id.into(), previous);
                return Err(error);
            }
        }
        Ok(())
    }

    /// Return a copy of the current pin map (edid_id -> preset_id).
    pub fn list_pins(&self) -> HashMap<String, String> {
        self.pinned.clone()
    }

    /// Set (or clear, when the trimmed alias is empty) a monitor's display
    /// name. Rejects aliases longer than 64 characters.
    pub fn set_monitor_name(&mut self, edid_id: &str, alias: &str) -> Result<(), StoreError> {
        self.ensure_writable()?;
        let trimmed = alias.trim();
        if trimmed.chars().count() > 64 {
            return Err(StoreError::InvalidInput(
                "monitor name must be 64 characters or fewer".into(),
            ));
        }
        let previous = self.monitor_names.clone();
        if trimmed.is_empty() {
            self.monitor_names.remove(edid_id);
        } else {
            self.monitor_names.insert(edid_id.into(), trimmed.into());
        }
        if let Err(error) = self.flush_names() {
            self.monitor_names = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Return a copy of the monitor alias map (edid_id -> alias).
    pub fn list_monitor_names(&self) -> HashMap<String, String> {
        self.monitor_names.clone()
    }

    /// The directory backing this store (diagnostics log lives alongside
    /// the JSON state files).
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Directory holding bundled `{sha256}.icc` files.
    pub fn profiles_dir(&self) -> &Path {
        &self.profiles_dir
    }

    /// Public writable check for sibling store-writing modules (e.g. `bundle`).
    pub fn ensure_writable_public(&self) -> Result<(), StoreError> {
        self.ensure_writable()
    }

    pub fn is_valid_color_tag_str(tag: &str) -> bool {
        is_valid_color_tag(tag)
    }

    pub fn is_valid_tag_shape_str(shape: &str) -> bool {
        is_valid_tag_shape(shape)
    }

    pub fn normalise_tag_shape_opt(shape: Option<String>) -> Option<String> {
        normalise_tag_shape(shape)
    }

    /// Write raw ICC bytes into `profiles/{expected_hash}.icc` after verifying
    /// their SHA-256 digest. Returns `Ok(true)` if a new file was written, or
    /// `Ok(false)` if the identical file was already present.
    pub fn import_icc_bytes(&self, expected_hash: &str, bytes: &[u8]) -> Result<bool, StoreError> {
        self.ensure_writable()?;
        if bytes.is_empty() {
            return Err(StoreError::InvalidInput("ICC profile data is empty".into()));
        }
        use sha2::{Digest, Sha256};
        let actual = hex::encode(Sha256::digest(bytes));
        let expected = expected_hash.trim().to_ascii_lowercase();
        if actual != expected {
            return Err(StoreError::InvalidInput(format!(
                "ICC digest mismatch: expected {expected}, got {actual}"
            )));
        }
        let dest = self.profiles_dir.join(format!("{actual}.icc"));
        if dest.exists() {
            return Ok(false);
        }
        std::fs::write(&dest, bytes)?;
        Ok(true)
    }

    /// Atomically append imported presets and flush `presets.json`.
    pub fn insert_imported_presets(&mut self, imported: Vec<Preset>) -> Result<(), StoreError> {
        self.ensure_writable()?;
        let previous = self.presets.clone();
        self.presets.extend(imported);
        if let Err(error) = self.flush() {
            self.presets = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Return all persisted time-of-day schedule rules.
    pub fn list_schedules(&self) -> Vec<crate::schedule::ScheduleRule> {
        self.schedules.clone()
    }

    /// Validate and atomically persist schedule rules in `settings.json`.
    pub fn set_schedules(
        &mut self,
        schedules: Vec<crate::schedule::ScheduleRule>,
    ) -> Result<Vec<crate::schedule::ScheduleRule>, StoreError> {
        self.ensure_writable()?;
        let validated = crate::schedule::validate_rules(&schedules, &self.presets)
            .map_err(StoreError::InvalidInput)?;
        let previous = std::mem::replace(&mut self.schedules, validated.clone());
        if let Err(error) = self.flush_settings() {
            self.schedules = previous;
            return Err(error);
        }
        Ok(validated)
    }

    /// Lift every pin. Returns the number removed and flushes once.
    pub fn clear_all_pins(&mut self) -> Result<usize, StoreError> {
        self.ensure_writable()?;
        let previous = std::mem::take(&mut self.pinned);
        let removed = previous.len();
        if removed > 0 {
            if let Err(error) = self.flush_pins() {
                self.pinned = previous;
                return Err(error);
            }
        }
        Ok(removed)
    }

    /// Record a manual preset application for a monitor.
    /// Flushes to applied.json atomically.
    pub fn record_applied(&mut self, edid_id: &str, preset_id: &str) -> Result<(), StoreError> {
        self.ensure_writable()?;
        let previous = self.applied.clone();
        self.applied.insert(edid_id.into(), preset_id.into());
        if let Err(error) = self.flush_applied() {
            self.applied = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Clear the applied record for a monitor (e.g. after reset).
    /// No-op when no record exists. Flushes only on change.
    pub fn clear_applied(&mut self, edid_id: &str) -> Result<(), StoreError> {
        self.ensure_writable()?;
        if let Some(previous) = self.applied.remove(edid_id) {
            if let Err(error) = self.flush_applied() {
                self.applied.insert(edid_id.into(), previous);
                return Err(error);
            }
        }
        Ok(())
    }

    /// Return a copy of the applied map (edid_id -> preset_id).
    pub fn list_applied(&self) -> HashMap<String, String> {
        self.applied.clone()
    }

    /// Whether the auto-apply master switch is on (persisted, default false).
    pub fn auto_apply_enabled(&self) -> bool {
        self.auto_apply_enabled
    }

    /// Persist the auto-apply master switch. No-op writes are skipped.
    pub fn set_auto_apply_enabled(&mut self, enabled: bool) -> Result<(), StoreError> {
        self.ensure_writable()?;
        if self.auto_apply_enabled == enabled {
            return Ok(());
        }
        let previous = self.auto_apply_enabled;
        self.auto_apply_enabled = enabled;
        if let Err(error) = self.flush_settings() {
            self.auto_apply_enabled = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Every monitor that currently references `preset_id`, as the union of
    /// pins and applied records (sorted, de-duplicated). Captured before a
    /// preset delete so the affected displays can be restored through the
    /// backend abstraction.
    pub fn monitors_using(&self, preset_id: &str) -> Vec<String> {
        let mut edids: Vec<String> = self
            .pinned
            .iter()
            .filter(|(_, id)| id.as_str() == preset_id)
            .map(|(edid, _)| edid.clone())
            .chain(
                self.applied
                    .iter()
                    .filter(|(_, id)| id.as_str() == preset_id)
                    .map(|(edid, _)| edid.clone()),
            )
            .collect();
        edids.sort();
        edids.dedup();
        edids
    }

    /// Atomically persist the applied map to `applied.json`.
    fn flush_applied(&self) -> Result<(), StoreError> {
        self.ensure_writable()?;
        let json = serde_json::to_string_pretty(&self.applied)?;
        let tmp = self.data_dir.join("applied.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.applied_path)?;
        Ok(())
    }

    /// Atomically persist the alias map to `monitor_names.json`.
    fn flush_names(&self) -> Result<(), StoreError> {
        self.ensure_writable()?;
        let json = serde_json::to_string_pretty(&self.monitor_names)?;
        let tmp = self.data_dir.join("monitor_names.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.names_path)?;
        Ok(())
    }

    /// Atomically persist the pin map to `pins.json`.
    fn flush_pins(&self) -> Result<(), StoreError> {
        self.ensure_writable()?;
        let json = serde_json::to_string_pretty(&self.pinned)?;
        let tmp = self.data_dir.join("pins.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.pins_path)?;
        Ok(())
    }

    /// Atomically persist user settings to `settings.json`.
    fn flush_settings(&self) -> Result<(), StoreError> {
        self.ensure_writable()?;
        let settings = Settings {
            auto_apply_enabled: self.auto_apply_enabled,
            reapply_shortcut: self.reapply_shortcut.clone(),
            cycle_shortcut: self.cycle_shortcut.clone(),
            schedules: self.schedules.clone(),
        };
        let json = serde_json::to_string_pretty(&settings)?;
        let tmp = self.data_dir.join("settings.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.settings_path)?;
        Ok(())
    }
}

// ── Default app data directory ─────────────────────────────────────────────

/// Return the default store path.
///
/// Precedence:
/// 1. `CHROMADECK_DATA_DIR` env var (explicit override; blank = ignored).
/// 2. Portable mode: a file named `portable` (or `portable.txt`) sitting
///    beside the executable keeps all data in `<exe-dir>/data`, so the
///    portable ZIP leaves no trace outside its own folder. Installed and
///    dev builds ship no marker and are unaffected.
/// 3. Otherwise `%LOCALAPPDATA%/ChromaDeck`.
pub fn default_store_path() -> PathBuf {
    if let Ok(dir) = std::env::var("CHROMADECK_DATA_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(portable) = portable_data_dir(dir) {
                return portable;
            }
        }
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ChromaDeck")
}

/// Portable data dir for an executable directory: `Some(<dir>/data)` when
/// a `portable` / `portable.txt` marker file is present, else `None`.
/// Split out for unit testing (`current_exe` can't be faked).
fn portable_data_dir(exe_dir: &Path) -> Option<PathBuf> {
    if exe_dir.join("portable").exists() || exe_dir.join("portable.txt").exists() {
        Some(exe_dir.join("data"))
    } else {
        None
    }
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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppStore>,
    input: PresetInput,
) -> Result<Preset, String> {
    let created = {
        let mut store = state.0.lock().map_err(|e| e.to_string())?;
        store.create_preset(input).map_err(|e| e.to_string())?
    };
    crate::tray::refresh_tray(&app);
    crate::hotkeys::sync_hotkeys(&app);
    Ok(created)
}

#[tauri::command]
pub fn update_preset_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppStore>,
    id: String,
    input: PresetInput,
) -> Result<Preset, String> {
    let updated = {
        let mut store = state.0.lock().map_err(|e| e.to_string())?;
        store.update_preset(&id, input).map_err(|e| e.to_string())?
    };
    crate::tray::refresh_tray(&app);
    crate::hotkeys::sync_hotkeys(&app);
    Ok(updated)
}

#[tauri::command]
pub fn delete_preset_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppStore>,
    id: String,
) -> Result<Preset, String> {
    let _operation = crate::auto_apply_runtime::DISPLAY_OPERATION
        .lock()
        .map_err(|e| e.to_string())?;
    // Capture affected displays before the delete cascades pins/applied.
    let (removed, affected) = {
        let mut store = state.0.lock().map_err(|e| e.to_string())?;
        let affected = store.monitors_using(&id);
        let removed = store.delete_preset(&id).map_err(|e| e.to_string())?;
        (removed, affected)
    };

    // Removing a preset restores every display that referenced it, with the
    // same backend path as `reset_monitor` (parity with explicit Reset).
    // Restore is best-effort: the delete itself already succeeded, and a
    // disconnected display simply returns an offline result.
    for edid in &affected {
        crate::auto_apply_runtime::relinquish_manual(&app, edid);
    }
    if !affected.is_empty() {
        let api = crate::color::RealColorApi;
        let nv = crate::nvapi::RealNvapi;
        for (edid_id, result) in crate::color::restore_monitors(&api, &nv, &affected) {
            if let Some(error) = result.error {
                eprintln!(
                    "ChromaDeck: preset {id} removed but {edid_id} could not be restored: {error}"
                );
            }
        }
    }

    crate::tray::refresh_tray(&app);
    crate::hotkeys::sync_hotkeys(&app);
    Ok(removed)
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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppStore>,
    edid_id: String,
    preset_id: String,
) -> Result<(), String> {
    let _operation = crate::auto_apply_runtime::DISPLAY_OPERATION
        .lock()
        .map_err(|e| e.to_string())?;
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    store
        .pin_preset(&edid_id, &preset_id)
        .map_err(|e| e.to_string())?;
    drop(store);
    crate::auto_apply_runtime::relinquish_manual(&app, &edid_id);
    crate::tray::refresh_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn unpin_monitor_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppStore>,
    edid_id: String,
) -> Result<(), String> {
    let _operation = crate::auto_apply_runtime::DISPLAY_OPERATION
        .lock()
        .map_err(|e| e.to_string())?;
    {
        let mut store = state.0.lock().map_err(|e| e.to_string())?;
        store.unpin_monitor(&edid_id).map_err(|e| e.to_string())?;
    }
    crate::tray::refresh_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn list_pins_cmd(state: tauri::State<'_, AppStore>) -> Result<HashMap<String, String>, String> {
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
pub fn get_auto_apply_enabled_cmd(state: tauri::State<'_, AppStore>) -> Result<bool, String> {
    let store = state.0.lock().map_err(|e| e.to_string())?;
    Ok(store.auto_apply_enabled())
}

#[tauri::command]
pub fn set_auto_apply_enabled_cmd(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    crate::auto_apply_runtime::set_auto_apply(&app, enabled)
}

#[tauri::command]
pub fn set_monitor_name_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppStore>,
    edid_id: String,
    alias: String,
) -> Result<(), String> {
    {
        let mut store = state.0.lock().map_err(|e| e.to_string())?;
        store
            .set_monitor_name(&edid_id, &alias)
            .map_err(|e| e.to_string())?;
    }
    crate::tray::refresh_tray(&app);
    Ok(())
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
            clear_icc: false,
            brightness: 55.0,
            contrast: 60.0,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 2.2,
            vibrance: 50.0,
            saturation: 50.0,
            hue_deg: 0.0,
            color_tag: None,
            tag_shape: None,
            attached_programs: vec![],
            shortcut: None,
        }
    }

    /// Create a Store backed by a unique temp directory (returns the dir).
    /// Uniqueness is a process-wide atomic counter, not the wall clock:
    /// Windows clock granularity is coarse, so two tests starting in the
    /// same tick would otherwise share a directory and race.
    static TEST_DIR_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

    #[test]
    fn failed_preset_writes_preserve_existing_library() {
        let dir = test_store_dir_unique();
        let mut store = Store::new(dir.clone()).unwrap();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.pin_preset("E", &preset.id).unwrap();
        std::fs::create_dir(dir.join("presets.json.tmp")).unwrap();
        assert!(store.create_preset(minimal_input()).is_err());
        assert!(store.update_preset(&preset.id, minimal_input()).is_err());
        assert!(store.delete_preset(&preset.id).is_err());
        assert_eq!(store.list_presets().len(), 1);
        assert_eq!(store.list_pins().get("E"), Some(&preset.id));
        let reopened = Store::new(dir).unwrap();
        assert_eq!(reopened.list_presets().len(), 1);
        assert_eq!(reopened.list_pins().get("E"), Some(&preset.id));
    }

    #[test]
    fn failed_settings_alias_and_applied_writes_preserve_state() {
        let dir = test_store_dir_unique();
        let mut store = Store::new(dir.clone()).unwrap();
        store.set_monitor_name("E", "Display").unwrap();
        store.record_applied("E", "p1").unwrap();
        for name in [
            "settings.json.tmp",
            "monitor_names.json.tmp",
            "applied.json.tmp",
        ] {
            std::fs::create_dir(dir.join(name)).unwrap();
        }
        assert!(store.set_auto_apply_enabled(true).is_err());
        assert!(!store.auto_apply_enabled());
        assert!(store.set_monitor_name("E", "").is_err());
        assert_eq!(
            store.list_monitor_names().get("E").map(String::as_str),
            Some("Display")
        );
        assert!(store.clear_applied("E").is_err());
        assert_eq!(
            store.list_applied().get("E").map(String::as_str),
            Some("p1")
        );
    }

    #[test]
    fn failed_pin_removal_preserves_memory_and_disk() {
        let dir = test_store_dir_unique();
        let mut store = Store::new(dir.clone()).unwrap();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.pin_preset("E", &preset.id).unwrap();
        std::fs::create_dir(dir.join("pins.json.tmp")).unwrap();
        assert!(store.unpin_monitor("E").is_err());
        assert_eq!(store.list_pins().get("E"), Some(&preset.id));
        assert!(store.clear_all_pins().is_err());
        assert_eq!(store.list_pins().get("E"), Some(&preset.id));
        assert_eq!(
            Store::new(dir).unwrap().list_pins().get("E"),
            Some(&preset.id)
        );
    }

    // ── portable data resolution ─────────────────────────────────────────

    #[test]
    fn portable_marker_selects_local_data_dir() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(portable_data_dir(&dir), None);
        std::fs::write(dir.join("portable"), b"").unwrap();
        assert_eq!(portable_data_dir(&dir), Some(dir.join("data")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn portable_txt_marker_selects_local_data_dir() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("portable.txt"), b"").unwrap();
        assert_eq!(portable_data_dir(&dir), Some(dir.join("data")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn env_override_selects_data_dir() {
        // No other test touches this var, so set/restore here is safe.
        let dir = test_store_dir_unique();
        std::env::set_var("CHROMADECK_DATA_DIR", &dir);
        assert_eq!(default_store_path(), dir);
        std::env::remove_var("CHROMADECK_DATA_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_opens_inside_portable_data_dir() {
        let exe_dir = test_store_dir_unique();
        std::fs::create_dir_all(&exe_dir).unwrap();
        std::fs::write(exe_dir.join("portable"), b"").unwrap();
        let data_dir = portable_data_dir(&exe_dir).unwrap();
        let store = Store::new(data_dir.clone()).unwrap();
        assert!(data_dir.join("profiles").is_dir());
        assert!(store.list_presets().is_empty());
        let _ = std::fs::remove_dir_all(&exe_dir);
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
                    clear_icc: false,
                    brightness: 55.0,
                    contrast: 60.0,
                    rgb_gains: [0.8, 0.9, 1.0],
                    gamma: 2.0,
                    vibrance: 50.0,
                    saturation: 50.0,
                    hue_deg: 0.0,
                    color_tag: None,
                    tag_shape: None,
                    attached_programs: vec![],
                    shortcut: None,
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
        let err = store.import_icc("C:\\nonexistent\\file.icc").unwrap_err();
        assert!(matches!(err, StoreError::InvalidInput(_)));
    }

    #[test]
    fn import_icc_rejects_empty_file() {
        let mut store = test_store();
        let empty_path = store.data_dir.join("empty.icc");
        std::fs::write(&empty_path, b"").unwrap();
        let err = store.import_icc(empty_path.to_str().unwrap()).unwrap_err();
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
                    clear_icc: false,
                    brightness: 30.0,
                    contrast: 60.0,
                    rgb_gains: [0.5, 0.5, 0.5],
                    gamma: 2.5,
                    vibrance: 50.0,
                    saturation: 50.0,
                    hue_deg: 0.0,
                    color_tag: None,
                    tag_shape: None,
                    attached_programs: vec![],
                    shortcut: None,
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
        assert_eq!(preset.saturation, 50.0);
        assert_eq!(preset.hue_deg, 0.0);
    }

    #[test]
    fn legacy_json_without_saturation_gets_neutral_50() {
        // Exact legacy presets.json shape: no `saturation` key at all.
        let json = r#"{"id":"x","name":"Legacy","icc_hash":"","icc_filename":"","brightness":55.0,"contrast":60.0,"rgb_gains":[1.0,1.0,1.0],"gamma":2.2,"vibrance":75.0,"hue_deg":0.0,"color_model":"nvcp-v1","color_tag":null}"#;
        let preset: Preset = serde_json::from_str(json).unwrap();
        assert_eq!(preset.saturation, 50.0);
        // The rest of the legacy record is unchanged.
        assert_eq!(preset.brightness, 55.0);
        assert_eq!(preset.contrast, 60.0);
        assert_eq!(preset.vibrance, 75.0);
    }

    #[test]
    fn preset_input_without_saturation_defaults_to_neutral() {
        // Older clients / hand-written payloads may omit `saturation`. It must
        // deserialize to the neutral 50 rather than hard-erroring on the
        // command boundary.
        let json = r#"{
            "name": "No saturation",
            "icc_path": null,
            "brightness": 50.0,
            "contrast": 50.0,
            "rgb_gains": [1.0, 1.0, 1.0],
            "gamma": 1.0,
            "vibrance": 50.0,
            "hue_deg": 0.0,
            "color_tag": null
        }"#;
        let input: PresetInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.saturation, 50.0);
    }

    #[test]
    fn saturation_round_trips_through_store() {
        let dir = test_store_dir_unique();
        let mut store = test_store_at(dir.clone());
        let mut input = minimal_input();
        input.saturation = 70.0;
        let preset = store.create_preset(input).unwrap();
        assert_eq!(preset.saturation, 70.0);
        drop(store);

        let reopened = Store::new(dir.clone()).unwrap();
        assert_eq!(reopened.list_presets()[0].saturation, 70.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_preset_rejects_bad_saturation() {
        for bad in [-1.0, 101.0] {
            let mut input = minimal_input();
            input.saturation = bad;
            let err = test_store().create_preset(input).unwrap_err();
            assert!(err.to_string().contains("saturation"), "bad value {bad}");
        }
    }

    #[test]
    fn update_preset_carries_saturation() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        let mut input = minimal_input();
        input.saturation = 20.0;
        let updated = store.update_preset(&preset.id, input).unwrap();
        assert_eq!(updated.saturation, 20.0);
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
    fn create_preset_carries_vibrance_hue_and_saturation() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.vibrance = 75.0;
        input.saturation = 65.0;
        input.hue_deg = 120.0;
        let preset = store.create_preset(input).unwrap();
        assert_eq!(preset.vibrance, 75.0);
        assert_eq!(preset.saturation, 65.0);
        assert_eq!(preset.hue_deg, 120.0);
    }

    // ── color_tag ──────────────────────────────────────────────────────────

    #[test]
    fn color_tag_defaults_to_none() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        assert!(
            preset.color_tag.is_none(),
            "untagged preset has no color_tag"
        );
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
        assert!(
            preset.color_tag.is_none(),
            "empty string color_tag becomes None"
        );
    }

    #[test]
    fn color_tag_rejects_bad_format() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.color_tag = Some("not-a-color".into());
        let err = store.create_preset(input).unwrap_err();
        assert!(
            err.to_string().contains("color_tag"),
            "error mentions color_tag"
        );
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
        assert!(
            preset.color_tag.is_none(),
            "old JSON without color_tag loads as None"
        );
        assert!(
            preset.tag_shape.is_none(),
            "old JSON without tag_shape loads as None"
        );
    }

    // ── tag_shape ──────────────────────────────────────────────────────────

    #[test]
    fn tag_shape_defaults_to_none() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        assert!(
            preset.tag_shape.is_none(),
            "default marker shape is pill (None)"
        );
    }

    #[test]
    fn tag_shape_store_and_roundtrip() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.tag_shape = Some("bar".into());
        let preset = store.create_preset(input).unwrap();
        assert_eq!(preset.tag_shape, Some("bar".to_string()));
    }

    #[test]
    fn tag_shape_pill_and_empty_normalise_to_none() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.tag_shape = Some("pill".into());
        assert!(store.create_preset(input).unwrap().tag_shape.is_none());

        let mut input = minimal_input();
        input.tag_shape = Some("".into());
        assert!(store.create_preset(input).unwrap().tag_shape.is_none());
    }

    #[test]
    fn tag_shape_rejects_unknown_value() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.tag_shape = Some("blob".into());
        let err = store.create_preset(input).unwrap_err();
        assert!(err.to_string().contains("tag_shape"));
    }

    #[test]
    fn tag_shape_accepts_every_supported_value() {
        let mut store = test_store();
        for shape in [
            "pill", "round", "square", "bar", "diamond", "hexagon", "triangle", "ring",
        ] {
            let mut input = minimal_input();
            input.tag_shape = Some(shape.into());
            assert!(
                store.create_preset(input).is_ok(),
                "{shape} should be accepted"
            );
        }
    }

    #[test]
    fn tag_shape_update_changes_and_clears() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        let mut input = minimal_input();
        input.tag_shape = Some("square".into());
        let updated = store.update_preset(&preset.id, input).unwrap();
        assert_eq!(updated.tag_shape, Some("square".to_string()));

        let mut input = minimal_input();
        input.tag_shape = Some("round".into());
        let updated = store.update_preset(&preset.id, input).unwrap();
        assert_eq!(updated.tag_shape, Some("round".to_string()));
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
                    clear_icc: false,
                    brightness: 55.0,
                    contrast: 60.0,
                    rgb_gains: [1.0, 1.0, 1.0],
                    gamma: 2.2,
                    vibrance: 50.0,
                    saturation: 50.0,
                    hue_deg: 0.0,
                    color_tag: Some("#ef4444".into()),
                    tag_shape: None,
                    attached_programs: vec![],
                    shortcut: None,
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
                    clear_icc: false,
                    brightness: 55.0,
                    contrast: 60.0,
                    rgb_gains: [1.0, 1.0, 1.0],
                    gamma: 2.2,
                    vibrance: 50.0,
                    saturation: 50.0,
                    hue_deg: 0.0,
                    color_tag: Some("".into()),
                    tag_shape: None,
                    attached_programs: vec![],
                    shortcut: None,
                },
            )
            .unwrap();
        assert!(
            updated.color_tag.is_none(),
            "clearing color_tag produces None"
        );
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
                    clear_icc: false,
                    brightness: 55.0,
                    contrast: 60.0,
                    rgb_gains: [1.0, 1.0, 1.0],
                    gamma: 2.2,
                    vibrance: 50.0,
                    saturation: 50.0,
                    hue_deg: 0.0,
                    color_tag: Some("invalid".into()),
                    tag_shape: None,
                    attached_programs: vec![],
                    shortcut: None,
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
        store.unpin_monitor("EDID-001").unwrap();
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

    #[test]
    fn clear_all_pins_counts_then_zero() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.pin_preset("EDID-001", &preset.id).unwrap();
        store.pin_preset("EDID-002", &preset.id).unwrap();
        store.pin_preset("EDID-003", &preset.id).unwrap();
        let removed = store.clear_all_pins().unwrap();
        assert_eq!(removed, 3);
        assert!(store.list_pins().is_empty());
        assert_eq!(store.clear_all_pins().unwrap(), 0);
    }

    #[test]
    fn clear_all_pins_flushes_across_reopen() {
        let dir = test_store_dir();
        let mut store = Store::new(dir.clone()).unwrap();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.pin_preset("EDID-001", &preset.id).unwrap();
        store.clear_all_pins().unwrap();
        drop(store);
        let reopened = Store::new(dir).unwrap();
        assert!(reopened.list_pins().is_empty());
    }

    // ── applied record (persistent last-manual-apply hint) ──────────────

    #[test]
    fn applied_record_flush_and_reload() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.record_applied("EDID-001", &preset.id).unwrap();
        assert_eq!(store.list_applied().get("EDID-001"), Some(&preset.id),);
        let list = store.list_applied();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn applied_clear_removes_entry() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        store.record_applied("EDID-001", &preset.id).unwrap();
        assert!(!store.list_applied().is_empty());
        store.clear_applied("EDID-001").unwrap();
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
        assert_eq!(reopened.list_applied().get("EDID-001"), Some(&preset.id),);
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
    fn malformed_applied_is_backed_up_and_blocks_writes() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        let corrupt = br#"this is not json at all"#;
        std::fs::write(dir.join("applied.json"), corrupt).unwrap();
        std::fs::write(dir.join("presets.json"), b"[]").unwrap();

        let mut store = Store::new(dir.clone()).unwrap();
        assert!(store.list_applied().is_empty());

        let backups: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("applied.json.bak-") && n.ends_with("-corrupt"))
            .collect();
        assert_eq!(backups.len(), 1, "corrupt applied file is backed up");
        assert_eq!(std::fs::read(dir.join(&backups[0])).unwrap(), corrupt);

        // Every write is refused, so the unreadable file is never replaced.
        let err = store.record_applied("EDID-001", "preset-1").unwrap_err();
        assert!(matches!(err, StoreError::LoadFault(_)), "{err}");
        assert_eq!(std::fs::read(dir.join("applied.json")).unwrap(), corrupt);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_presets_json_is_backed_up_and_writes_refused() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        // Truncated record: valid prefix, invalid JSON overall.
        let corrupt = br#"[{"id":"x","name":"Broken""#;
        std::fs::write(dir.join("presets.json"), corrupt).unwrap();

        let mut store = Store::new(dir.clone()).unwrap();
        assert!(
            store.list_presets().is_empty(),
            "unparseable data must not be served as an empty library"
        );

        let backups: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("presets.json.bak-") && n.ends_with("-corrupt"))
            .collect();
        assert_eq!(backups.len(), 1, "corrupt preset file is backed up");
        assert_eq!(std::fs::read(dir.join(&backups[0])).unwrap(), corrupt);

        // The next save must be refused: the corrupt library stays on disk
        // for recovery instead of being overwritten by an empty one.
        let err = store.create_preset(minimal_input()).unwrap_err();
        assert!(matches!(err, StoreError::LoadFault(_)), "{err}");
        assert_eq!(std::fs::read(dir.join("presets.json")).unwrap(), corrupt);
        assert!(
            !dir.join("presets.json.tmp").exists(),
            "a refused save must not leave a temp file behind"
        );

        // No migration sentinels were written over unreadable data.
        assert!(!dir.join(".monitor-enumeration-v2").exists());
        let _ = std::fs::remove_dir_all(&dir);
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
            !store.list_pins().contains_key("EDID-A"),
            "Standard pin is purged"
        );
        assert_eq!(
            store.list_pins().get("EDID-B"),
            Some(&"custom-1".to_string()),
            "unrelated pin is untouched"
        );
        assert!(
            !store.list_applied().contains_key("EDID-A"),
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

    #[test]
    fn standard_seed_purge_keeps_saturation_tweaked_standard() {
        let dir = test_store_dir_unique();
        // Saturation defaults to 50 on legacy JSON; an explicit non-neutral
        // value means the user tuned it, so the Standard must survive.
        let tweaked = NEUTRAL_STANDARD_JSON
            .replace("\"vibrance\":50.0", "\"vibrance\":50.0,\"saturation\":60.0");
        write_standard_purge_fixture(&dir, &tweaked);
        let store = Store::new(dir).unwrap();
        assert_eq!(
            store.list_presets().len(),
            2,
            "saturation-tweaked Standard survives the purge"
        );
        assert_eq!(
            store.list_pins().get("EDID-A"),
            Some(&"std-1".to_string()),
            "saturation-tweaked Standard keeps its pin"
        );
    }

    // ── removal restore targeting ─────────────────────────────────────────

    #[test]
    fn monitors_using_unions_pinned_and_applied_records() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();
        let other = store.create_preset(minimal_input()).unwrap();

        store.pin_preset("EDID-A", &preset.id).unwrap();
        store.record_applied("EDID-B", &preset.id).unwrap();
        // A display that appears in both maps must be reported once.
        store.pin_preset("EDID-C", &preset.id).unwrap();
        store.record_applied("EDID-C", &preset.id).unwrap();
        // Another preset's displays must not leak in.
        store.pin_preset("EDID-D", &other.id).unwrap();
        store.record_applied("EDID-E", &other.id).unwrap();

        assert_eq!(
            store.monitors_using(&preset.id),
            vec![
                "EDID-A".to_string(),
                "EDID-B".to_string(),
                "EDID-C".to_string()
            ]
        );
        assert!(store.monitors_using("does-not-exist").is_empty());
    }

    // ── attached programs (auto-apply mappings) ──────────────────────────

    #[test]
    fn attached_programs_are_normalized_and_persisted() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("presets.json"), b"[]").unwrap();
        let mut store = Store::new(dir.clone()).unwrap();
        let mut input = minimal_input();
        input.attached_programs = vec![
            r"C:\Games\VLC.EXE".into(),
            "mpc-hc.exe".into(),
            "mpc-hc.exe".into(),
            "   ".into(),
        ];
        let preset = store.create_preset(input).unwrap();
        assert_eq!(
            preset.attached_programs,
            vec!["vlc.exe".to_string(), "mpc-hc.exe".to_string()]
        );
        drop(store);
        let reopened = Store::new(dir.clone()).unwrap();
        assert_eq!(
            reopened.list_presets()[0].attached_programs,
            vec!["vlc.exe".to_string(), "mpc-hc.exe".to_string()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_program_across_presets_is_rejected() {
        let mut store = test_store();
        let mut first = minimal_input();
        first.attached_programs = vec!["chrome.exe".into()];
        store.create_preset(first).unwrap();

        let mut second = minimal_input();
        second.name = "Second".into();
        second.attached_programs = vec!["CHROME.EXE".into()];
        let err = store.create_preset(second).unwrap_err();
        assert!(matches!(err, StoreError::InvalidInput(_)), "{err}");
        assert!(
            err.to_string()
                .contains("already mapped to \"Test Preset\""),
            "{err}"
        );
    }

    #[test]
    fn updating_a_preset_keeps_its_own_programs() {
        let mut store = test_store();
        let mut input = minimal_input();
        input.attached_programs = vec!["chrome.exe".into()];
        let preset = store.create_preset(input).unwrap();

        let mut update = minimal_input();
        update.attached_programs = vec!["chrome.exe".into(), "code.exe".into()];
        let updated = store.update_preset(&preset.id, update).unwrap();
        assert_eq!(
            updated.attached_programs,
            vec!["chrome.exe".to_string(), "code.exe".to_string()]
        );
    }

    #[test]
    fn legacy_preset_json_without_attached_programs_loads_empty() {
        let json = r#"{"id":"x","name":"Old","icc_hash":"","icc_filename":"","brightness":50.0,"contrast":50.0,"rgb_gains":[1.0,1.0,1.0],"gamma":1.0}"#;
        let preset: Preset = serde_json::from_str(json).unwrap();
        assert!(preset.attached_programs.is_empty());
    }

    // ── auto-apply master switch ─────────────────────────────────────────

    #[test]
    fn auto_apply_defaults_off_and_persists() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("presets.json"), b"[]").unwrap();

        {
            let mut store = Store::new(dir.clone()).unwrap();
            assert!(!store.auto_apply_enabled(), "default is off");
            store.set_auto_apply_enabled(true).unwrap();
            assert!(store.auto_apply_enabled());
        }

        let reopened = Store::new(dir.clone()).unwrap();
        assert!(reopened.auto_apply_enabled(), "switch survives reload");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_settings_is_backed_up_and_leaves_switch_off() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("presets.json"), b"[]").unwrap();
        let corrupt = br#"not json"#;
        std::fs::write(dir.join("settings.json"), corrupt).unwrap();

        let mut store = Store::new(dir.clone()).unwrap();
        assert!(!store.auto_apply_enabled());
        let backups: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("settings.json.bak-") && n.ends_with("-corrupt"))
            .collect();
        assert_eq!(backups.len(), 1, "corrupt settings file is backed up");
        assert_eq!(std::fs::read(dir.join(&backups[0])).unwrap(), corrupt);

        let err = store.set_auto_apply_enabled(true).unwrap_err();
        assert!(matches!(err, StoreError::LoadFault(_)), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Phase 1: rgb_gains update parity & clear_icc ─────────────────────

    #[test]
    fn update_preset_rejects_out_of_range_rgb_gains() {
        let mut store = test_store();
        let preset = store.create_preset(minimal_input()).unwrap();

        let mut negative = minimal_input();
        negative.rgb_gains = [-0.1, 1.0, 1.0];
        let err = store.update_preset(&preset.id, negative).unwrap_err();
        assert!(err.to_string().contains("rgb_gains"), "{err}");

        let mut too_high = minimal_input();
        too_high.rgb_gains = [1.0, 10.1, 1.0];
        let err = store.update_preset(&preset.id, too_high).unwrap_err();
        assert!(err.to_string().contains("rgb_gains"), "{err}");

        let mut nan = minimal_input();
        nan.rgb_gains = [1.0, 1.0, f64::NAN];
        let err = store.update_preset(&preset.id, nan).unwrap_err();
        assert!(err.to_string().contains("rgb_gains"), "{err}");
    }

    #[test]
    fn update_preset_clear_icc_removes_association_and_orphan_file() {
        let dir = test_store_dir_unique();
        let mut store = test_store_at(dir.clone());
        let src_icc = dir.join("sample.icc");
        std::fs::write(&src_icc, b"fake icc profile bytes").unwrap();

        let mut input = minimal_input();
        input.icc_path = Some(src_icc.to_string_lossy().into_owned());
        let preset = store.create_preset(input).unwrap();
        assert!(!preset.icc_hash.is_empty());
        let bundled = dir.join("profiles").join(&preset.icc_filename);
        assert!(bundled.exists(), "bundled ICC file should exist after create");

        let mut clear = minimal_input();
        clear.clear_icc = true;
        let updated = store.update_preset(&preset.id, clear).unwrap();
        assert!(updated.icc_hash.is_empty());
        assert!(updated.icc_filename.is_empty());
        assert!(!bundled.exists(), "unreferenced ICC file should be cleaned up");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn update_preset_clear_icc_keeps_shared_icc_file() {
        let dir = test_store_dir_unique();
        let mut store = test_store_at(dir.clone());
        let src_icc = dir.join("shared.icc");
        std::fs::write(&src_icc, b"shared icc profile bytes").unwrap();
        let path_str = src_icc.to_string_lossy().into_owned();

        let mut first = minimal_input();
        first.icc_path = Some(path_str.clone());
        let p1 = store.create_preset(first).unwrap();

        let mut second = minimal_input();
        second.name = "Second".into();
        second.icc_path = Some(path_str);
        let p2 = store.create_preset(second).unwrap();
        assert_eq!(p1.icc_hash, p2.icc_hash);

        let bundled = dir.join("profiles").join(&p1.icc_filename);
        let mut clear = minimal_input();
        clear.clear_icc = true;
        let updated = store.update_preset(&p1.id, clear).unwrap();
        assert!(updated.icc_hash.is_empty());
        assert!(bundled.exists(), "ICC file still referenced by p2 must remain");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preset_input_deserializes_without_clear_icc() {
        let json = r#"{"name":"Legacy","icc_path":null,"brightness":50.0,"contrast":50.0,"rgb_gains":[1.0,1.0,1.0],"gamma":1.0,"vibrance":50.0,"hue_deg":0.0}"#;
        let input: PresetInput = serde_json::from_str(json).unwrap();
        assert!(!input.clear_icc);
    }

    // ── Phase 3: preset & global shortcuts ───────────────────────────────

    #[test]
    fn preset_shortcuts_normalize_persist_and_reject_duplicates() {
        let dir = test_store_dir_unique();
        let mut store = test_store_at(dir.clone());

        let mut first = minimal_input();
        first.shortcut = Some("alt+ctrl+f1".into());
        let p1 = store.create_preset(first).unwrap();
        assert_eq!(p1.shortcut.as_deref(), Some("Ctrl+Alt+F1"));

        let mut dup = minimal_input();
        dup.name = "Second".into();
        dup.shortcut = Some("Ctrl+Alt+F1".into());
        let err = store.create_preset(dup).unwrap_err();
        assert!(err.to_string().contains("already bound"), "{err}");

        // Updating the same preset with its own shortcut succeeds
        let mut upd = minimal_input();
        upd.shortcut = Some("Ctrl+Alt+F1".into());
        let p1_updated = store.update_preset(&p1.id, upd).unwrap();
        assert_eq!(p1_updated.shortcut.as_deref(), Some("Ctrl+Alt+F1"));

        // Global hotkey collides with preset shortcut -> rejected
        let gerr = store
            .set_global_hotkeys(crate::hotkeys::GlobalHotkeySettings {
                reapply_shortcut: Some("Ctrl+Alt+F1".into()),
                cycle_shortcut: None,
            })
            .unwrap_err();
        assert!(gerr.to_string().contains("already bound"), "{gerr}");

        // Distinct global hotkeys persist across reload
        let saved = store
            .set_global_hotkeys(crate::hotkeys::GlobalHotkeySettings {
                reapply_shortcut: Some("ctrl+alt+r".into()),
                cycle_shortcut: Some("ctrl+alt+c".into()),
            })
            .unwrap();
        assert_eq!(saved.reapply_shortcut.as_deref(), Some("Ctrl+Alt+R"));
        assert_eq!(saved.cycle_shortcut.as_deref(), Some("Ctrl+Alt+C"));

        let reopened = Store::new(dir.clone()).unwrap();
        assert_eq!(reopened.global_hotkeys(), saved);
        assert_eq!(
            reopened.list_presets()[0].shortcut.as_deref(),
            Some("Ctrl+Alt+F1")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn schedules_persist_across_reopen_and_cascade_on_preset_delete() {
        let dir = test_store_dir_unique();
        let mut store = test_store_at(dir.clone());
        let preset = store.create_preset(minimal_input()).unwrap();

        let saved = store
            .set_schedules(vec![crate::schedule::ScheduleRule {
                id: String::new(),
                edid_id: "EDID:MON-1".into(),
                preset_id: preset.id.clone(),
                start_time: "21:00".into(),
                end_time: "07:00".into(),
                enabled: true,
            }])
            .unwrap();
        assert_eq!(saved.len(), 1);
        assert!(!saved[0].id.is_empty());

        let reopened = Store::new(dir.clone()).unwrap();
        assert_eq!(reopened.list_schedules(), saved);

        store.delete_preset(&preset.id).unwrap();
        assert!(store.list_schedules().is_empty());

        let reopened_after = Store::new(dir.clone()).unwrap();
        assert!(reopened_after.list_schedules().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_settings_json_without_schedules_loads_empty() {
        let dir = test_store_dir_unique();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"auto_apply_enabled": true}"#,
        )
        .unwrap();
        let store = Store::new(dir.clone()).unwrap();
        assert!(store.auto_apply_enabled());
        assert!(store.list_schedules().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
