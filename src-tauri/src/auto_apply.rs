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
    let base = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed).trim();
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

/// Look up the prior manually applied preset for `edid_id` (from `applied.json`),
/// falling back to legacy base-key matching. Returns `None` when no manual preset
/// was applied or the referenced preset no longer exists (caller resets to defaults).
pub fn revert_target<'a>(
    edid_id: &str,
    applied: &HashMap<String, String>,
    presets: &'a [Preset],
) -> Option<&'a Preset> {
    let preset_id = applied.get(edid_id).or_else(|| {
        applied
            .iter()
            .find(|(k, _)| crate::monitor::key_match(k, edid_id))
            .map(|(_, v)| v)
    })?;
    presets.iter().find(|p| p.id == *preset_id)
}

/// Multi-monitor transition engine.
///
/// Tracks concurrent auto-applied monitors (`active: HashMap<edid_id, ActiveAuto>`):
/// - Any active monitor that became pinned reverts immediately (pin wins).
/// - Any non-foreground active monitor stays active as long as its mapped program
///   still has a visible, non-minimized window on that monitor (`visible`); once
///   closed, minimized, or moved away, that monitor reverts.
/// - On the foreground monitor:
///   - If unmapped (or `foreground` is `None`), revert that monitor if active.
///   - If mapped to a preset:
///     - Pinned monitor -> `Skip` (pin wins).
///     - Already running the same preset -> no-op.
///     - Otherwise apply the new preset directly (avoiding an identity flash).
#[allow(dead_code)]
pub fn transition_multi(
    active: &HashMap<String, ActiveAuto>,
    presets: &[Preset],
    pins: &HashMap<String, String>,
    foreground: Option<&ObservedForeground>,
    visible: &[ObservedForeground],
) -> Vec<AutoAction> {
    let mut actions = Vec::new();
    let fg_mapped = foreground.and_then(|fg| {
        let preset = resolve_mapping(presets, &fg.program)?;
        let program = normalize_program_name(&fg.program).unwrap_or_else(|| fg.program.clone());
        Some((fg, preset, program))
    });

    // Deterministic order across monitors
    let mut active_edids: Vec<&String> = active.keys().collect();
    active_edids.sort();

    for edid in active_edids {
        let current = &active[edid];
        let is_pinned = pins.contains_key(edid);
        let is_fg_monitor = foreground.is_some_and(|fg| fg.edid_id == *edid);

        if is_pinned {
            actions.push(AutoAction::Revert {
                edid_id: edid.clone(),
                preset_id: current.preset_id.clone(),
            });
            continue;
        }

        if is_fg_monitor {
            // Foreground monitor: if the foreground app is mapped to another preset
            // on this same monitor, the Apply step below will overwrite it directly
            // without flashing to identity first. If unmapped, revert this monitor.
            if fg_mapped.is_none() {
                actions.push(AutoAction::Revert {
                    edid_id: edid.clone(),
                    preset_id: current.preset_id.clone(),
                });
            }
            continue;
        }

        // Non-foreground monitor: keep active while ANY program mapped to its
        // current preset still has a visible non-minimized window on this display.
        let preset_still_visible = presets
            .iter()
            .find(|p| p.id == current.preset_id)
            .is_some_and(|p| {
                visible.iter().any(|w| {
                    w.edid_id == *edid
                        && normalize_program_name(&w.program)
                            .is_some_and(|name| p.attached_programs.iter().any(|m| m == &name))
                })
            });

        if !preset_still_visible {
            actions.push(AutoAction::Revert {
                edid_id: edid.clone(),
                preset_id: current.preset_id.clone(),
            });
        }
    }

    if let Some((fg, preset, program)) = fg_mapped {
        if pins.contains_key(&fg.edid_id) {
            actions.push(AutoAction::Skip {
                edid_id: fg.edid_id.clone(),
                preset_id: preset.id.clone(),
                program,
                reason: "monitor is pinned".into(),
            });
            return actions;
        }

        let already_same = active
            .get(&fg.edid_id)
            .is_some_and(|a| a.preset_id == preset.id);
        if !already_same {
            actions.push(AutoAction::Apply {
                edid_id: fg.edid_id.clone(),
                preset_id: preset.id.clone(),
                program,
            });
        }
    }

    actions
}

