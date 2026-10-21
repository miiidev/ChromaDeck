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
    /// Read NVCP registry B/C/G channel means (internal scale, 80–120).
    /// Returns `None` when NVAPI or registry is unreachable.
    fn read_nvcp_registry(&self, edid_id: &str) -> Option<(f64, f64, f64)>;
    /// The gamma engine active for this display (NVAPI or GDI).
    fn engine(&self, edid_id: &str) -> crate::color::GammaEngine;
    /// Whether this display is routed to the Intel IGCL backend (pixtx). The
    /// GDI/NVAPI drift paths are inapplicable to Intel: Intel apply purges the
    /// GDI ramp to identity and drives a pixtx LUT, so a GDI read-back would
    /// always look "drifted" and re-apply every cycle.
    fn is_intel(&self, edid_id: &str) -> bool;
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

    fn read_nvcp_registry(&self, edid_id: &str) -> Option<(f64, f64, f64)> {
        let fns = crate::nvapi::fns()?;
        let device = crate::nvapi::resolve_device_name(edid_id)?;
        let display_id = crate::nvapi::display_id_for_device(fns, &device).ok()?;
        let luid = crate::nvgamma::display_luid(fns, display_id).ok()?;
        Some(crate::nvgamma::read_nvcp(luid))
    }

    fn engine(&self, edid_id: &str) -> crate::color::GammaEngine {
        crate::color::gamma_engine(edid_id)
    }

    fn is_intel(&self, edid_id: &str) -> bool {
        // Cached negative verdicts avoid a DriverStore re-scan every drift
        // cycle on non-Intel machines (L-4). The uncached path stays for
        // backend selection, which must remain hot-plug correct.
        crate::backend::intel::correlates_to_intel_cached(edid_id)
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
            || match reader.engine(edid_id) {
                crate::color::GammaEngine::Nvapi => nvapi_drifted(preset, nv, reader, edid_id),
                crate::color::GammaEngine::Gdi => {
                    // Intel displays report the GDI engine (`gamma_engine`
                    // probes NVAPI, which Intel lacks), but Intel apply drives
                    // a pixtx LUT and purges the GDI ramp to identity. A GDI
                    // ramp read-back would therefore always differ from the
                    // expected preset ramp and force a re-apply every 10s
                    // (H1). A real Intel drift check would read the pixtx 1D
                    // LUT and compare against the composed expectation — but
                    // the pixtx 3×3 matrix (saturation/hue/rgb_gains) is NOT
                    // readable on this driver (spec §6/§10), so even a "real"
                    // check is partial, and running a full output-enumeration +
                    // pixtx read + compose each 10s cycle is disproportionate
                    // for that partial signal. Honest interim: skip drift
                    // enforcement for Intel and document the gap — an in-sync
                    // Intel display is never re-applied, and a genuinely
                    // drifted one is healed only by an explicit re-apply
                    // (`reapply_now_cmd`, force=true) or restart. pixtx state
                    // is runtime-only driver state (a reboot resets it), and
                    // only ChromaDeck writes it, so the foreign-clobber case
                    // the loop exists for is rare for Intel.
                    if reader.is_intel(edid_id) {
                        false
                    } else {
                        lut_drifted(preset, reader, edid_id)
                            || color_drifted(preset, nv, reader, edid_id)
                    }
                }
            };

        if !drifted {
            events.push(EnforceEvent {
                edid_id: edid_id.clone(),
                preset_id: preset_id.clone(),
                applied: false,
                error: None,
            });
            continue;
        }

        let result = crate::color::apply_color(api, nv, preset, edid_id);
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

fn color_drifted(
    preset: &crate::store::Preset,
    nv: &dyn crate::nvapi::NvColorApi,
    reader: &dyn StateReader,
    edid_id: &str,
) -> bool {
    let neutral = preset.vibrance == crate::nvapi::VIBRANCE_NEUTRAL
        && preset.hue_deg == crate::nvapi::HUE_NEUTRAL;
    if neutral && !nv.supported(edid_id) {
        return false; // unmanaged where unsupported: silent, no error
    }
    match reader.read_color(edid_id) {
        Ok((v, h)) => v != preset.vibrance || h != preset.hue_deg,
        Err(_) => true,
    }
}

/// Drift check for the NVAPI engine path: compare registry B/C/G (internal
/// scale) against preset-converted values. Then verify the GDI LUT is still
/// the identity ramp — exclusive-fullscreen games clobber only the GDI LUT
/// while leaving the NVAPI registry untouched, so registry-match alone would
/// miss the canonical evaporation case. Vibrance/hue are also checked.
fn nvapi_drifted(
    preset: &crate::store::Preset,
    nv: &dyn crate::nvapi::NvColorApi,
    reader: &dyn StateReader,
    edid_id: &str,
) -> bool {
    // Registry compare: B/C/G internal must equal preset's converted values.
    // Registry values are DWORDs written with rounding.  Compare rounded-to-u32
    // on both sides to avoid false-drift from f64 truncation (e.g. UI 37 →
    // internal 94.8 → DWORD 95 via round).
    match reader.read_nvcp_registry(edid_id) {
        Some((b, c, g)) => {
            let expected_b = crate::nvgamma::ui_to_internal(preset.brightness);
            let expected_c = crate::nvgamma::ui_to_internal(preset.contrast);
            let expected_g = preset.gamma * 100.0;
            if (b.round() as u32) != (expected_b.round() as u32)
                || (c.round() as u32) != (expected_c.round() as u32)
                || (g.round() as u32) != (expected_g.round() as u32)
            {
                return true; // registry drift
            }
        }
        None => return true, // unreachable registry → treat as drift (heal attempt)
    };

    // GDI LUT must be identity — NVAPI owns the color pipeline.  A foreign
    // writer (e.g. exclusive-fullscreen game) only touches the GDI LUT, so
    // a registry match alone would miss the evaporation entirely.
    let identity = crate::color::build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0);
    match reader.read_lut(edid_id) {
        Ok(live) => {
            if live != identity {
                return true; // foreign LUT writer detected
            }
        }
        Err(_) => return true, // unreadable → treat as drift
    };

    // Vibrance/hue check (same logic as GDI branch).
    color_drifted(preset, nv, reader, edid_id)
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

// ── Lifecycle: startup reassertion + exit semantics ──────────────────────────
//
// ChromaDeck's documented promise (README "Pin & enforce", PRODUCT "enforcement
// just holds", and the driver-registry persistence feature) is that applied
// colour SURVIVES a clean exit and is reasserted on the next launch. The app
// has never promised to wipe displays on quit; "Reset" is the explicit action
// that does that. Phase F therefore:
//
//   STARTUP — every connected display with a pin OR an applied record is
//   re-applied through the backend registry. If the recorded preset no longer
//   exists (removed while the app was closed), the display is restored through
//   the same backend path `reset_monitor` uses, and the stale records are
//   cleared. Disconnected displays are skipped untouched.
//
//   EXIT — policy is `KeepApplied` (see `ExitPolicy`). Per backend:
//     • nvapi — NVCP registry persistence is the documented feature; quitting
//       must not erase it.
//     • gdi   — the OS gamma ramp is session state; Windows clears it on
//       logoff/reboot. Nothing to do.
//     • intel — the pixtx pipeline is driver state and its unattended reset is
//       best-effort only (`ctlDisplayFeatureReset` is not exported on this
//       driver), so the exit path never claims a restore it cannot verify.
//   A `Restore` policy exists as a tested seam for an explicit
//   "quit and reset" flow, but the tray does NOT use it today.
//
// Shutdown never panics on a disconnected display: connectivity is checked
// first and a disconnected target is reported as skipped, with no write.

/// Divergence between persisted records and the store's presets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleAction {
    /// Preset still exists and the display is connected: re-apply it.
    Apply(String),
    /// Recorded preset no longer exists: restore the display to defaults.
    Restore(String),
    /// Display is disconnected: leave it (and its records) untouched.
    Skip,
}

