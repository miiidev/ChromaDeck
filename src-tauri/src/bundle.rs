// ── Preset & library portable bundle (.chromadeck.json) ────────────────────
// Self-contained JSON export/import format with hex-encoded ICC blobs keyed by
// SHA-256 hash. Zero external zip/C dependencies, human-inspectable, and every
// imported ICC blob is SHA-256 verified before touching `profiles/{hash}.icc`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Emitter;

use crate::store::{AppStore, Preset, Store, StoreError};

pub const BUNDLE_SCHEMA_VERSION: u32 = 1;

fn default_vibrance() -> f64 {
    50.0
}
fn default_saturation() -> f64 {
    50.0
}
fn default_hue() -> f64 {
    0.0
}
fn default_rgb_gains() -> [f64; 3] {
    [1.0, 1.0, 1.0]
}
fn default_color_model() -> String {
    "nvcp-v1".into()
}

/// One preset entry inside a `.chromadeck.json` bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedPreset {
    pub name: String,
    #[serde(default)]
    pub icc_hash: String,
    #[serde(default)]
    pub icc_filename: String,
    pub brightness: f64,
    pub contrast: f64,
    #[serde(default = "default_rgb_gains")]
    pub rgb_gains: [f64; 3],
    pub gamma: f64,
    #[serde(default = "default_vibrance")]
    pub vibrance: f64,
    #[serde(default = "default_saturation")]
    pub saturation: f64,
    #[serde(default = "default_hue")]
    pub hue_deg: f64,
    #[serde(default = "default_color_model")]
    pub color_model: String,
    #[serde(default)]
    pub color_tag: Option<String>,
    #[serde(default)]
    pub tag_shape: Option<String>,
    #[serde(default)]
    pub attached_programs: Vec<String>,
    #[serde(default)]
    pub shortcut: Option<String>,
}

/// Self-contained `.chromadeck.json` bundle file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetBundle {
    pub schema_version: u32,
    pub exported_at: String,
    #[serde(default)]
    pub app_version: String,
    pub presets: Vec<ExportedPreset>,
    /// Lowercase hex-encoded raw `.icc` file bytes keyed by lowercase SHA-256 hex digest.
    #[serde(default)]
    pub icc_blobs: HashMap<String, String>,
}

/// Returned by `export_presets_cmd`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportSummary {
    pub exported_count: usize,
    pub icc_count: usize,
    pub dest_path: String,
}

/// Returned by `import_presets_cmd` — honestly reports renames and stripped conflicts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportSummary {
    pub imported_count: usize,
    pub icc_imported_count: usize,
    pub renamed_collisions: Vec<String>,
    pub stripped_conflicting_programs: Vec<String>,
    pub stripped_conflicting_shortcuts: Vec<String>,
}

/// Build an in-memory bundle for `ids` (`None` exports the entire preset library).
pub fn build_bundle(
    store: &Store,
    ids: Option<&[String]>,
    exported_at: String,
) -> Result<PresetBundle, StoreError> {
    let all = store.list_presets();
    let selected: Vec<Preset> = match ids {
        None => all,
        Some(wanted) => {
            if wanted.is_empty() {
                return Err(StoreError::InvalidInput(
                    "select at least one preset to export".into(),
                ));
            }
            let mut picked = Vec::with_capacity(wanted.len());
            for id in wanted {
                let preset = all
                    .iter()
                    .find(|p| p.id == *id)
                    .ok_or_else(|| StoreError::NotFound(id.clone()))?;
                picked.push(preset.clone());
            }
            picked
        }
    };

    if selected.is_empty() {
        return Err(StoreError::InvalidInput("no presets to export".into()));
    }

    let mut icc_blobs: HashMap<String, String> = HashMap::new();
    let mut presets = Vec::with_capacity(selected.len());

    for preset in selected {
        if !preset.icc_hash.is_empty() && !icc_blobs.contains_key(&preset.icc_hash) {
            let icc_path = store.profiles_dir().join(format!("{}.icc", preset.icc_hash));
            let bytes = std::fs::read(&icc_path).map_err(|e| {
                StoreError::InvalidInput(format!(
                    "missing ICC profile {} for \"{}\": {e}",
                    preset.icc_hash, preset.name
                ))
            })?;
            let actual = hex::encode(Sha256::digest(&bytes));
            if actual != preset.icc_hash.to_ascii_lowercase() {
                return Err(StoreError::InvalidInput(format!(
                    "ICC profile {} for \"{}\" failed integrity check",
                    preset.icc_hash, preset.name
                )));
            }
            icc_blobs.insert(actual, hex::encode(&bytes));
        }

        presets.push(ExportedPreset {
            name: preset.name,
            icc_hash: preset.icc_hash.to_ascii_lowercase(),
            icc_filename: preset.icc_filename,
            brightness: preset.brightness,
            contrast: preset.contrast,
            rgb_gains: preset.rgb_gains,
            gamma: preset.gamma,
            vibrance: preset.vibrance,
            saturation: preset.saturation,
            hue_deg: preset.hue_deg,
            color_model: preset.color_model,
            color_tag: preset.color_tag,
            tag_shape: preset.tag_shape,
            attached_programs: preset.attached_programs,
            shortcut: preset.shortcut,
        });
    }

    Ok(PresetBundle {
        schema_version: BUNDLE_SCHEMA_VERSION,
        exported_at,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        presets,
        icc_blobs,
    })
}

