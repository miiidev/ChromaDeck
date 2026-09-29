// ── Preset enforcer: resident drift-check + reapply ──────────────────────────

use std::collections::HashMap;
use serde::Serialize;

/// Outcome of one enforced (or skipped) pin.
#[derive(Debug, Clone, Serialize)]
pub struct EnforceEvent {
    pub edid_id: String,
    pub preset_id: String,
    pub applied: bool,
    pub error: Option<String>,
}

/// Hardware state reads, isolated for testability.
pub trait StateReader {
    fn read_lut(&self, edid_id: &str) -> Result<[u16; 768], String>;
    fn read_color(&self, edid_id: &str) -> Result<(f64, f64), String>;
}

pub struct RealStateReader;

impl StateReader for RealStateReader {
    fn read_lut(&self, edid_id: &str) -> Result<[u16; 768], String> {
        let device = crate::nvapi::resolve_device_name(edid_id)
            .ok_or_else(|| "unknown monitor".to_string())?;
        crate::color::read_gamma_ramp(&device)
    }

    fn read_color(&self, edid_id: &str) -> Result<(f64, f64), String> {
        crate::nvapi::read_levels(edid_id)
    }
}

/// Pure drift-check + conditional apply over snapshots.  `force` reapplies
/// regardless of drift (manual Reapply-now path).
pub fn check_once(
    api: &dyn crate::color::ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    reader: &dyn StateReader,
    pins: &HashMap<String, String>,
    presets: &[crate::store::Preset],
    force: bool,
) -> Vec<EnforceEvent> {
    let mut events = Vec::new();
    for (edid_id, preset_id) in pins {
        let preset = match presets.iter().find(|p| &p.id == preset_id) {
            Some(p) => p,
            None => continue, // stale pin (preset gone without cascade): skip
        };
        if preset.edid_id != *edid_id {
            continue; // pin/preset mismatch: skip, never apply cross-monitor
        }
        if !api.is_connected(edid_id) {
            events.push(EnforceEvent {
                edid_id: edid_id.clone(),
                preset_id: preset_id.clone(),
                applied: false,
                error: None,
            });
            continue;
        }

        let drifted = force
            || lut_drifted(preset, reader, edid_id)
            || color_drifted(preset, reader, edid_id);

        if !drifted {
            events.push(EnforceEvent {
                edid_id: edid_id.clone(),
                preset_id: preset_id.clone(),
                applied: false,
                error: None,
            });
            continue;
        }

        let result = crate::color::apply_color(api, nv, preset);
        events.push(EnforceEvent {
            edid_id: edid_id.clone(),
            preset_id: preset_id.clone(),
            applied: result.gamma_applied || result.vibrance_applied,
            error: result.error,
        });
    }
    events
}

fn lut_drifted(preset: &crate::store::Preset, reader: &dyn StateReader, edid_id: &str) -> bool {
    match reader.read_lut(edid_id) {
        Ok(live) => {
            live != crate::color::build_gamma_ramp(
                preset.brightness,
                preset.contrast,
                preset.rgb_gains,
                preset.gamma,
            )
        }
        Err(_) => true, // unreadable state counts as drift (try to heal)
    }
}

fn color_drifted(preset: &crate::store::Preset, reader: &dyn StateReader, edid_id: &str) -> bool {
    if preset.vibrance == crate::nvapi::VIBRANCE_NEUTRAL
        && preset.hue_deg == crate::nvapi::HUE_NEUTRAL
    {
        return false; // unmanaged when neutral (mirrors apply skip rule)
    }
    match reader.read_color(edid_id) {
        Ok((v, h)) => v != preset.vibrance || h != preset.hue_deg,
        Err(_) => true,
    }
}