/// One startup lifecycle outcome (also useful as an honest diagnostic log).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleEvent {
    pub edid_id: String,
    pub preset_id: Option<String>,
    /// "apply" | "restore" | "skip"
    pub action: String,
    pub applied: bool,
    pub error: Option<String>,
}

/// Merge pins and applied records into one target list, pins winning, sorted
/// by EDID for deterministic behaviour and tests.
pub fn lifecycle_targets(
    pins: &HashMap<String, String>,
    applied: &HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut targets = pins.clone();
    for (edid_id, preset_id) in applied {
        targets
            .entry(edid_id.clone())
            .or_insert_with(|| preset_id.clone());
    }
    let mut ordered: Vec<(String, String)> = targets.into_iter().collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    ordered
}

/// Pure startup plan. No hardware: connectivity is injected.
pub fn startup_plan(
    targets: &[(String, String)],
    presets: &[crate::store::Preset],
    connected: &dyn Fn(&str) -> bool,
) -> Vec<(String, LifecycleAction)> {
    targets
        .iter()
        .map(|(edid_id, preset_id)| {
            let action = if !connected(edid_id) {
                LifecycleAction::Skip
            } else if presets.iter().any(|p| &p.id == preset_id) {
                LifecycleAction::Apply(preset_id.clone())
            } else {
                LifecycleAction::Restore(preset_id.clone())
            };
            (edid_id.clone(), action)
        })
        .collect()
}