/// Export presets to `dest_path` atomically (write `.tmp`, then rename).
pub fn export_bundle_to_path(
    store: &Store,
    ids: Option<&[String]>,
    dest_path: &Path,
) -> Result<ExportSummary, StoreError> {
    let bundle = build_bundle(store, ids, crate::diagnostics::timestamp_now_utc())?;
    let json = serde_json::to_string_pretty(&bundle)?;
    if let Some(parent) = dest_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let tmp = dest_path.with_extension("chromadeck.tmp");
    std::fs::write(&tmp, json.as_bytes())?;
    if let Err(err) = std::fs::rename(&tmp, dest_path) {
        // Fallback when target exists on Windows older filesystems or cross-volume.
        std::fs::write(dest_path, json.as_bytes()).map_err(|_| err)?;
        let _ = std::fs::remove_file(&tmp);
    }

    Ok(ExportSummary {
        exported_count: bundle.presets.len(),
        icc_count: bundle.icc_blobs.len(),
        dest_path: dest_path.to_string_lossy().to_string(),
    })
}

fn validate_exported_preset(p: &ExportedPreset) -> Result<(), StoreError> {
    if p.name.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "bundle contains a preset with an empty name".into(),
        ));
    }
    if !(0.0..=100.0).contains(&p.brightness) {
        return Err(StoreError::InvalidInput(format!(
            "preset \"{}\": brightness must be in 0.0..=100.0",
            p.name
        )));
    }
    if !(0.0..=100.0).contains(&p.contrast) {
        return Err(StoreError::InvalidInput(format!(
            "preset \"{}\": contrast must be in 0.0..=100.0",
            p.name
        )));
    }
    if !(0.3..=2.8).contains(&p.gamma) {
        return Err(StoreError::InvalidInput(format!(
            "preset \"{}\": gamma must be in 0.3..=2.8",
            p.name
        )));
    }
    if !(0.0..=100.0).contains(&p.vibrance) {
        return Err(StoreError::InvalidInput(format!(
            "preset \"{}\": vibrance must be in 0.0..=100.0",
            p.name
        )));
    }
    if !(0.0..=100.0).contains(&p.saturation) {
        return Err(StoreError::InvalidInput(format!(
            "preset \"{}\": saturation must be in 0.0..=100.0",
            p.name
        )));
    }
    if !(0.0..=359.0).contains(&p.hue_deg) {
        return Err(StoreError::InvalidInput(format!(
            "preset \"{}\": hue_deg must be in 0.0..=359.0",
            p.name
        )));
    }
    for &g in &p.rgb_gains {
        if !(0.0..=10.0).contains(&g) {
            return Err(StoreError::InvalidInput(format!(
                "preset \"{}\": rgb_gains values must be in 0.0..=10.0",
                p.name
            )));
        }
    }
    if let Some(ref tag) = p.color_tag {
        if !Store::is_valid_color_tag_str(tag) {
            return Err(StoreError::InvalidInput(format!(
                "preset \"{}\": invalid color_tag {tag:?}",
                p.name
            )));
        }
    }
    if let Some(ref shape) = p.tag_shape {
        if !Store::is_valid_tag_shape_str(shape) {
            return Err(StoreError::InvalidInput(format!(
                "preset \"{}\": invalid tag_shape {shape:?}",
                p.name
            )));
        }
    }
    Ok(())
}

