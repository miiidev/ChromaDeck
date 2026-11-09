// ── Auto-apply: program → preset matching and the pure state machine ───────
//
// This module owns the decisions. It deliberately contains no Win32 calls and
// no hardware writes so every rule — programme identity, pin precedence,
// revert-on-close, monitor moves — is unit-testable. The runtime that polls
// the foreground window and executes the actions lives in `auto_apply_runtime`
// (added in a later task).

use std::collections::HashMap;

use crate::store::Preset;

/// The window currently in front of the user, resolved to a mapped preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedForeground {
    pub program: String,
    pub edid_id: String,
}

/// The one monitor currently coloured by auto-apply. Auto state is in-memory
/// only; it is never written to `applied.json` and never survives a restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAuto {
    pub edid_id: String,
    pub preset_id: String,
    pub program: String,
}

/// One side effect the runtime must perform, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoAction {
    Apply {
        edid_id: String,
        preset_id: String,
        program: String,
    },
    Revert {
        edid_id: String,
        preset_id: String,
    },
    Skip {
        edid_id: String,
        preset_id: String,
        program: String,
        reason: String,
    },
}

/// Reduce an executable path or typed name to the stored base-name identity:
/// trim, take the segment after the last path separator, lowercase. Rejects
/// empty names and names containing whitespace (a path with spaces still
/// normalises to a clean base name because the directory part is dropped).
pub fn normalize_program_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let base = trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(trimmed)
        .trim();
    if base.is_empty() || base.chars().any(char::is_whitespace) {
        return None;
    }
    Some(base.to_lowercase())
}

/// The preset that owns `program`, if any. Stored programs are already
/// normalised; normalise the live process name again so casing never matters.
pub fn resolve_mapping<'a>(presets: &'a [Preset], program: &str) -> Option<&'a Preset> {
    let program = normalize_program_name(program)?;
    presets
        .iter()
        .find(|preset| preset.attached_programs.iter().any(|m| m == &program))
}

fn push_revert(actions: &mut Vec<AutoAction>, active: Option<&ActiveAuto>) {
    if let Some(active) = active {
        actions.push(AutoAction::Revert {
            edid_id: active.edid_id.clone(),
            preset_id: active.preset_id.clone(),
        });
    }
}