/// Background task: check every 10s.  Spawned once from setup (Task 4).
pub async fn enforce_loop(app: tauri::AppHandle) {
    loop {
        check_and_enforce(&app);
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}

fn check_and_enforce(app: &tauri::AppHandle) {
    use tauri::Manager;
    let (pins, presets) = match app.try_state::<crate::store::AppStore>() {
        Some(s) => match s.0.lock() {
            Ok(store) => (store.list_pins(), store.list_presets()),
            Err(_) => return,
        },
        None => return,
    };
    if pins.is_empty() {
        return;
    }
    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let reader = RealStateReader;
    let _ = check_once(&api, &nv, &reader, &pins, &presets, false);
}

#[tauri::command]
pub fn reapply_now_cmd(state: tauri::State<'_, crate::store::AppStore>) -> Vec<EnforceEvent> {
    let (pins, presets) = match state.0.lock() {
        Ok(store) => (store.list_pins(), store.list_presets()),
        Err(e) => {
            return vec![EnforceEvent {
                edid_id: String::new(),
                preset_id: String::new(),
                applied: false,
                error: Some(format!("store lock: {e}")),
            }]
        }
    };
    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let reader = RealStateReader;
    check_once(&api, &nv, &reader, &pins, &presets, true)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{build_gamma_ramp, TestRecorder};
    use crate::nvapi::MockNvapi;

    // MockStateReader for tests
    pub struct MockStateReader {
        pub lut: [u16; 768],
        pub color: (f64, f64),
        pub fail: bool,
    }

    impl MockStateReader {
        /// Identity ramp + neutral color (50.0, 0.0), no fail.
        pub fn clean() -> Self {
            MockStateReader {
                lut: build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0),
                color: (50.0, 0.0),
                fail: false,
            }
        }
    }

    impl StateReader for MockStateReader {
        fn read_lut(&self, _edid_id: &str) -> Result<[u16; 768], String> {
            if self.fail {
                Err("mock failure".into())
            } else {
                Ok(self.lut)
            }
        }

        fn read_color(&self, _edid_id: &str) -> Result<(f64, f64), String> {
            if self.fail {
                Err("mock failure".into())
            } else {
                Ok(self.color)
            }
        }
    }

    /// Build a minimal preset for tests.
    fn p(id: &str, edid: &str, brightness: f64, vibrance: f64, hue: f64) -> crate::store::Preset {
        crate::store::Preset {
            id: id.into(),
            name: "test".into(),
            edid_id: edid.into(),
            icc_hash: String::new(),
            icc_filename: String::new(),
            brightness,
            contrast: 0.8,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 2.2,
            vibrance,
            hue_deg: hue,
        }
    }

    // ── Test 1: no_pins_no_calls ──────────────────────────────────────────

    #[test]
    fn no_pins_no_calls() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let reader = MockStateReader::clean();
        let events = check_once(
            &color,
            &nv,
            &reader,
            &HashMap::new(),
            &[p("a", "E", 1.0, 50.0, 0.0)],
            false,
        );
        assert!(events.is_empty(), "no pins → no events");
        assert!(
            color.calls.lock().unwrap().is_empty(),
            "no driver calls with no pins"
        );
    }

    // ── Test 2: in_sync_preset_issues_no_writes ───────────────────────────

    #[test]
    fn in_sync_preset_issues_no_writes() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = p("p1", "E", 0.7, 50.0, 0.0);
        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        // Reader returns matching values
        let reader = MockStateReader {
            lut: build_gamma_ramp(
                preset.brightness,
                preset.contrast,
                preset.rgb_gains,
                preset.gamma,
            ),
            color: (preset.vibrance, preset.hue_deg),
            fail: false,
        };

        let events = check_once(&color, &nv, &reader, &pins, &[preset], false);
        assert_eq!(events.len(), 1);
        assert!(!events[0].applied, "in-sync → no apply");
        assert!(events[0].error.is_none());
        assert!(
            color.calls.lock().unwrap().is_empty(),
            "no driver calls when in sync"
        );
        assert!(
            nv.calls.lock().unwrap().is_empty(),
            "no nv calls when in sync"
        );
    }

    // ── Test 3: drifted_lut_reapplies_gamma_only_when_neutral ────────────

    #[test]
    fn drifted_lut_reapplies_gamma_only_when_neutral() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = p("p1", "E", 0.5, 50.0, 0.0); // non-default brightness, neutral v/h
        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        // Reader returns identity ramp (differs from preset's ramp)
        let reader = MockStateReader::clean();

        let events = check_once(&color, &nv, &reader, &pins, &[preset], false);
        assert_eq!(events.len(), 1);
        assert!(events[0].applied, "drifted LUT → should apply");
        assert!(events[0].error.is_none());

        let calls = color.calls.lock().unwrap();
        assert_eq!(*calls, vec!["gamma"], "gamma must be called");

        assert!(
            nv.calls.lock().unwrap().is_empty(),
            "no nv calls for neutral vibrance"
        );
    }

    // ── Test 4: drifted_vibrance_reapplies ────────────────────────────

    #[test]
    fn drifted_vibrance_reapplies() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = p("p1", "E", 1.0, 75.0, 0.0); // non-neutral vibrance
        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        // Reader returns matching LUT but neutral color (vibrance differs)
        let reader = MockStateReader {
            lut: build_gamma_ramp(1.0, 0.8, [1.0, 1.0, 1.0], 2.2),
            color: (50.0, 0.0), // neutral — preset has 75.0
            fail: false,
        };

        let events = check_once(&color, &nv, &reader, &pins, &[preset], false);
        assert_eq!(events.len(), 1);
        assert!(events[0].applied, "drifted vbrance → should apply");

        // NV calls should be [(set, 75.0, 0.0)]
        let nv_calls = nv.calls.lock().unwrap();
        assert_eq!(nv_calls.len(), 1, "one nv call expected");
        assert_eq!(
            nv_calls[0],
            ("set".to_string(), 75.0, 0.0),
            "call should set vibrance=75.0 hue=0.0"
        );

        // Gamma should also be called (apply_color always applies gamma)
        let color_calls = color.calls.lock().unwrap();
        assert!(color_calls.contains(&"gamma".to_string()));
    }

    // ── Test 5: offline_monitor_skipped ────────────────────────────────

    #[test]
    fn offline_monitor_skipped() {
        let color = TestRecorder::new(false); // offline
        let nv = MockNvapi::new(false); // unsupported
        let preset = p("p1", "E", 1.0, 75.0, 0.0);
        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());
        let reader = MockStateReader::clean();

        let events = check_once(&color, &nv, &reader, &pins, &[preset], false);
        assert_eq!(events.len(), 1);
        assert!(!events[0].applied, "offline → not applied");
        assert!(events[0].error.is_none()); // offline is silently skipped

        // No driver calls
        assert!(color.calls.lock().unwrap().is_empty());
        assert!(nv.calls.lock().unwrap().is_empty());
    }

    // ── Test 6: stale_pin_and_missing_preset_skipped ────────────────────

    #[test]
    fn stale_pin_and_missing_preset_skipped() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let reader = MockStateReader::clean();

        // Pins referencing unknown preset and unknown edid
        let mut pins = HashMap::new();
        pins.insert("unknown-edid".to_string(), "unknown-preset".to_string());
        pins.insert("edid-2".to_string(), "also-missing".to_string());

        // Only one preset in the store, not matching any pin
        let presets = &[p("a", "real", 1.0, 50.0, 0.0)];

        let events = check_once(&color, &nv, &reader, &pins, presets, false);
        assert!(events.is_empty(), "no matching pins → no events");

        // No driver calls
        assert!(color.calls.lock().unwrap().is_empty());
        assert!(nv.calls.lock().unwrap().is_empty());
    }

    // ── Test 7: force_reapplies_despite_sync ─────────────────

    #[test]
    fn force_reapplies_despite_sync() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = p("p1", "E", 1.0, 50.0, 0.0); // neutral
        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        // In-sync state: reader matches preset exactly
        let reader = MockStateReader {
            lut: build_gamma_ramp(1.0, 0.8, [1.0, 1.0, 1.0], 2.2),
            color: (50.0, 0.0),
            fail: false,
        };

        // force = true
        let events = check_once(&color, &nv, &reader, &pins, &[preset], true);
        assert_eq!(events.len(), 1);
        assert!(events[0].applied, "force=true → should apply");
        assert!(events[0].error.is_none());

        // Driver calls should happen
        let calls = color.calls.lock().unwrap();
        assert!(!calls.is_empty(), "color should be called when forced");
    }
}