/// Resolve a non-colliding preset name (case-insensitive) against `used_names`.
fn unique_import_name(original: &str, used_names: &mut HashSet<String>) -> (String, bool) {
    let trimmed = original.trim();
    if !used_names.contains(&trimmed.to_ascii_lowercase()) {
        used_names.insert(trimmed.to_ascii_lowercase());
        return (trimmed.to_string(), false);
    }
    let first = format!("{trimmed} (imported)");
    if !used_names.contains(&first.to_ascii_lowercase()) {
        used_names.insert(first.to_ascii_lowercase());
        return (first, true);
    }
    let mut n = 2usize;
    loop {
        let candidate = format!("{trimmed} (imported {n})");
        if !used_names.contains(&candidate.to_ascii_lowercase()) {
            used_names.insert(candidate.to_ascii_lowercase());
            return (candidate, true);
        }
        n += 1;
    }
}

/// Validate and import a parsed `PresetBundle` into `store`.
pub fn import_bundle(store: &mut Store, bundle: PresetBundle) -> Result<ImportSummary, StoreError> {
    store.ensure_writable_public()?;
    if bundle.schema_version != BUNDLE_SCHEMA_VERSION {
        return Err(StoreError::InvalidInput(format!(
            "unsupported bundle schema_version {} (expected {BUNDLE_SCHEMA_VERSION})",
            bundle.schema_version
        )));
    }
    if bundle.presets.is_empty() {
        return Err(StoreError::InvalidInput(
            "bundle does not contain any presets".into(),
        ));
    }

    // Step 1: validate every preset and verify every referenced ICC blob up front
    // so a corrupted bundle fails cleanly before mutating any state on disk.
    let mut decoded_iccs: HashMap<String, Vec<u8>> = HashMap::new();
    for preset in &bundle.presets {
        validate_exported_preset(preset)?;
        let hash = preset.icc_hash.trim().to_ascii_lowercase();
        if hash.is_empty() {
            continue;
        }
        if decoded_iccs.contains_key(&hash) {
            continue;
        }
        let hex_blob = bundle
            .icc_blobs
            .get(&hash)
            .or_else(|| {
                bundle
                    .icc_blobs
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&hash))
                    .map(|(_, v)| v)
            })
            .ok_or_else(|| {
                StoreError::InvalidInput(format!(
                    "bundle is missing ICC data for \"{}\" (hash {hash})",
                    preset.name
                ))
            })?;
        let bytes = hex::decode(hex_blob.trim()).map_err(|e| {
            StoreError::InvalidInput(format!("corrupt ICC hex encoding for {hash}: {e}"))
        })?;
        if bytes.is_empty() {
            return Err(StoreError::InvalidInput(format!(
                "embedded ICC blob for {hash} is empty"
            )));
        }
        let actual_hash = hex::encode(Sha256::digest(&bytes));
        if actual_hash != hash {
            return Err(StoreError::InvalidInput(format!(
                "ICC integrity check failed for \"{}\": expected {hash}, got {actual_hash}",
                preset.name
            )));
        }
        decoded_iccs.insert(hash, bytes);
    }

    // Step 2: write verified ICC blobs into store profiles directory.
    let mut icc_imported_count = 0usize;
    for (hash, bytes) in &decoded_iccs {
        if store.import_icc_bytes(hash, bytes)? {
            icc_imported_count += 1;
        }
    }

    // Step 3: resolve name collisions, program conflicts, and shortcut conflicts.
    let existing = store.list_presets();
    let mut used_names: HashSet<String> = existing
        .iter()
        .map(|p| p.name.trim().to_ascii_lowercase())
        .collect();
    let mut owned_programs: HashSet<String> = existing
        .iter()
        .flat_map(|p| p.attached_programs.iter().cloned())
        .collect();
    let mut owned_shortcuts: HashSet<String> = existing
        .iter()
        .filter_map(|p| p.shortcut.clone())
        .collect();
    let globals = store.global_hotkeys();
    if let Some(s) = globals.reapply_shortcut {
        owned_shortcuts.insert(s);
    }
    if let Some(s) = globals.cycle_shortcut {
        owned_shortcuts.insert(s);
    }

    let mut renamed_collisions = Vec::new();
    let mut stripped_conflicting_programs = Vec::new();
    let mut stripped_conflicting_shortcuts = Vec::new();
    let mut new_presets = Vec::with_capacity(bundle.presets.len());

    for raw in bundle.presets {
        let (final_name, renamed) = unique_import_name(&raw.name, &mut used_names);
        if renamed {
            renamed_collisions.push(format!("{} -> {}", raw.name.trim(), final_name));
        }

        let mut programs = Vec::new();
        for prog in &raw.attached_programs {
            let Some(norm) = crate::auto_apply::normalize_program_name(prog) else {
                continue;
            };
            if programs.contains(&norm) {
                continue;
            }
            if owned_programs.contains(&norm) {
                stripped_conflicting_programs.push(format!("{norm} ({final_name})"));
                continue;
            }
            owned_programs.insert(norm.clone());
            programs.push(norm);
        }

        let shortcut = match raw
            .shortcut
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            None => None,
            Some(sc) => match crate::hotkeys::normalize_shortcut(sc) {
                Ok(canonical) => {
                    if owned_shortcuts.contains(&canonical) {
                        stripped_conflicting_shortcuts.push(format!("{canonical} ({final_name})"));
                        None
                    } else {
                        owned_shortcuts.insert(canonical.clone());
                        Some(canonical)
                    }
                }
                Err(_) => {
                    stripped_conflicting_shortcuts.push(format!("{sc} ({final_name})"));
                    None
                }
            },
        };

        let icc_hash = raw.icc_hash.trim().to_ascii_lowercase();
        let icc_filename = if icc_hash.is_empty() {
            String::new()
        } else if raw.icc_filename.trim().is_empty() {
            format!("{icc_hash}.icc")
        } else {
            raw.icc_filename
        };

        new_presets.push(Preset {
            id: uuid::Uuid::new_v4().to_string(),
            name: final_name,
            icc_hash,
            icc_filename,
            brightness: raw.brightness,
            contrast: raw.contrast,
            rgb_gains: raw.rgb_gains,
            gamma: raw.gamma,
            vibrance: raw.vibrance,
            saturation: raw.saturation,
            hue_deg: raw.hue_deg,
            color_model: if raw.color_model.trim().is_empty() {
                "nvcp-v1".into()
            } else {
                raw.color_model
            },
            color_tag: raw.color_tag.filter(|t| !t.is_empty()),
            tag_shape: Store::normalise_tag_shape_opt(raw.tag_shape),
            attached_programs: programs,
            shortcut,
        });
    }

    let imported_count = new_presets.len();
    store.insert_imported_presets(new_presets)?;

    Ok(ImportSummary {
        imported_count,
        icc_imported_count,
        renamed_collisions,
        stripped_conflicting_programs,
        stripped_conflicting_shortcuts,
    })
}

