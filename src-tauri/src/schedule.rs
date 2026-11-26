// ── Time-of-day per-monitor schedule engine & runtime ──────────────────────
// Pure window evaluation (supporting daytime and midnight wrap-around windows)
// plus a lightweight background loop enforcing precedence:
//   Pinned monitor > Active Foreground Auto-Apply > Active Schedule Rule > Manual Default.
//
// Only acts on schedule window transitions (entering a rule or leaving a rule)
// so manual overrides within an active window are respected until the next
// scheduled transition.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::auto_apply::revert_target;
use crate::monitor::Monitor;
use crate::store::{AppStore, Preset};

pub const SCHEDULE_EVENT: &str = "schedule://outcome";

/// One persisted time-of-day rule for a display.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduleRule {
    pub id: String,
    pub edid_id: String,
    pub preset_id: String,
    /// Local wall-clock start time in 24-hour `HH:MM` format (inclusive).
    pub start_time: String,
    /// Local wall-clock end time in 24-hour `HH:MM` format (exclusive).
    pub end_time: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// UI-facing notification payload when a schedule transition executes.
#[derive(Debug, Clone, Serialize)]
pub struct ScheduleOutcomeEvent {
    /// "applied" | "reverted" | "error"
    pub kind: String,
    pub edid_id: String,
    pub monitor_name: String,
    pub preset_id: String,
    pub preset_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restored_preset_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parse `"HH:MM"` (24h) into minutes since midnight (`0..1440`).
pub fn parse_hhmm(raw: &str) -> Result<u16, String> {
    let trimmed = raw.trim();
    let (hh, mm) = trimmed
        .split_once(':')
        .ok_or_else(|| format!("invalid time \"{trimmed}\" — expected HH:MM"))?;
    if hh.len() != 2 || mm.len() != 2 {
        return Err(format!("invalid time \"{trimmed}\" — expected HH:MM"));
    }
    let h: u16 = hh
        .parse()
        .map_err(|_| format!("invalid hour in \"{trimmed}\""))?;
    let m: u16 = mm
        .parse()
        .map_err(|_| format!("invalid minute in \"{trimmed}\""))?;
    if h >= 24 || m >= 60 {
        return Err(format!("time out of range \"{trimmed}\" (00:00–23:59)"));
    }
    Ok(h * 60 + m)
}

pub fn format_hhmm(minutes: u16) -> String {
    let m = minutes % 1440;
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Validate a slice of schedule rules against existing presets.
pub fn validate_rules(rules: &[ScheduleRule], presets: &[Preset]) -> Result<Vec<ScheduleRule>, String> {
    let mut normalized = Vec::with_capacity(rules.len());
    let mut seen_ids = HashSet::new();

    for rule in rules {
        let edid = rule.edid_id.trim();
        if edid.is_empty() {
            return Err("schedule rule must specify a monitor".into());
        }
        if !presets.iter().any(|p| p.id == rule.preset_id) {
            return Err(format!(
                "schedule references unknown preset {}",
                rule.preset_id
            ));
        }
        let start = parse_hhmm(&rule.start_time)?;
        let end = parse_hhmm(&rule.end_time)?;
        if start == end {
            return Err(format!(
                "schedule start and end time cannot be identical ({})",
                format_hhmm(start)
            ));
        }
        let id = if rule.id.trim().is_empty() || !seen_ids.insert(rule.id.trim().to_string()) {
            let fresh = uuid::Uuid::new_v4().to_string();
            seen_ids.insert(fresh.clone());
            fresh
        } else {
            rule.id.trim().to_string()
        };
        normalized.push(ScheduleRule {
            id,
            edid_id: edid.to_string(),
            preset_id: rule.preset_id.clone(),
            start_time: format_hhmm(start),
            end_time: format_hhmm(end),
            enabled: rule.enabled,
        });
    }

    Ok(normalized)
}

/// True when `now_min` falls in `[start_min, end_min)`.
/// Supports midnight wrap-around (e.g. `21:00` → `07:00`: `now >= 1260 || now < 420`).
pub fn is_active_at(start_min: u16, end_min: u16, now_min: u16) -> bool {
    if start_min == end_min {
        return false;
    }
    if start_min < end_min {
        now_min >= start_min && now_min < end_min
    } else {
        now_min >= start_min || now_min < end_min
    }
}

/// Decide the desired active rule for one monitor at `now_min`.
/// First matching enabled rule wins deterministically.
pub fn active_rule_for_monitor<'a>(
    rules: &'a [ScheduleRule],
    edid_id: &str,
    now_min: u16,
) -> Option<&'a ScheduleRule> {
    rules.iter().find(|rule| {
        if !rule.enabled || !crate::monitor::key_match(&rule.edid_id, edid_id) {
            return false;
        }
        match (parse_hhmm(&rule.start_time), parse_hhmm(&rule.end_time)) {
            (Ok(s), Ok(e)) => is_active_at(s, e, now_min),
            _ => false,
        }
    })
}

