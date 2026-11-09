// ── Auto-apply runtime ────────────────────────────────────────────────────
//
// The impure half of auto-apply: a 1 s foreground poll, the hardware calls it
// drives, the in-memory active record, and the UI events. All decisions live
// in `auto_apply::transition`; this module only executes them.
//
// State is in-memory by design. Nothing here writes `applied.json`, so an
// auto-applied look never survives a restart or masquerades as a manual apply.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::auto_apply::{transition, ActiveAuto, AutoAction, ObservedForeground};
use crate::monitor::Monitor;
use crate::store::{AppStore, Preset};

/// Emitted for every applied / reverted / skipped / errored / cleared change.
pub const OUTCOME_EVENT: &str = "auto-apply://outcome";
/// Emitted when the master switch changes (footer toggle ↔ tray checkbox).
pub const ENABLED_EVENT: &str = "auto-apply://enabled";
/// Tray checkbox id.
pub const TRAY_CHECK_ID: &str = "tray-auto-apply";

#[derive(Debug, Clone, Serialize)]
pub struct AutoApplyEvent {
    pub kind: String, // applied | reverted | skipped | error | cleared
    pub edid_id: String,
    pub preset_id: String,
    pub preset_name: String,
    pub monitor_name: String,
    pub program: String,
    pub error: Option<String>,
}

/// What the frontend needs to restore its marker after remounting.
#[derive(Debug, Clone, Serialize)]
pub struct AutoAppliedInfo {
    pub edid_id: String,
    pub preset_id: String,
}

/// In-memory bookkeeping for the watcher thread.
#[derive(Default)]
pub struct AutoRuntime {
    active: Option<ActiveAuto>,
    /// (program, edid) from the previous tick, used to skip unchanged state.
    last_key: Option<(String, String)>,
    /// Cold start: the first tick only records the key, never acts, so a
    /// program already in front at launch is not stomped on.
    seeded: bool,
}

pub struct AutoState {
    pub runtime: Mutex<AutoRuntime>,
    pub check_item: Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,
}

impl Default for AutoState {
    fn default() -> Self {
        Self {
            runtime: Mutex::new(AutoRuntime::default()),
            check_item: Mutex::new(None),
        }
    }
}