/// Execute the startup plan through the backend abstraction.
///
/// `is_intel` reports whether a display routes to the Intel IGCL backend.
/// Intel displays are NOT auto-reapplied at startup (see C1): pixtx state is
/// runtime-only driver state — a reboot resets it to factory default, while a
/// restart-without-reboot may still carry the previous session's applied
/// curve. A startup apply would capture whichever of those is present as the
/// "pre-apply" state, so a later Reset would return the panel to that
/// (possibly non-factory) state and falsely report success. Skipping the Intel
/// reapply keeps the restore guarantee honest: Reset on a display never
/// applied this session reports "no capture" rather than faking a restore.
///
/// As a side effect (M-4), Intel-correlated targets with a persisted record
/// are marked capture-untrusted via `intel::mark_capture_untrusted`, so a
/// later first apply cannot capture the leftover curve as its baseline.
pub fn run_startup(
    api: &dyn crate::color::ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    targets: &[(String, String)],
    presets: &[crate::store::Preset],
    is_intel: &dyn Fn(&str) -> bool,
) -> Vec<LifecycleEvent> {
    // M-4: mark every Intel-correlated target (a persisted applied/pin record)
    // as capture-untrusted for this session. The persisted record means the
    // pixtx state may be ChromaDeck's own leftover curve after a
    // restart-without-reboot; the first apply must not capture that leftover as
    // a trusted factory baseline. `IntelBackend::apply` refuses (or verifies
    // the LUT is the factory-identity default) accordingly. Displays with no
    // persisted record are never marked, so a fresh-boot/no-record apply is
    // unaffected.
    for (edid_id, _preset_id) in targets {
        if is_intel(edid_id) {
            crate::backend::intel::mark_capture_untrusted(edid_id);
        }
    }

    startup_plan(targets, presets, &|edid_id| api.is_connected(edid_id))
        .into_iter()
        .map(|(edid_id, action)| match action {
            LifecycleAction::Skip => LifecycleEvent {
                edid_id,
                preset_id: None,
                action: "skip".into(),
                applied: false,
                error: None,
            },
            LifecycleAction::Apply(preset_id) => {
                // Skip Intel auto-reapply at startup (documented above).
                if is_intel(&edid_id) {
                    return LifecycleEvent {
                        edid_id,
                        preset_id: Some(preset_id),
                        action: "skip".into(),
                        applied: false,
                        error: Some(
                            "skipped Intel startup reapply: pixtx state is runtime-only and \
                             capturing it here would poison Reset's restore guarantee (a reboot \
                             restores the factory default)"
                                .into(),
                        ),
                    };
                }
                let preset = match presets.iter().find(|p| p.id == preset_id) {
                    Some(p) => p,
                    None => {
                        return LifecycleEvent {
                            edid_id,
                            preset_id: Some(preset_id),
                            action: "apply".into(),
                            applied: false,
                            error: Some("preset disappeared during startup".into()),
                        }
                    }
                };
                let result = crate::color::apply_color(api, nv, preset, &edid_id);
                LifecycleEvent {
                    edid_id,
                    preset_id: Some(preset_id),
                    action: "apply".into(),
                    applied: result.gamma_applied || result.vibrance_applied,
                    error: result.error,
                }
            }
            LifecycleAction::Restore(preset_id) => {
                // Parity with the explicit Reset action: the backend owns the
                // reset path (GDI identity ramp, NVAPI identity+persist, Intel
                // RESTORE_DEFAULT).
                let result = crate::color::reset_monitor(api, nv, &edid_id);
                LifecycleEvent {
                    edid_id,
                    preset_id: Some(preset_id),
                    action: "restore".into(),
                    applied: result.gamma_applied || result.vibrance_applied,
                    error: result.error,
                }
            }
        })
        .collect()
}

/// Startup entry point: reassert pins/applied presets, restore stale records,
/// then clear those stale records. Runs off the main thread (spawned in setup).
pub fn startup_reapply(app: &tauri::AppHandle) {
    use tauri::Manager;
    let state = match app.try_state::<crate::store::AppStore>() {
        Some(s) => s,
        None => return,
    };
    let (targets, presets) = match state.0.lock() {
        Ok(store) => (
            lifecycle_targets(&store.list_pins(), &store.list_applied()),
            store.list_presets(),
        ),
        Err(_) => return,
    };
    if targets.is_empty() {
        return;
    }

    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let events = run_startup(
        &api,
        &nv,
        &targets,
        &presets,
        &|edid_id| crate::backend::intel::correlates_to_intel(edid_id),
    );

    let stale: Vec<String> = events
        .iter()
        .filter(|e| e.action == "restore")
        .map(|e| e.edid_id.clone())
        .collect();

    for event in &events {
        if let Some(error) = &event.error {
            eprintln!(
                "ChromaDeck startup: {} {} failed: {error}",
                event.action, event.edid_id
            );
        }
    }

    // A preset no longer exists, so its records can never apply again. Clear
    // them even when the restore write failed (the failure is logged): keeping
    // a ghost IN USE record would over-claim.
    if !stale.is_empty() {
        if let Ok(mut store) = state.0.lock() {
            for edid_id in stale {
                store.clear_applied(&edid_id);
                store.unpin_monitor(&edid_id);
            }
        }
    }
}

/// Quit policy. `KeepApplied` is the documented behaviour; `Restore` is the
/// tested seam for an explicit "quit and reset" flow and is not wired to the
/// tray today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitPolicy {
    /// Leave applied colour in place (documented: enforcement persists).
    KeepApplied,
    /// Explicitly restore every connected target through its backend.
    #[allow(dead_code)]
    Restore,
}

/// One display's exit outcome. `restored` is true only when a backend restore
/// reported success; `detail` states plainly what happened (or why nothing was
/// attempted) so logs never fake a cleanup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExitOutcome {
    pub edid_id: String,
    pub backend: String,
    pub restored: bool,
    pub detail: String,
}