/// One transition emitted by `plan_schedule_transitions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleAction {
    Apply {
        edid_id: String,
        rule_id: String,
        preset_id: String,
    },
    Revert {
        edid_id: String,
        previous_preset_id: String,
    },
}

/// Tracked in-memory per-monitor schedule activation so we only act on window
/// boundaries (manual overrides within a window stay intact until transition).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSchedule {
    pub rule_id: String,
    pub preset_id: String,
}

/// Pure transition planner respecting:
/// **Pinned monitor > Active Foreground Auto-Apply > Active Schedule Rule > Manual Default**.
pub fn plan_schedule_transitions(
    rules: &[ScheduleRule],
    connected_edids: &[String],
    pins: &HashMap<String, String>,
    active_auto_edids: &HashSet<String>,
    now_min: u16,
    active_state: &mut HashMap<String, ActiveSchedule>,
) -> Vec<ScheduleAction> {
    let mut actions = Vec::new();
    let connected_set: HashSet<&str> = connected_edids.iter().map(String::as_str).collect();

    // Drop disconnected monitors from in-memory schedule tracking cleanly.
    active_state.retain(|edid, _| connected_set.contains(edid.as_str()));

    for edid in connected_edids {
        let is_pinned = pins
            .keys()
            .any(|pin_edid| crate::monitor::key_match(pin_edid, edid));
        let is_auto = active_auto_edids
            .iter()
            .any(|auto_edid| crate::monitor::key_match(auto_edid, edid));

        if is_pinned || is_auto {
            // Higher precedence owns this display right now; clear our active
            // marker so once pin/auto clears, an active window can apply cleanly.
            active_state.remove(edid);
            continue;
        }

        let desired = active_rule_for_monitor(rules, edid, now_min);
        let current = active_state.get(edid).cloned();

        match (current, desired) {
            (None, Some(rule)) => {
                active_state.insert(
                    edid.clone(),
                    ActiveSchedule {
                        rule_id: rule.id.clone(),
                        preset_id: rule.preset_id.clone(),
                    },
                );
                actions.push(ScheduleAction::Apply {
                    edid_id: edid.clone(),
                    rule_id: rule.id.clone(),
                    preset_id: rule.preset_id.clone(),
                });
            }
            (Some(prev), Some(rule))
                if prev.rule_id != rule.id || prev.preset_id != rule.preset_id =>
            {
                active_state.insert(
                    edid.clone(),
                    ActiveSchedule {
                        rule_id: rule.id.clone(),
                        preset_id: rule.preset_id.clone(),
                    },
                );
                actions.push(ScheduleAction::Apply {
                    edid_id: edid.clone(),
                    rule_id: rule.id.clone(),
                    preset_id: rule.preset_id.clone(),
                });
            }
            (Some(prev), None) => {
                active_state.remove(edid);
                actions.push(ScheduleAction::Revert {
                    edid_id: edid.clone(),
                    previous_preset_id: prev.preset_id,
                });
            }
            // Same rule still active, or still outside any window → no-op
            // (preserves manual overrides inside the window).
            _ => {}
        }
    }

    actions
}

/// Local wall-clock minutes since midnight (`0..1440`) using Win32 `GetLocalTime`.
pub fn current_local_minutes() -> u16 {
    #[cfg(windows)]
    {
        let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        (st.wHour % 24) * 60 + (st.wMinute % 60)
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        ((secs % 86_400) / 60) as u16
    }
}

// ── Tauri managed state & background loop ───────────────────────────────────

#[derive(Default)]
pub struct ScheduleState {
    pub active: Mutex<HashMap<String, ActiveSchedule>>,
}

/// Return all monitor EDIDs currently coloured by an active schedule window
/// (included in quit-restore sweep).
pub fn active_edids(app: &AppHandle) -> Vec<String> {
    let Some(state) = app.try_state::<ScheduleState>() else {
        return Vec::new();
    };
    let Ok(active) = state.active.lock() else {
        return Vec::new();
    };
    active.keys().cloned().collect()
}

