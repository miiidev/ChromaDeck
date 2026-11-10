// ── Auto-apply runtime ────────────────────────────────────────────────────
//
// The impure half of auto-apply: a 1 s foreground poll, the hardware calls it
// drives, the in-memory active record, and the UI events. All decisions live
// in `auto_apply::transition`; this module only executes them.
//
// State is in-memory by design. Nothing here writes `applied.json`, so an
// auto-applied look never survives a restart or masquerades as a manual apply.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::auto_apply::{transition, ActiveAuto, AutoAction, ObservedForeground};
use crate::foreground::ObservedWindow;
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
    /// Last observation written to the diagnostics log, so the 1 s poll only
    /// logs real foreground / monitor changes instead of one line per second.
    last_logged: String,
}

impl AutoRuntime {
    /// Reset bookkeeping for a master-switch change.
    ///
    /// Enabling marks the runtime seeded and forgets the last key, so the very
    /// next tick evaluates whatever is in front immediately. Without this the
    /// first tick only seeded itself and the user had to alt-tab away and back
    /// before anything applied — the "auto-apply does nothing" report.
    ///
    /// Disabling keeps `active` so the next tick can revert the colour that
    /// auto-apply applied; `seeded` is cleared so re-enabling starts fresh.
    fn apply_switch(&mut self, enabled: bool) {
        self.last_key = None;
        self.last_logged.clear();
        if enabled {
            self.active = None;
            self.seeded = true;
        } else {
            self.seeded = false;
        }
    }
}

pub struct AutoState {
    pub runtime: Mutex<AutoRuntime>,
    pub check_item: Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,
    /// Store directory, used only for the append-only diagnostics log.
    data_dir: PathBuf,
}

impl AutoState {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            runtime: Mutex::new(AutoRuntime::default()),
            check_item: Mutex::new(None),
            data_dir,
        }
    }
}

// ── Diagnostics ────────────────────────────────────────────────────────────

const LOG_FILE: &str = "auto-apply.log";
/// Truncate the log once it reaches this size; auto-apply is a background
/// feature and its diagnostics must never grow without bound.
const LOG_MAX_BYTES: u64 = 256 * 1024;

/// Append one diagnostic line to `<data>/auto-apply.log`. Best effort: a
/// failed write must never disturb the watcher. Timestamps are Unix epoch
/// milliseconds so the file sorts and correlates with system logs.
fn log(app: &AppHandle, message: &str) {
    let Some(state) = app.try_state::<AutoState>() else {
        return;
    };
    let path = state.data_dir.join(LOG_FILE);
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() >= LOG_MAX_BYTES {
            let _ = std::fs::remove_file(&path);
        }
    }
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "{ms} {message}");
    }
}

/// Log an observation only when it differs from the previous tick's.
fn log_observed(app: &AppHandle, rt: &mut AutoRuntime, line: String) {
    if rt.last_logged == line {
        return;
    }
    rt.last_logged = line.clone();
    log(app, &line);
}