/// Decide what to do for the current foreground window.
///
/// Rules, in order:
/// 1. No foreground window, or an unmapped programme: revert the active auto
///    application (its programme stopped being foreground).
/// 2. The mapped preset targets a pinned monitor: revert the previous auto
///    application, then skip — pin intent beats automation.
/// 3. The same preset is already active on the same monitor: no action, even
///    when a second programme mapped to that preset came forward.
/// 4. Otherwise revert any previous auto application, then apply.
pub fn transition(
    active: Option<&ActiveAuto>,
    presets: &[Preset],
    pins: &HashMap<String, String>,
    foreground: Option<&ObservedForeground>,
) -> Vec<AutoAction> {
    let mut actions = Vec::new();

    let Some(foreground) = foreground else {
        push_revert(&mut actions, active);
        return actions;
    };

    let Some(preset) = resolve_mapping(presets, &foreground.program) else {
        push_revert(&mut actions, active);
        return actions;
    };

    let program = normalize_program_name(&foreground.program)
        .unwrap_or_else(|| foreground.program.clone());

    if pins.contains_key(&foreground.edid_id) {
        push_revert(&mut actions, active);
        actions.push(AutoAction::Skip {
            edid_id: foreground.edid_id.clone(),
            preset_id: preset.id.clone(),
            program,
            reason: "monitor is pinned".into(),
        });
        return actions;
    }

    let already_active = active
        .map(|a| a.edid_id == foreground.edid_id && a.preset_id == preset.id)
        .unwrap_or(false);
    if already_active {
        return actions;
    }

    push_revert(&mut actions, active);
    actions.push(AutoAction::Apply {
        edid_id: foreground.edid_id.clone(),
        preset_id: preset.id.clone(),
        program,
    });
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(id: &str, name: &str, programs: &[&str]) -> Preset {
        Preset {
            id: id.into(),
            name: name.into(),
            icc_hash: String::new(),
            icc_filename: String::new(),
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
            attached_programs: programs.iter().map(|p| p.to_string()).collect(),
        }
    }

    fn fg(program: &str, edid: &str) -> ObservedForeground {
        ObservedForeground {
            program: program.into(),
            edid_id: edid.into(),
        }
    }

    fn active(edid: &str, preset_id: &str, program: &str) -> ActiveAuto {
        ActiveAuto {
            edid_id: edid.into(),
            preset_id: preset_id.into(),
            program: program.into(),
        }
    }

    #[test]
    fn normalizes_full_paths_and_case() {
        assert_eq!(
            normalize_program_name(r"C:\Program Files\Google\Chrome\chrome.EXE"),
            Some("chrome.exe".into())
        );
        assert_eq!(normalize_program_name("  chrome.exe "), Some("chrome.exe".into()));
        assert_eq!(normalize_program_name(""), None);
        assert_eq!(normalize_program_name("   "), None);
        assert_eq!(normalize_program_name("chrome setup.exe"), None);
    }

    #[test]
    fn unmapped_foreground_reverts_active_once() {
        let presets = [preset("p1", "Cinema", &["vlc.exe"])];
        let actions = transition(
            Some(&active("D1", "p1", "vlc.exe")),
            &presets,
            &HashMap::new(),
            Some(&fg("explorer.exe", "D1")),
        );
        assert_eq!(
            actions,
            vec![AutoAction::Revert {
                edid_id: "D1".into(),
                preset_id: "p1".into()
            }]
        );
    }

    #[test]
    fn no_foreground_reverts_active_once() {
        let presets = [preset("p1", "Cinema", &["vlc.exe"])];
        let actions = transition(
            Some(&active("D1", "p1", "vlc.exe")),
            &presets,
            &HashMap::new(),
            None,
        );
        assert_eq!(
            actions,
            vec![AutoAction::Revert {
                edid_id: "D1".into(),
                preset_id: "p1".into()
            }]
        );
    }

    #[test]
    fn no_active_and_unmapped_is_a_noop() {
        let presets = [preset("p1", "Cinema", &["vlc.exe"])];
        let actions = transition(
            None,
            &presets,
            &HashMap::new(),
            Some(&fg("explorer.exe", "D1")),
        );
        assert!(actions.is_empty());
    }

    #[test]
    fn moving_window_reverts_old_and_applies_new() {
        let presets = [preset("p1", "Cinema", &["vlc.exe"])];
        let actions = transition(
            Some(&active("D1", "p1", "vlc.exe")),
            &presets,
            &HashMap::new(),
            Some(&fg("vlc.exe", "D2")),
        );
        assert_eq!(
            actions,
            vec![
                AutoAction::Revert {
                    edid_id: "D1".into(),
                    preset_id: "p1".into()
                },
                AutoAction::Apply {
                    edid_id: "D2".into(),
                    preset_id: "p1".into(),
                    program: "vlc.exe".into()
                }
            ]
        );
    }

    #[test]
    fn pinned_target_skips_and_reverts_previous() {
        let presets = [preset("p1", "Cinema", &["vlc.exe"])];
        let mut pins = HashMap::new();
        pins.insert("D2".to_string(), "other".to_string());
        let actions = transition(
            Some(&active("D1", "p1", "vlc.exe")),
            &presets,
            &pins,
            Some(&fg("vlc.exe", "D2")),
        );
        assert_eq!(
            actions,
            vec![
                AutoAction::Revert {
                    edid_id: "D1".into(),
                    preset_id: "p1".into()
                },
                AutoAction::Skip {
                    edid_id: "D2".into(),
                    preset_id: "p1".into(),
                    program: "vlc.exe".into(),
                    reason: "monitor is pinned".into()
                }
            ]
        );
    }

    #[test]
    fn pinned_target_without_active_state_only_skips() {
        let presets = [preset("p1", "Cinema", &["vlc.exe"])];
        let mut pins = HashMap::new();
        pins.insert("D2".to_string(), "other".to_string());
        let actions = transition(None, &presets, &pins, Some(&fg("vlc.exe", "D2")));
        assert_eq!(
            actions,
            vec![AutoAction::Skip {
                edid_id: "D2".into(),
                preset_id: "p1".into(),
                program: "vlc.exe".into(),
                reason: "monitor is pinned".into()
            }]
        );
    }

    #[test]
    fn same_preset_same_monitor_is_a_noop() {
        let presets = [preset("p1", "Cinema", &["vlc.exe", "mpc.exe"])];
        let actions = transition(
            Some(&active("D1", "p1", "vlc.exe")),
            &presets,
            &HashMap::new(),
            Some(&fg("mpc.exe", "D1")),
        );
        assert!(actions.is_empty());
    }
}