/// 1 s foreground watcher. Spawned once from setup alongside the enforcer.
pub async fn auto_apply_loop(app: AppHandle) {
    loop {
        tick(&app);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

fn tick(app: &AppHandle) {
    let (enabled, presets, pins) = {
        let store_state = match app.try_state::<AppStore>() {
            Some(s) => s,
            None => return,
        };
        let Ok(store) = store_state.0.lock() else {
            return;
        };
        (
            store.auto_apply_enabled(),
            store.list_presets(),
            store.list_pins(),
        )
    };

    let Some(state) = app.try_state::<AutoState>() else {
        return;
    };
    let Ok(mut rt) = state.runtime.lock() else {
        return;
    };

    if !enabled {
        if rt.active.take().is_some() {
            emit(
                app,
                AutoApplyEvent {
                    kind: "cleared".into(),
                    edid_id: String::new(),
                    preset_id: String::new(),
                    preset_name: String::new(),
                    monitor_name: String::new(),
                    program: String::new(),
                    error: None,
                },
            );
        }
        rt.last_key = None;
        rt.seeded = false;
        return;
    }

    let monitors = crate::monitor::list_monitors();

    // Active monitor unplugged: drop the record, leave colour alone (the
    // display is gone; a reconnect re-evaluates from scratch).
    if let Some(active) = &rt.active {
        if !monitors
            .iter()
            .any(|m| m.edid_id == active.edid_id && m.connected)
        {
            let edid = active.edid_id.clone();
            rt.active = None;
            emit_cleared(app, &edid);
            return;
        }
    }

    // ChromaDeck's own window must never trigger a revert: skip the whole
    // tick so the previous state stands while the user is in this app.
    let self_program = crate::foreground::self_program();
    let observed = match crate::foreground::foreground_window() {
        Some(w) => w,
        None => {
            // No foreground window (lock screen, secure desktop): do nothing.
            return;
        }
    };
    if self_program.as_deref() == Some(observed.program.as_str()) {
        return;
    }

    let foreground = observed.device_name.as_ref().and_then(|device| {
        monitors
            .iter()
            .find(|m| m.device_name == *device)
            .map(|m| ObservedForeground {
                program: observed.program.clone(),
                edid_id: m.edid_id.clone(),
            })
    });

    let key = foreground
        .as_ref()
        .map(|f| (f.program.clone(), f.edid_id.clone()));

    if !rt.seeded {
        rt.seeded = true;
        rt.last_key = key;
        return;
    }
    if rt.last_key == key {
        return;
    }
    rt.last_key = key.clone();

    let actions = transition(rt.active.as_ref(), &presets, &pins, foreground.as_ref());
    for action in actions {
        match action {
            AutoAction::Apply {
                edid_id,
                preset_id,
                program,
            } => {
                let Some(preset) = presets.iter().find(|p| p.id == preset_id) else {
                    continue;
                };
                let result = crate::color::apply_color(
                    &crate::color::RealColorApi,
                    &crate::nvapi::RealNvapi,
                    preset,
                    &edid_id,
                );
                if let Some(error) = result.error {
                    emit(
                        app,
                        event_for("error", &edid_id, preset, &program, &monitors, Some(error)),
                    );
                    rt.active = None;
                } else {
                    rt.active = Some(ActiveAuto {
                        edid_id: edid_id.clone(),
                        preset_id: preset_id.clone(),
                        program: program.clone(),
                    });
                    emit(
                        app,
                        event_for("applied", &edid_id, preset, &program, &monitors, None),
                    );
                }
            }
            AutoAction::Revert {
                edid_id,
                preset_id,
            } => {
                let result = crate::color::reset_monitor(
                    &crate::color::RealColorApi,
                    &crate::nvapi::RealNvapi,
                    &edid_id,
                );
                rt.active = None;
                let preset_name = presets
                    .iter()
                    .find(|p| p.id == preset_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                emit(
                    app,
                    AutoApplyEvent {
                        kind: "reverted".into(),
                        monitor_name: monitor_name(&monitors, &edid_id),
                        edid_id,
                        preset_id,
                        preset_name,
                        program: String::new(),
                        error: result.error,
                    },
                );
            }
            AutoAction::Skip {
                edid_id,
                preset_id,
                program,
                reason,
            } => {
                let preset_name = presets
                    .iter()
                    .find(|p| p.id == preset_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                emit(
                    app,
                    AutoApplyEvent {
                        kind: "skipped".into(),
                        monitor_name: monitor_name(&monitors, &edid_id),
                        edid_id,
                        preset_id,
                        preset_name,
                        program,
                        error: Some(reason),
                    },
                );
            }
        }
    }
}

fn event_for(
    kind: &str,
    edid_id: &str,
    preset: &Preset,
    program: &str,
    monitors: &[Monitor],
    error: Option<String>,
) -> AutoApplyEvent {
    AutoApplyEvent {
        kind: kind.into(),
        edid_id: edid_id.into(),
        preset_id: preset.id.clone(),
        preset_name: preset.name.clone(),
        monitor_name: monitor_name(monitors, edid_id),
        program: program.into(),
        error,
    }
}

fn emit_cleared(app: &AppHandle, edid_id: &str) {
    emit(
        app,
        AutoApplyEvent {
            kind: "cleared".into(),
            edid_id: edid_id.into(),
            preset_id: String::new(),
            preset_name: String::new(),
            monitor_name: String::new(),
            program: String::new(),
            error: None,
        },
    );
}

fn emit(app: &AppHandle, event: AutoApplyEvent) {
    let _ = app.emit(OUTCOME_EVENT, event);
}

fn monitor_name(monitors: &[Monitor], edid_id: &str) -> String {
    monitors
        .iter()
        .find(|m| m.edid_id == edid_id)
        .map(|m| {
            if m.alias.is_empty() {
                m.model.clone()
            } else {
                m.alias.clone()
            }
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "monitor".into())
}

/// Turn the master switch on/off from either surface (footer or tray).
/// Persists the value, syncs the tray checkbox, drops the auto record, and
/// notifies the frontend.
pub fn set_auto_apply(app: &AppHandle, enabled: bool) -> Result<(), String> {
    {
        let store_state = app
            .try_state::<AppStore>()
            .ok_or_else(|| "store unavailable".to_string())?;
        let mut store = store_state.0.lock().map_err(|e| e.to_string())?;
        store
            .set_auto_apply_enabled(enabled)
            .map_err(|e| e.to_string())?;
    }

    if let Some(state) = app.try_state::<AutoState>() {
        if let Ok(mut rt) = state.runtime.lock() {
            rt.active = None;
            rt.last_key = None;
            rt.seeded = false;
        }
        if let Ok(item) = state.check_item.lock() {
            if let Some(item) = item.as_ref() {
                let _ = item.set_checked(enabled);
            }
        }
    }

    if !enabled {
        emit_cleared(app, "");
    }
    let _ = app.emit(ENABLED_EVENT, enabled);
    Ok(())
}

#[tauri::command]
pub fn list_auto_applied_cmd(state: tauri::State<'_, AutoState>) -> Option<AutoAppliedInfo> {
    let rt = state.runtime.lock().ok()?;
    rt.active.as_ref().map(|a| AutoAppliedInfo {
        edid_id: a.edid_id.clone(),
        preset_id: a.preset_id.clone(),
    })
}