/// Backend lookup seam so exit routing is testable without hardware.
pub trait BackendSelector {
    fn select_backend(&self, edid_id: &str) -> &dyn crate::backend::ColorBackend;

    /// Cheap backend identity for classification-only paths (`KeepApplied`).
    /// Implementations MUST NOT load a driver library; the default is
    /// deliberately non-committal so a test double without a hint still works.
    fn backend_id_hint(&self, _edid_id: &str) -> &'static str {
        "unknown"
    }
}

impl BackendSelector for crate::backend::BackendRegistry {
    fn select_backend(&self, edid_id: &str) -> &dyn crate::backend::ColorBackend {
        crate::backend::BackendRegistry::select(self, edid_id)
    }

    fn backend_id_hint(&self, edid_id: &str) -> &'static str {
        crate::backend::BackendRegistry::backend_id_hint(self, edid_id)
    }
}

fn keep_detail(backend_id: &str) -> &'static str {
    match backend_id {
        "nvapi" => {
            "kept: NVCP driver-registry persistence (only an explicit Reset clears it)"
        }
        "gdi" => "kept: OS gamma ramp is session state, cleared by Windows at logoff/reboot",
        "intel" => {
            "kept: Intel pixtx state is driver-level; unattended reset is best-effort only"
        }
        _ => "kept: applied colour persists by design",
    }
}

/// Compute/execute exit semantics per display.
///
/// With `KeepApplied` this is a pure classification: no backend write happens.
/// With `Restore` every connected target goes through its selected backend's
/// `restore`. Disconnected displays are skipped before any write, so a
/// display unplugged during shutdown cannot cause a failed driver call.
pub fn run_exit(
    policy: ExitPolicy,
    selector: &dyn BackendSelector,
    api: &dyn crate::color::ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    targets: &[(String, String)],
    connected: &dyn Fn(&str) -> bool,
) -> Vec<ExitOutcome> {
    let mut outcomes: Vec<ExitOutcome> = Vec::new();
    for (edid_id, _preset_id) in targets {
        // Connectivity first: never probe/load a driver for a display that
        // vanished during shutdown.
        if !connected(edid_id) {
            outcomes.push(ExitOutcome {
                edid_id: edid_id.clone(),
                backend: "n/a".into(),
                restored: false,
                detail: "skipped: display disconnected during shutdown (no write attempted)"
                    .into(),
            });
            continue;
        }
        match policy {
            // Classification only: never call `select_backend` here, because
            // selection probes NVAPI/IGCL and can stall tray quit. The cheap
            // hint never loads a driver library.
            ExitPolicy::KeepApplied => {
                let backend_id = selector.backend_id_hint(edid_id);
                outcomes.push(ExitOutcome {
                    edid_id: edid_id.clone(),
                    backend: backend_id.to_string(),
                    restored: false,
                    detail: keep_detail(backend_id).into(),
                });
            }
            ExitPolicy::Restore => {
                let backend = selector.select_backend(edid_id);
                let result = backend.restore(api, nv, edid_id);
                outcomes.push(ExitOutcome {
                    edid_id: edid_id.clone(),
                    backend: backend.id().to_string(),
                    restored: result.gamma_applied || result.vibrance_applied,
                    detail: result
                        .error
                        .unwrap_or_else(|| "restored through the selected backend".into()),
                });
            }
        }
    }
    outcomes
}