/// Evaluate schedules immediately (called by the background loop and after
/// saving updated rules).
pub fn evaluate_now(app: &AppHandle) {
    let Ok(_operation) = crate::auto_apply_runtime::DISPLAY_OPERATION.lock() else {
        return;
    };
    let (rules, presets, pins, applied) = {
        let Some(store_state) = app.try_state::<AppStore>() else {
            return;
        };
        let Ok(store) = store_state.0.lock() else {
            return;
        };
        (
            store.list_schedules(),
            store.list_presets(),
            store.list_pins(),
            store.list_applied(),
        )
    };

    let Some(sched_state) = app.try_state::<ScheduleState>() else {
        return;
    };

    let auto_active = crate::auto_apply_runtime::active_edids(app);
    let monitors = crate::monitor::list_monitors();
    let connected_edids: Vec<String> = monitors
        .iter()
        .filter(|m| m.connected)
        .map(|m| m.edid_id.clone())
        .collect();
    let now_min = current_local_minutes();

    let actions = {
        let Ok(mut active) = sched_state.active.lock() else {
            return;
        };
        plan_schedule_transitions(
            &rules,
            &connected_edids,
            &pins,
            &auto_active,
            now_min,
            &mut active,
        )
    };

    if actions.is_empty() {
        return;
    }

    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let profiles_dir = crate::store::default_store_path().join("profiles");
    let profiles_dir_str = profiles_dir.to_string_lossy().to_string();

    for action in actions {
        match action {
            ScheduleAction::Apply {
                edid_id, preset_id, ..
            } => {
                let Some(preset) = presets.iter().find(|p| p.id == preset_id) else {
                    continue;
                };
                let outcome = crate::color::apply_preset_detailed(
                    &api,
                    &nv,
                    preset,
                    &edid_id,
                    &profiles_dir_str,
                );
                let kind = if outcome.result.error.is_some() {
                    "error"
                } else {
                    "applied"
                };
                let _ = app.emit(
                    SCHEDULE_EVENT,
                    ScheduleOutcomeEvent {
                        kind: kind.into(),
                        monitor_name: monitor_label(&monitors, &edid_id),
                        edid_id,
                        preset_id: preset.id.clone(),
                        preset_name: preset.name.clone(),
                        restored_preset_name: None,
                        error: outcome.result.error,
                    },
                );
            }
            ScheduleAction::Revert {
                edid_id,
                previous_preset_id,
            } => {
                let prev_name = presets
                    .iter()
                    .find(|p| p.id == previous_preset_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let (result, restored_name) = match revert_target(&edid_id, &applied, &presets) {
                    Some(prior) => (
                        crate::color::apply_color(&api, &nv, prior, &edid_id),
                        Some(prior.name.clone()),
                    ),
                    None => (crate::color::reset_monitor(&api, &nv, &edid_id), None),
                };
                let kind = if result.error.is_some() {
                    "error"
                } else {
                    "reverted"
                };
                let _ = app.emit(
                    SCHEDULE_EVENT,
                    ScheduleOutcomeEvent {
                        kind: kind.into(),
                        monitor_name: monitor_label(&monitors, &edid_id),
                        edid_id,
                        preset_id: previous_preset_id,
                        preset_name: prev_name,
                        restored_preset_name: restored_name,
                        error: result.error,
                    },
                );
            }
        }
    }
    crate::tray::refresh_tray(app);
}

fn monitor_label(monitors: &[Monitor], edid_id: &str) -> String {
    monitors
        .iter()
        .find(|m| crate::monitor::key_match(&m.edid_id, edid_id))
        .map(|m| {
            if !m.alias.is_empty() {
                m.alias.clone()
            } else if !m.model.is_empty() {
                m.model.clone()
            } else {
                m.edid_id.clone()
            }
        })
        .unwrap_or_else(|| edid_id.to_string())
}

/// Background schedule watcher: checks once on startup (after a short settle)
/// and every 30 seconds so minute boundaries and wake-from-sleep trigger cleanly.
pub async fn schedule_loop(app: AppHandle) {
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    loop {
        let handle = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || evaluate_now(&handle)).await;
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_schedules_cmd(state: tauri::State<'_, AppStore>) -> Result<Vec<ScheduleRule>, String> {
    let store = state.0.lock().map_err(|e| e.to_string())?;
    Ok(store.list_schedules())
}

#[tauri::command]
pub fn save_schedules_cmd(
    app: AppHandle,
    state: tauri::State<'_, AppStore>,
    schedules: Vec<ScheduleRule>,
) -> Result<Vec<ScheduleRule>, String> {
    let saved = {
        let mut store = state.0.lock().map_err(|e| e.to_string())?;
        store.set_schedules(schedules).map_err(|e| e.to_string())?
    };
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || evaluate_now(&handle));
    Ok(saved)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, edid: &str, preset: &str, start: &str, end: &str, enabled: bool) -> ScheduleRule {
        ScheduleRule {
            id: id.into(),
            edid_id: edid.into(),
            preset_id: preset.into(),
            start_time: start.into(),
            end_time: end.into(),
            enabled,
        }
    }

    #[test]
    fn parse_hhmm_accepts_valid_and_rejects_bad() {
        assert_eq!(parse_hhmm("00:00").unwrap(), 0);
        assert_eq!(parse_hhmm("07:30").unwrap(), 450);
        assert_eq!(parse_hhmm("23:59").unwrap(), 1439);
        assert!(parse_hhmm("24:00").is_err());
        assert!(parse_hhmm("12:60").is_err());
        assert!(parse_hhmm("7:00").is_err());
        assert!(parse_hhmm("noon").is_err());
    }

    #[test]
    fn is_active_at_handles_daytime_and_midnight_wrap_around() {
        // Daytime: 09:00 -> 17:00
        let s = parse_hhmm("09:00").unwrap();
        let e = parse_hhmm("17:00").unwrap();
        assert!(!is_active_at(s, e, parse_hhmm("08:59").unwrap()));
        assert!(is_active_at(s, e, parse_hhmm("09:00").unwrap()));
        assert!(is_active_at(s, e, parse_hhmm("16:59").unwrap()));
        assert!(!is_active_at(s, e, parse_hhmm("17:00").unwrap()));

        // Overnight wrap: 21:00 -> 07:00
        let ns = parse_hhmm("21:00").unwrap();
        let ne = parse_hhmm("07:00").unwrap();
        assert!(!is_active_at(ns, ne, parse_hhmm("20:59").unwrap()));
        assert!(is_active_at(ns, ne, parse_hhmm("21:00").unwrap()));
        assert!(is_active_at(ns, ne, parse_hhmm("23:59").unwrap()));
        assert!(is_active_at(ns, ne, parse_hhmm("00:00").unwrap()));
        assert!(is_active_at(ns, ne, parse_hhmm("06:59").unwrap()));
        assert!(!is_active_at(ns, ne, parse_hhmm("07:00").unwrap()));
        assert!(!is_active_at(ns, ne, parse_hhmm("12:00").unwrap()));
    }

    #[test]
    fn precedence_pinned_and_auto_apply_beat_schedule() {
        let rules = vec![rule("r1", "M1", "night", "21:00", "07:00", true)];
        let connected = vec!["M1".to_string()];
        let mut state = HashMap::new();

        // Pinned monitor skips schedule.
        let mut pins = HashMap::new();
        pins.insert("M1".into(), "pinned-preset".into());
        let actions = plan_schedule_transitions(
            &rules,
            &connected,
            &pins,
            &HashSet::new(),
            parse_hhmm("22:00").unwrap(),
            &mut state,
        );
        assert!(actions.is_empty());

        // Unpinned, but active auto-apply skips schedule.
        let mut auto = HashSet::new();
        auto.insert("M1".to_string());
        let actions = plan_schedule_transitions(
            &rules,
            &connected,
            &HashMap::new(),
            &auto,
            parse_hhmm("22:00").unwrap(),
            &mut state,
        );
        assert!(actions.is_empty());

        // Once both pin and auto-apply are clear, entering window applies once.
        let actions = plan_schedule_transitions(
            &rules,
            &connected,
            &HashMap::new(),
            &HashSet::new(),
            parse_hhmm("22:00").unwrap(),
            &mut state,
        );
        assert_eq!(
            actions,
            vec![ScheduleAction::Apply {
                edid_id: "M1".into(),
                rule_id: "r1".into(),
                preset_id: "night".into(),
            }]
        );

        // Next minute inside same window is a no-op (respects manual overrides).
        let actions = plan_schedule_transitions(
            &rules,
            &connected,
            &HashMap::new(),
            &HashSet::new(),
            parse_hhmm("22:01").unwrap(),
            &mut state,
        );
        assert!(actions.is_empty());

        // Exiting window at 07:00 triggers Revert.
        let actions = plan_schedule_transitions(
            &rules,
            &connected,
            &HashMap::new(),
            &HashSet::new(),
            parse_hhmm("07:00").unwrap(),
            &mut state,
        );
        assert_eq!(
            actions,
            vec![ScheduleAction::Revert {
                edid_id: "M1".into(),
                previous_preset_id: "night".into(),
            }]
        );
    }

    #[test]
    fn disabled_rule_is_ignored_and_reverts_active_rule() {
        let connected = vec!["M1".to_string()];
        let mut state = HashMap::new();
        let active_rule = vec![rule("r1", "M1", "night", "21:00", "07:00", true)];
        let _ = plan_schedule_transitions(
            &active_rule,
            &connected,
            &HashMap::new(),
            &HashSet::new(),
            parse_hhmm("23:00").unwrap(),
            &mut state,
        );
        assert!(state.contains_key("M1"));

        let disabled_rule = vec![rule("r1", "M1", "night", "21:00", "07:00", false)];
        let actions = plan_schedule_transitions(
            &disabled_rule,
            &connected,
            &HashMap::new(),
            &HashSet::new(),
            parse_hhmm("23:01").unwrap(),
            &mut state,
        );
        assert_eq!(
            actions,
            vec![ScheduleAction::Revert {
                edid_id: "M1".into(),
                previous_preset_id: "night".into(),
            }]
        );
    }
}