/// Read a `.chromadeck.json` file from disk and import it into `store`.
pub fn import_bundle_from_path(
    store: &mut Store,
    src_path: &Path,
) -> Result<ImportSummary, StoreError> {
    let text = std::fs::read_to_string(src_path)?;
    let bundle: PresetBundle = serde_json::from_str(&text)?;
    import_bundle(store, bundle)
}

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn export_presets_cmd(
    state: tauri::State<'_, AppStore>,
    ids: Option<Vec<String>>,
    dest_path: String,
) -> Result<ExportSummary, String> {
    let store = state.0.lock().map_err(|e| e.to_string())?;
    export_bundle_to_path(&store, ids.as_deref(), Path::new(&dest_path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_presets_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppStore>,
    src_path: String,
) -> Result<ImportSummary, String> {
    let summary = {
        let mut store = state.0.lock().map_err(|e| e.to_string())?;
        import_bundle_from_path(&mut store, Path::new(&src_path)).map_err(|e| e.to_string())?
    };
    crate::tray::refresh_tray(&app);
    crate::hotkeys::sync_hotkeys(&app);
    let _ = app.emit("store://changed", ());
    Ok(summary)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::PresetInput;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "chromadeck-bundle-{label}-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn base_input(name: &str) -> PresetInput {
        PresetInput {
            name: name.into(),
            icc_path: None,
            clear_icc: false,
            brightness: 54.0,
            contrast: 58.0,
            rgb_gains: [1.05, 1.0, 0.92],
            gamma: 1.15,
            vibrance: 68.0,
            saturation: 60.0,
            hue_deg: 12.0,
            color_tag: Some("#ff5500".into()),
            tag_shape: Some("diamond".into()),
            attached_programs: vec!["cs2.exe".into()],
            shortcut: Some("Ctrl+Alt+F1".into()),
        }
    }

    #[test]
    fn round_trip_export_and_import_preserves_icc_and_all_fields() {
        let src_dir = TempDir::new("rt-src");
        let dst_dir = TempDir::new("rt-dst");
        let icc_file = src_dir.path().join("sample.icc");
        let icc_bytes = b"fake-icc-profile-payload-bytes-for-sha256-test";
        std::fs::write(&icc_file, icc_bytes).unwrap();

        let mut src_store = Store::new(src_dir.path().to_path_buf()).unwrap();
        let mut inp = base_input("Competitive - CS2");
        inp.icc_path = Some(icc_file.to_string_lossy().to_string());
        let created = src_store.create_preset(inp).unwrap();

        let bundle_path = src_dir.path().join("export.chromadeck.json");
        let exp = export_bundle_to_path(&src_store, None, &bundle_path).unwrap();
        assert_eq!(exp.exported_count, 1);
        assert_eq!(exp.icc_count, 1);

        let mut dst_store = Store::new(dst_dir.path().to_path_buf()).unwrap();
        let imp = import_bundle_from_path(&mut dst_store, &bundle_path).unwrap();
        assert_eq!(imp.imported_count, 1);
        assert_eq!(imp.icc_imported_count, 1);
        assert!(imp.renamed_collisions.is_empty());
        assert!(imp.stripped_conflicting_programs.is_empty());
        assert!(imp.stripped_conflicting_shortcuts.is_empty());

        let presets = dst_store.list_presets();
        assert_eq!(presets.len(), 1);
        let p = &presets[0];
        assert_eq!(p.name, "Competitive - CS2");
        assert_eq!(p.icc_hash, created.icc_hash);
        assert_eq!(p.rgb_gains, [1.05, 1.0, 0.92]);
        assert_eq!(p.attached_programs, vec!["cs2.exe"]);
        assert_eq!(p.shortcut.as_deref(), Some("Ctrl+Alt+F1"));
        assert!(dst_store
            .profiles_dir()
            .join(format!("{}.icc", p.icc_hash))
            .exists());
    }

    #[test]
    fn import_rejects_corrupted_icc_hash_without_modifying_store() {
        let dir = TempDir::new("corrupt-icc");
        let mut store = Store::new(dir.path().to_path_buf()).unwrap();

        let fake_hash = hex::encode(Sha256::digest(b"expected-icc"));
        let mut icc_blobs = HashMap::new();
        icc_blobs.insert(fake_hash.clone(), hex::encode(b"tampered-icc"));

        let bundle = PresetBundle {
            schema_version: BUNDLE_SCHEMA_VERSION,
            exported_at: "2026-11-26T00:00:00Z".into(),
            app_version: "0.9.8".into(),
            presets: vec![ExportedPreset {
                name: "Tampered".into(),
                icc_hash: fake_hash,
                icc_filename: "tampered.icc".into(),
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
            }],
            icc_blobs,
        };

        let err = import_bundle(&mut store, bundle).unwrap_err();
        assert!(err.to_string().contains("integrity check failed"));
        assert!(store.list_presets().is_empty());
    }

    #[test]
    fn import_renames_collisions_and_strips_conflicting_programs_and_shortcuts() {
        let dir = TempDir::new("collisions");
        let mut store = Store::new(dir.path().to_path_buf()).unwrap();
        store.create_preset(base_input("FPS Boost")).unwrap();

        let bundle = build_bundle(&store, None, "2026-11-26T00:00:00Z".into()).unwrap();
        let summary = import_bundle(&mut store, bundle).unwrap();

        assert_eq!(summary.imported_count, 1);
        assert_eq!(summary.renamed_collisions, vec!["FPS Boost -> FPS Boost (imported)"]);
        assert_eq!(
            summary.stripped_conflicting_programs,
            vec!["cs2.exe (FPS Boost (imported))"]
        );
        assert_eq!(
            summary.stripped_conflicting_shortcuts,
            vec!["Ctrl+Alt+F1 (FPS Boost (imported))"]
        );

        let presets = store.list_presets();
        assert_eq!(presets.len(), 2);
        let imported = &presets[1];
        assert_eq!(imported.name, "FPS Boost (imported)");
        assert!(imported.attached_programs.is_empty());
        assert_eq!(imported.shortcut, None);
    }

    #[test]
    fn export_single_preset_subset_only_includes_selected_preset_and_its_icc() {
        let dir = TempDir::new("subset");
        let mut store = Store::new(dir.path().to_path_buf()).unwrap();
        let p1 = store.create_preset(base_input("One")).unwrap();
        let mut inp2 = base_input("Two");
        inp2.attached_programs = vec!["vlc.exe".into()];
        inp2.shortcut = None;
        let _p2 = store.create_preset(inp2).unwrap();

        let bundle = build_bundle(&store, Some(&[p1.id.clone()]), "2026-11-26T00:00:00Z".into())
            .unwrap();
        assert_eq!(bundle.presets.len(), 1);
        assert_eq!(bundle.presets[0].name, "One");
    }
}