/// Tray-quit entry point. Documents the chosen policy in the log and never
/// writes to hardware; safe when a display disconnects mid-shutdown.
pub fn shutdown_sweep(app: &tauri::AppHandle) {
    use tauri::Manager;
    let state = match app.try_state::<crate::store::AppStore>() {
        Some(s) => s,
        None => return,
    };
    let targets = match state.0.lock() {
        Ok(store) => lifecycle_targets(&store.list_pins(), &store.list_applied()),
        Err(_) => return,
    };
    if targets.is_empty() {
        return;
    }

    let selector = crate::backend::BackendRegistry::new();
    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let outcomes = run_exit(
        ExitPolicy::KeepApplied,
        &selector,
        &api,
        &nv,
        &targets,
        &|edid_id| crate::color::ColorApi::is_connected(&api, edid_id),
    );
    for outcome in outcomes {
        eprintln!(
            "ChromaDeck exit: {} [{}] {}",
            outcome.edid_id, outcome.backend, outcome.detail
        );
    }
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
        pub nvcp_registry: Option<(f64, f64, f64)>,
        pub engine: crate::color::GammaEngine,
        pub is_intel: bool,
    }

    impl MockStateReader {
        /// Identity ramp + neutral color (50.0, 0.0), no fail, GDI engine.
        pub fn clean() -> Self {
            MockStateReader {
                lut: build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0),
                color: (50.0, 0.0),
                fail: false,
                nvcp_registry: None,
                engine: crate::color::GammaEngine::Gdi,
                is_intel: false,
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

        fn read_nvcp_registry(&self, _edid_id: &str) -> Option<(f64, f64, f64)> {
            if self.fail {
                None
            } else {
                self.nvcp_registry
            }
        }

        fn engine(&self, _edid_id: &str) -> crate::color::GammaEngine {
            self.engine
        }

        fn is_intel(&self, _edid_id: &str) -> bool {
            self.is_intel
        }
    }

    /// Build a minimal preset for tests.
fn p(id: &str, _edid: &str, brightness: f64, vibrance: f64, hue: f64) -> crate::store::Preset {
        crate::store::Preset {
            id: id.into(),
            name: "test".into(),
            icc_hash: String::new(),
            icc_filename: String::new(),
            brightness,
            contrast: 0.8,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 2.2,
            vibrance,
            saturation: 50.0,
            hue_deg: hue,
color_model: "nvcp-v1".into(),
            color_tag: None,
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
            nvcp_registry: None,
            engine: crate::color::GammaEngine::Gdi,
            is_intel: false,
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

    // ── Test 3: drifted LUT restores gamma AND neutral vibrance ──────

    #[test]
    fn drifted_lut_reapplies_gamma_and_neutral_vibrance() {
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

        assert_eq!(
            *nv.calls.lock().unwrap(),
            vec![("set".to_string(), 50.0, 0.0)],
            "neutral vibrance is still restored when supported"
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
            nvcp_registry: None,
            engine: crate::color::GammaEngine::Gdi,
            is_intel: false,
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
            nvcp_registry: None,
            engine: crate::color::GammaEngine::Gdi,
            is_intel: false,
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

    // ── Test 8: nvapi_registry_match_is_not_drift ─────────────────
    //
    // On an NVAPI display, drift is detected via registry compare AND GDI
    // LUT identity check. Registry matching + identity LUT = no drift.

    #[test]
    fn nvapi_registry_match_is_not_drift() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);

        // NVCP-scale neutral preset
        let mut preset = p("p1", "E", 50.0, 50.0, 0.0);
        preset.contrast = 50.0;
        preset.gamma = 1.0;

        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        let reader = MockStateReader {
            lut: build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0), // identity GDI LUT
            color: (50.0, 0.0), // matches neutral v/h
            fail: false,
            nvcp_registry: Some((100.0, 100.0, 100.0)), // matches ui_to_internal(50)
            engine: crate::color::GammaEngine::Nvapi,
            is_intel: false,
        };

        let events = check_once(&color, &nv, &reader, &pins, &[preset], false);
        assert_eq!(events.len(), 1);
        assert!(!events[0].applied, "registry match + identity LUT → no apply");
        assert!(events[0].error.is_none());
        assert!(
            color.calls.lock().unwrap().is_empty(),
            "no gamma calls when registry matches and LUT is identity"
        );
        assert!(
            nv.calls.lock().unwrap().is_empty(),
            "no nv calls when registry matches and LUT is identity"
        );
    }

    // ── Test 9: nvapi_registry_mismatch_reapplies ─────────────────
    //
    // On an NVAPI display, a registry mismatch must trigger reapply.

    #[test]
    fn nvapi_registry_mismatch_reapplies() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);

        let mut preset = p("p1", "E", 50.0, 50.0, 0.0);
        preset.contrast = 50.0;
        preset.gamma = 1.0;

        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        let reader = MockStateReader {
            lut: [0u16; 768],
            color: (50.0, 0.0),
            fail: false,
            nvcp_registry: Some((90.0, 100.0, 100.0)), // brightness 90 ≠ expected 100
            engine: crate::color::GammaEngine::Nvapi,
            is_intel: false,
        };

        let events = check_once(&color, &nv, &reader, &pins, &[preset], false);
        assert_eq!(events.len(), 1);
        assert!(events[0].applied, "registry mismatch → must reapply");
        assert!(events[0].error.is_none());

        // apply_color is called (with GDI fallback in test context)
        let calls = color.calls.lock().unwrap();
        assert!(!calls.is_empty(), "gamma should be called on registry mismatch");
    }

    // ── Test 10: gdi_branch_unchanged ─────────────────────────────
    //
    // GDI engine path uses the same lut_drifted + color_drifted logic
    // as before. Already green for the 7 existing tests; verify with a
    // named test that explicitly sets GDI engine.

    #[test]
    fn gdi_branch_unchanged() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = p("p1", "E", 1.0, 50.0, 0.0);
        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        // Matching LUT (same as preset's expected ramp) + GDI engine
        let reader = MockStateReader {
            lut: build_gamma_ramp(
                preset.brightness,
                preset.contrast,
                preset.rgb_gains,
                preset.gamma,
            ),
            color: (preset.vibrance, preset.hue_deg),
            fail: false,
            nvcp_registry: None,
            engine: crate::color::GammaEngine::Gdi,
            is_intel: false,
        };

        let events = check_once(&color, &nv, &reader, &pins, &[preset], false);
        assert_eq!(events.len(), 1);
        assert!(!events[0].applied, "in-sync GDI → no apply");
        assert!(events[0].error.is_none());
        assert!(color.calls.lock().unwrap().is_empty());
    }

    // ── Test 11: nvapi_dirty_lut_counts_as_drift ─────────────────
    //
    // Canonical evaporation case: registry matches but a foreign writer
    // (e.g. exclusive-fullscreen game) clobbered the GDI LUT. Must be
    // detected as drift even though the registry is pristine.

    #[test]
    fn nvapi_dirty_lut_counts_as_drift() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);

        let mut preset = p("p1", "E", 50.0, 50.0, 0.0);
        preset.contrast = 50.0;
        preset.gamma = 1.0;

        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        let reader = MockStateReader {
            lut: [0u16; 768], // dimmed — NOT identity
            color: (50.0, 0.0),
            fail: false,
            nvcp_registry: Some((100.0, 100.0, 100.0)), // pristine registry
            engine: crate::color::GammaEngine::Nvapi,
            is_intel: false,
        };

        let events = check_once(&color, &nv, &reader, &pins, &[preset], false);
        assert_eq!(events.len(), 1);
        assert!(events[0].applied, "dirty LUT → must reapply despite clean registry");
        assert!(events[0].error.is_none());

        // apply_color is called (GDI fallback in test context)
        let calls = color.calls.lock().unwrap();
        assert!(!calls.is_empty(), "gamma should be called on dirty LUT");
    }

    // ── Test 12: nvapi_registry_rounding_no_false_drift ─────────────
    //
    // Non-multiple-of-5 values (e.g. UI 37 → internal 94.8) round when
    // persisted to DWORD. The drift check must round both sides so that
    // a stored DWORD 95 matches expected 95 (not false-vs-94.8).

    #[test]
    fn nvapi_registry_rounding_no_false_drift() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);

        let mut preset = p("p1", "E", 37.0, 50.0, 0.0);
        preset.contrast = 50.0;
        preset.gamma = 1.0;

        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        let expected_b = crate::nvgamma::ui_to_internal(37.0); // 94.8
        let stored_b = expected_b.round() as u32; // 95

        // Registry holding 95 → no drift (matches rounded expected)
        let reader_match = MockStateReader {
            lut: crate::color::build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0),
            color: (50.0, 0.0),
            fail: false,
            nvcp_registry: Some((stored_b as f64, 100.0, 100.0)),
            engine: crate::color::GammaEngine::Nvapi,
            is_intel: false,
        };
        let events = check_once(&color, &nv, &reader_match, &pins, &[preset.clone()], false);
        assert_eq!(events.len(), 1);
        assert!(!events[0].applied, "95 matches rounded 94.8 → no drift");
        assert!(events[0].error.is_none());

        // Registry holding 94 → IS drift (94 != 95)
        let reader_drift = MockStateReader {
            lut: crate::color::build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0),
            color: (50.0, 0.0),
            fail: false,
            nvcp_registry: Some((94.0, 100.0, 100.0)),
            engine: crate::color::GammaEngine::Nvapi,
            is_intel: false,
        };
        let events2 = check_once(&color, &nv, &reader_drift, &pins, &[preset], false);
        assert_eq!(events2.len(), 1);
        assert!(events2[0].applied, "94 ≠ rounded 95 → drift detected");
        assert!(events2[0].error.is_none());
    }

    // ── Test 13: Intel display is NOT reapplied by the drift loop ─────
    //
    // Intel displays report the GDI engine (NVAPI is absent), but Intel apply
    // drives a pixtx LUT and purges the GDI ramp to identity. Without routing,
    // the GDI branch reads the identity ramp, sees it differ from a non-neutral
    // preset's expected ramp, and re-applies every 10s (H1). The `is_intel`
    // skip must prevent that re-apply.

    #[test]
    fn intel_display_is_not_reapplied_by_drift_loop() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = p("p1", "E", 0.5, 50.0, 0.0); // non-neutral brightness → GDI LUT would "drift"
        let mut pins = HashMap::new();
        pins.insert("E".to_string(), preset.id.clone());

        // Intel display: GDI engine with an identity ramp that differs from the
        // preset's expected ramp — the exact condition that used to re-apply.
        let reader = MockStateReader {
            lut: build_gamma_ramp(1.0, 1.0, [1.0, 1.0, 1.0], 1.0),
            color: (50.0, 0.0),
            fail: false,
            nvcp_registry: None,
            engine: crate::color::GammaEngine::Gdi,
            is_intel: true,
        };

        let events = check_once(&color, &nv, &reader, &pins, &[preset], false);
        assert_eq!(events.len(), 1);
        assert!(!events[0].applied, "Intel display must NOT be re-applied by the drift loop");
        assert!(events[0].error.is_none());
        assert!(
            color.calls.lock().unwrap().is_empty(),
            "no gamma/apply calls for an in-sync Intel display"
        );
        assert!(nv.calls.lock().unwrap().is_empty(), "no NV calls for Intel");
    }

    // ── Lifecycle: startup ──────────────────────────────────────────────

    fn connected_true(_: &str) -> bool {
        true
    }

    fn pin_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(edid, preset)| (edid.to_string(), preset.to_string()))
            .collect()
    }

    #[test]
    fn lifecycle_targets_prefers_pins_over_applied_and_sorts() {
        let pins = pin_map(&[("E2", "pin-preset")]);
        let applied = pin_map(&[("E1", "applied-preset"), ("E2", "stale-applied")]);
        let targets = lifecycle_targets(&pins, &applied);
        assert_eq!(
            targets,
            vec![
                ("E1".to_string(), "applied-preset".to_string()),
                ("E2".to_string(), "pin-preset".to_string()),
            ],
            "pins win; output is sorted by EDID"
        );
    }

    #[test]
    fn startup_plan_marks_missing_preset_for_restore() {
        let targets = vec![
            ("E1".to_string(), "exists".to_string()),
            ("E2".to_string(), "gone".to_string()),
        ];
        let presets = vec![p("exists", "E1", 1.0, 50.0, 0.0)];
        let plan = startup_plan(&targets, &presets, &connected_true);
        assert_eq!(plan[0].1, LifecycleAction::Apply("exists".into()));
        assert_eq!(plan[1].1, LifecycleAction::Restore("gone".into()));
    }

    #[test]
    fn startup_skips_offline_display_without_writes() {
        let color = TestRecorder::new(false);
        let nv = MockNvapi::new(false);
        let preset = p("p1", "E", 1.0, 50.0, 0.0);
        let targets = vec![("E".to_string(), preset.id.clone())];
        let events = run_startup(&color, &nv, &targets, &[preset], &|_| false);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "skip");
        assert!(!events[0].applied);
        assert!(events[0].error.is_none());
        assert!(color.calls.lock().unwrap().is_empty());
        assert!(nv.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn startup_applies_existing_preset_through_backend() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = p("p1", "E", 0.5, 50.0, 0.0);
        let targets = vec![("E".to_string(), preset.id.clone())];
        let events = run_startup(&color, &nv, &targets, &[preset], &|_| false);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "apply");
        assert!(events[0].applied);
        assert!(events[0].error.is_none());
        assert!(
            color.calls.lock().unwrap().contains(&"gamma".to_string()),
            "startup reapply must reach the backend"
        );
    }

    #[test]
    fn startup_skips_intel_reapply_to_preserve_restore_guarantee() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let preset = p("p1", "E", 0.5, 50.0, 0.0);
        let targets = vec![("E".to_string(), preset.id.clone())];
        // A display that routes to Intel must be skipped at startup (C1): a
        // startup apply would capture leftover pixtx state as "pre-apply" and
        // later Reset would falsely claim a restore to non-factory state.
        let events = run_startup(&color, &nv, &targets, &[preset], &|_| true);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "skip", "Intel must skip startup reapply");
        assert!(!events[0].applied);
        assert!(events[0].error.as_ref().unwrap().contains("Intel"), "got: {:?}", events[0].error);
        assert!(
            color.calls.lock().unwrap().is_empty(),
            "no driver write for Intel at startup"
        );
        assert!(nv.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn startup_restores_when_recorded_preset_is_gone() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let targets = vec![("E".to_string(), "deleted-preset".to_string())];
        let events = run_startup(&color, &nv, &targets, &[], &|_| false);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "restore");
        assert!(events[0].applied, "reset should report the identity ramp write");
        assert!(events[0].error.is_none());
        assert_eq!(
            *color.last_gamma_params.lock().unwrap(),
            Some((1.0, 1.0, [1.0, 1.0, 1.0], 1.0)),
            "stale record must use reset_monitor's identity-ramp parity path"
        );
    }

    #[test]
    fn startup_offline_stale_record_is_skipped_not_restored() {
        let color = TestRecorder::new(false);
        let nv = MockNvapi::new(false);
        let targets = vec![("E".to_string(), "deleted-preset".to_string())];
        let events = run_startup(&color, &nv, &targets, &[], &|_| false);
        assert_eq!(events[0].action, "skip");
        assert!(color.calls.lock().unwrap().is_empty());
    }

    // ── Lifecycle: exit ─────────────────────────────────────────────────

    struct RecordingBackend {
        id: &'static str,
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        restore_result: crate::color::ApplyResult,
    }

    impl RecordingBackend {
        fn new(id: &'static str, calls: &std::sync::Arc<std::sync::Mutex<Vec<String>>>) -> Self {
            RecordingBackend {
                id,
                calls: calls.clone(),
                restore_result: crate::color::ApplyResult {
                    icc_applied: false,
                    gamma_applied: true,
                    vibrance_applied: false,
                    error: None,
                },
            }
        }
    }

    impl crate::backend::ColorBackend for RecordingBackend {
        fn id(&self) -> &'static str {
            self.id
        }

        fn probe(&self, _edid_id: &str) -> Vec<crate::backend::AdjustmentCapability> {
            Vec::new()
        }

        fn apply(
            &self,
            _api: &dyn crate::color::ColorApi,
            _nv: &dyn crate::nvapi::NvColorApi,
            _preset: &crate::store::Preset,
            _edid_id: &str,
        ) -> crate::color::ApplyResult {
            crate::color::ApplyResult {
                icc_applied: false,
                gamma_applied: false,
                vibrance_applied: false,
                error: Some("apply not expected on the exit path".into()),
            }
        }

        fn restore(
            &self,
            _api: &dyn crate::color::ColorApi,
            _nv: &dyn crate::nvapi::NvColorApi,
            _edid_id: &str,
        ) -> crate::color::ApplyResult {
            self.calls.lock().unwrap().push(format!("restore:{}", self.id));
            self.restore_result.clone()
        }
    }

    struct MapSelector {
        backends: HashMap<String, RecordingBackend>,
    }

    impl BackendSelector for MapSelector {
        fn select_backend(&self, edid_id: &str) -> &dyn crate::backend::ColorBackend {
            &self.backends[edid_id]
        }

        fn backend_id_hint(&self, edid_id: &str) -> &'static str {
            self.backends[edid_id].id
        }
    }

    #[test]
    fn exit_keep_policy_never_writes() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut backends = HashMap::new();
        backends.insert("E1".to_string(), RecordingBackend::new("nvapi", &calls));
        let selector = MapSelector { backends };
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let targets = vec![("E1".to_string(), "p1".to_string())];

        let outcomes = run_exit(
            ExitPolicy::KeepApplied,
            &selector,
            &color,
            &nv,
            &targets,
            &connected_true,
        );
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].restored, "keep policy must not claim a restore");
        assert!(outcomes[0].detail.contains("kept"));
        assert!(calls.lock().unwrap().is_empty(), "no backend write on quit");
    }

    /// Selector whose `select_backend` panics: KeepApplied must classify from
    /// the cheap hint instead of loading a driver library.
    struct NoSelectSelector;

    impl BackendSelector for NoSelectSelector {
        fn select_backend(&self, _edid_id: &str) -> &dyn crate::backend::ColorBackend {
            panic!("select_backend must not be called on the KeepApplied path");
        }

        fn backend_id_hint(&self, _edid_id: &str) -> &'static str {
            "intel"
        }
    }

    #[test]
    fn exit_keep_policy_classifies_without_selecting_a_backend() {
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let targets = vec![("E1".to_string(), "p1".to_string())];

        let outcomes = run_exit(
            ExitPolicy::KeepApplied,
            &NoSelectSelector,
            &color,
            &nv,
            &targets,
            &connected_true,
        );
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].backend, "intel");
        assert!(!outcomes[0].restored);
        assert!(outcomes[0].detail.contains("best-effort"));
    }

    #[test]
    fn exit_restore_routes_each_display_to_its_backend() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut backends = HashMap::new();
        backends.insert("E2".to_string(), RecordingBackend::new("gdi", &calls));
        backends.insert("E1".to_string(), RecordingBackend::new("nvapi", &calls));
        let selector = MapSelector { backends };
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let targets = vec![
            ("E1".to_string(), "p1".to_string()),
            ("E2".to_string(), "p2".to_string()),
        ];

        let outcomes = run_exit(
            ExitPolicy::Restore,
            &selector,
            &color,
            &nv,
            &targets,
            &connected_true,
        );
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| o.restored));
        assert_eq!(outcomes[0].backend, "nvapi");
        assert_eq!(outcomes[1].backend, "gdi");
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["restore:nvapi".to_string(), "restore:gdi".to_string()]
        );
    }

    #[test]
    fn exit_disconnected_display_is_skipped_without_writes() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut backends = HashMap::new();
        backends.insert("E1".to_string(), RecordingBackend::new("nvapi", &calls));
        let selector = MapSelector { backends };
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let targets = vec![("E1".to_string(), "p1".to_string())];

        let outcomes = run_exit(
            ExitPolicy::Restore,
            &selector,
            &color,
            &nv,
            &targets,
            &|_| false,
        );
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].restored);
        assert!(outcomes[0].detail.contains("disconnected"));
        assert!(calls.lock().unwrap().is_empty(), "no write to a vanished display");
    }

    #[test]
    fn exit_restore_failure_is_reported_not_hidden() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut backend = RecordingBackend::new("intel", &calls);
        backend.restore_result = crate::color::ApplyResult {
            icc_applied: false,
            gamma_applied: false,
            vibrance_applied: false,
            error: Some("intel reset: driver refused".into()),
        };
        let mut backends = HashMap::new();
        backends.insert("E1".to_string(), backend);
        let selector = MapSelector { backends };
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let targets = vec![("E1".to_string(), "p1".to_string())];

        let outcomes = run_exit(
            ExitPolicy::Restore,
            &selector,
            &color,
            &nv,
            &targets,
            &connected_true,
        );
        assert!(!outcomes[0].restored);
        assert!(outcomes[0].detail.contains("driver refused"));
    }

    #[test]
    fn exit_keep_detail_for_intel_does_not_claim_restore() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut backends = HashMap::new();
        backends.insert("E1".to_string(), RecordingBackend::new("intel", &calls));
        let selector = MapSelector { backends };
        let color = TestRecorder::new(true);
        let nv = MockNvapi::new(true);
        let targets = vec![("E1".to_string(), "p1".to_string())];

        let outcomes = run_exit(
            ExitPolicy::KeepApplied,
            &selector,
            &color,
            &nv,
            &targets,
            &connected_true,
        );
        assert!(!outcomes[0].restored);
        assert!(outcomes[0].detail.contains("best-effort"));
        assert!(!outcomes[0].detail.contains("restored"));
    }
}