/// Decide what to do for the current foreground window (single-monitor helper).
#[allow(dead_code)]
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

    let program =
        normalize_program_name(&foreground.program).unwrap_or_else(|| foreground.program.clone());

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
            shortcut: None,
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
        assert_eq!(
            normalize_program_name("  chrome.exe "),
            Some("chrome.exe".into())
        );
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

    // ── Phase 2: smart revert & multi-monitor concurrent auto-apply ─────

    #[test]
    fn revert_target_resolves_exact_and_legacy_applied_preset() {
        let presets = [
            preset("p1", "Desktop Warm", &[]),
            preset("p2", "Cinema", &["vlc.exe"]),
        ];
        let mut applied = HashMap::new();
        applied.insert("EDID:AAA-111-222".to_string(), "p1".to_string());

        assert_eq!(
            revert_target("EDID:AAA-111-222", &applied, &presets).map(|p| p.id.as_str()),
            Some("p1")
        );
        // Legacy base-key fallback for port-discriminated monitor key (`|...`)
        assert_eq!(
            revert_target(r"EDID:AAA-111-222|MON\1", &applied, &presets).map(|p| p.id.as_str()),
            Some("p1")
        );
        // Unapplied monitor returns None -> caller resets to identity
        assert!(revert_target("EDID:OTHER", &applied, &presets).is_none());
    }

    #[test]
    fn multi_keeps_second_monitor_active_while_window_stays_visible() {
        let presets = [
            preset("p1", "Cinema", &["vlc.exe"]),
            preset("p2", "Reading", &["chrome.exe"]),
        ];
        let mut act = HashMap::new();
        act.insert("D2".into(), active("D2", "p1", "vlc.exe"));

        // User focuses unmapped explorer.exe on D1 while vlc.exe stays visible on D2
        let visible = [fg("vlc.exe", "D2"), fg("explorer.exe", "D1")];
        let actions = transition_multi(
            &act,
            &presets,
            &HashMap::new(),
            Some(&fg("explorer.exe", "D1")),
            &visible,
        );
        assert!(
            actions.is_empty(),
            "D2 must keep Cinema while vlc.exe remains visible there: {actions:?}"
        );
    }

    #[test]
    fn multi_reverts_second_monitor_when_window_closes_or_minimizes() {
        let presets = [preset("p1", "Cinema", &["vlc.exe"])];
        let mut act = HashMap::new();
        act.insert("D2".into(), active("D2", "p1", "vlc.exe"));

        // vlc.exe is no longer in visible windows
        let visible = [fg("explorer.exe", "D1")];
        let actions = transition_multi(
            &act,
            &presets,
            &HashMap::new(),
            Some(&fg("explorer.exe", "D1")),
            &visible,
        );
        assert_eq!(
            actions,
            vec![AutoAction::Revert {
                edid_id: "D2".into(),
                preset_id: "p1".into()
            }]
        );
    }

    #[test]
    fn multi_supports_concurrent_presets_on_two_monitors() {
        let presets = [
            preset("p1", "Cinema", &["vlc.exe"]),
            preset("p2", "Reading", &["chrome.exe"]),
        ];
        let mut act = HashMap::new();
        act.insert("D2".into(), active("D2", "p1", "vlc.exe"));

        // User focuses chrome.exe on D1 while vlc.exe remains visible on D2
        let visible = [fg("vlc.exe", "D2"), fg("chrome.exe", "D1")];
        let actions = transition_multi(
            &act,
            &presets,
            &HashMap::new(),
            Some(&fg("chrome.exe", "D1")),
            &visible,
        );
        assert_eq!(
            actions,
            vec![AutoAction::Apply {
                edid_id: "D1".into(),
                preset_id: "p2".into(),
                program: "chrome.exe".into()
            }]
        );
    }

    #[test]
    fn multi_window_drag_reverts_old_monitor_and_applies_new() {
        let presets = [preset("p1", "Cinema", &["vlc.exe"])];
        let mut act = HashMap::new();
        act.insert("D1".into(), active("D1", "p1", "vlc.exe"));

        // vlc.exe moved from D1 to D2
        let visible = [fg("vlc.exe", "D2")];
        let actions = transition_multi(
            &act,
            &presets,
            &HashMap::new(),
            Some(&fg("vlc.exe", "D2")),
            &visible,
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
}