/// Fast foreground watcher (500 ms poll). Spawned once from setup alongside
/// the enforcer. Half a second is imperceptible enough that apply / un-apply
/// feels instant when switching between mapped programs, while staying gentle
/// on CPU. Foreground changes are detected within one poll cycle so both
/// apply and un-apply (including minimize) happen without perceptible delay.
pub async fn auto_apply_loop(app: AppHandle) {
    loop {
        tick(&app);
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
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
        // Turning the switch off must undo the colour auto-apply wrote, not
        // just forget the record: the preset could otherwise stay on the
        // panel after the user believed auto-apply was off.
        if let Some(active) = rt.active.take() {
            let preset_name = presets
                .iter()
                .find(|p| p.id == active.preset_id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let result = crate::color::reset_monitor(
                &crate::color::RealColorApi,
                &crate::nvapi::RealNvapi,
                &active.edid_id,
            );
            let monitors = crate::monitor::list_monitors();
            log(
                app,
                &format!(
                    "switch off: reverted edid={} preset={:?} error={:?}",
                    active.edid_id, preset_name, result.error
                ),
            );
            emit(
                app,
                AutoApplyEvent {
                    kind: "reverted".into(),
                    monitor_name: monitor_name(&monitors, &active.edid_id),
                    edid_id: active.edid_id,
                    preset_id: active.preset_id,
                    preset_name,
                    program: String::new(),
                    error: result.error,
                },
            );
        }
        rt.last_key = None;
        rt.seeded = false;
        rt.last_logged.clear();
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
            log(app, &format!("active display gone: cleared edid={edid}"));
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
            // No foreground window (lock screen, secure desktop, or the
            // foreground process could not be queried — e.g. after
            // minimizing a mapped program). Treat as unmapped so any
            // active auto-apply is reverted rather than left stuck.
            // An empty program / None device flows through the normal path
            // so `rt.last_key` is reset and `transition(None)` reverts.
            log_observed(app, &mut rt, "fg=none".into());
            ObservedWindow {
                program: String::new(),
                device_name: None,
            }
        }
    };
    if self_program.as_deref() == Some(observed.program.as_str()) {
        log_observed(
            app,
            &mut rt,
            format!("fg={} self window: skipped", observed.program),
        );
        return;
    }

    // Compare device names case-insensitively: Windows is not guaranteed to
    // preserve the casing returned by EnumDisplayDevices.
    let matched = observed.device_name.as_ref().and_then(|device| {
        monitors
            .iter()
            .find(|m| m.device_name.eq_ignore_ascii_case(device))
    });
    let foreground = matched.map(|m| ObservedForeground {
        program: observed.program.clone(),
        edid_id: m.edid_id.clone(),
    });

    log_observed(
        app,
        &mut rt,
        format!(
            "fg={} device={} edid={}",
            observed.program,
            observed.device_name.as_deref().unwrap_or("none"),
            matched.map(|m| m.edid_id.as_str()).unwrap_or("unmatched"),
        ),
    );

    let key = foreground
        .as_ref()
        .map(|f| (f.program.clone(), f.edid_id.clone()));

    if !rt.seeded {
        rt.seeded = true;
        rt.last_key = key;
        log(app, "cold start: seeded foreground without acting");
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
                    log(
                        app,
                        &format!(
                            "apply failed preset={:?} edid={} program={} error={error}",
                            preset.name, edid_id, program
                        ),
                    );
                    emit(
                        app,
                        event_for("error", &edid_id, preset, &program, &monitors, Some(error)),
                    );
                    rt.active = None;
                } else {
                    log(
                        app,
                        &format!(
                            "apply ok preset={:?} edid={} program={}",
                            preset.name, edid_id, program
                        ),
                    );
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
                log(
                    app,
                    &format!(
                        "revert edid={} preset={:?} error={:?}",
                        edid_id, preset_name, result.error
                    ),
                );
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
                log(
                    app,
                    &format!(
                        "skip preset={:?} edid={} program={} reason={}",
                        preset_name, edid_id, program, reason
                    ),
                );
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
/// Persists the value, syncs the tray checkbox, resets the watcher's
/// bookkeeping, and notifies the frontend.
///
/// Enabling does not apply immediately (the watcher owns hardware calls);
/// it marks the runtime seeded so the *next* tick acts on the current
/// foreground. Disabling reverts the active auto application synchronously,
/// before the event is emitted: the panel must not keep an auto preset after
/// the user turned the feature off.
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

    // The tick holds the runtime lock for the whole hardware write, so taking
    // it here cannot interleave with an in-flight apply.
    let mut reverted: Option<ActiveAuto> = None;
    if let Some(state) = app.try_state::<AutoState>() {
        if let Ok(mut rt) = state.runtime.lock() {
            rt.apply_switch(enabled);
            if !enabled {
                reverted = rt.active.take();
            }
        }
        if let Ok(item) = state.check_item.lock() {
            if let Some(item) = item.as_ref() {
                let _ = item.set_checked(enabled);
            }
        }
    }

    if let Some(active) = reverted {
        let preset_name = app
            .try_state::<AppStore>()
            .and_then(|s| s.0.lock().ok().map(|s| s.list_presets()))
            .and_then(|presets| {
                presets
                    .iter()
                    .find(|p| p.id == active.preset_id)
                    .map(|p| p.name.clone())
            })
            .unwrap_or_default();
        let result = crate::color::reset_monitor(
            &crate::color::RealColorApi,
            &crate::nvapi::RealNvapi,
            &active.edid_id,
        );
        let monitors = crate::monitor::list_monitors();
        log(
            app,
            &format!(
                "switch off: reverted edid={} preset={:?} error={:?}",
                active.edid_id, preset_name, result.error
            ),
        );
        emit(
            app,
            AutoApplyEvent {
                kind: "reverted".into(),
                monitor_name: monitor_name(&monitors, &active.edid_id),
                edid_id: active.edid_id,
                preset_id: active.preset_id,
                preset_name,
                program: String::new(),
                error: result.error,
            },
        );
    }

    log(
        app,
        if enabled {
            "switch on: next tick evaluates the current foreground"
        } else {
            "switch off: revert complete"
        },
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn active() -> ActiveAuto {
        ActiveAuto {
            edid_id: "D1".into(),
            preset_id: "p1".into(),
            program: "vlc.exe".into(),
        }
    }

    #[test]
    fn enabling_seeds_and_forgets_the_last_key() {
        // Regression: enabling used to leave `seeded = false`, so the first
        // tick after the switch only seeded and silently skipped the apply
        // until the user alt-tabbed away and back.
        let mut rt = AutoRuntime {
            active: Some(active()),
            last_key: Some(("old.exe".into(), "D1".into())),
            seeded: false,
            last_logged: String::new(),
        };
        rt.apply_switch(true);
        assert!(rt.seeded, "next tick must evaluate the foreground");
        assert_eq!(rt.last_key, None, "the stale key must not suppress the apply");
        assert!(rt.active.is_none(), "a fresh enable starts with no active record");
    }

    #[test]
    fn disabling_keeps_the_active_record_for_the_revert_tick() {
        let mut rt = AutoRuntime {
            active: Some(active()),
            last_key: Some(("vlc.exe".into(), "D1".into())),
            seeded: true,
            last_logged: String::new(),
        };
        rt.apply_switch(false);
        assert!(
            rt.active.is_some(),
            "the watcher still needs the record to reset the monitor"
        );
        assert!(!rt.seeded);
        assert_eq!(rt.last_key, None);
    }

    #[test]
    fn re_enabling_after_a_disable_evaluates_the_foreground_again() {
        let mut rt = AutoRuntime::default();
        rt.apply_switch(true);
        rt.seeded = false; // simulate the disabled branch resetting the flag
        rt.apply_switch(true);
        assert!(rt.seeded);
        assert_eq!(rt.last_key, None);
    }
}
