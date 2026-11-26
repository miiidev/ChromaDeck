// ── CLI automation (Stream Deck / AutoHotkey / scripts) ─────────────────────
//
// Parses argv passed on cold launch or via `tauri-plugin-single-instance` when
// a second `chromadeck.exe` invocation occurs. Commands execute silently in the
// background without stealing focus from fullscreen games.

use tauri::{AppHandle, Emitter, Manager};

use crate::auto_apply_runtime;
use crate::monitor::Monitor;
use crate::store::{AppStore, Preset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoApplyArg {
    On,
    Off,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Apply {
        preset: String,
        monitor: Option<String>,
        pin: bool,
    },
    Reset {
        monitor: Option<String>,
        clear_icc: bool,
    },
    Reapply,
    AutoApply(AutoApplyArg),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedCli {
    /// Start hidden in tray without showing the main window.
    pub headless: bool,
    /// Actionable command requested on the command line.
    pub command: Option<CliCommand>,
    /// Parse error (logged to stderr; never panics).
    pub error: Option<String>,
}

/// Pure argument parser (`args[0]` is typically the executable path and skipped).
pub fn parse_cli_args(args: &[String]) -> ParsedCli {
    let mut out = ParsedCli::default();
    if args.len() <= 1 {
        return out;
    }

    let mut apply_preset: Option<String> = None;
    let mut reset = false;
    let mut reapply = false;
    let mut auto_apply: Option<AutoApplyArg> = None;
    let mut monitor: Option<String> = None;
    let mut pin = false;
    let mut clear_icc = false;

    let mut i = 1;
    while i < args.len() {
        let raw = args[i].as_str();
        let (flag, inline_val) = match raw.split_once('=') {
            Some((f, v)) => (f, Some(v)),
            None => (raw, None),
        };

        let mut take_value = |flag_name: &str| -> Option<String> {
            if let Some(v) = inline_val {
                if v.is_empty() {
                    out.error = Some(format!("missing value for {flag_name}"));
                    None
                } else {
                    Some(v.to_string())
                }
            } else if i + 1 < args.len() {
                i += 1;
                Some(args[i].clone())
            } else {
                out.error = Some(format!("missing value for {flag_name}"));
                None
            }
        };

        match flag {
            "--headless" | "--quiet" => out.headless = true,
            "--pin" => pin = true,
            "--clear-icc" => clear_icc = true,
            "--reapply" => reapply = true,
            "--reset" => reset = true,
            "--reset-all" => {
                reset = true;
                monitor = Some("all".into());
            }
            "--apply" => {
                apply_preset = take_value("--apply");
            }
            "--monitor" | "-m" => {
                monitor = take_value(flag);
            }
            "--auto-apply" => {
                if let Some(v) = take_value("--auto-apply") {
                    match v.to_ascii_lowercase().as_str() {
                        "on" | "true" | "1" => auto_apply = Some(AutoApplyArg::On),
                        "off" | "false" | "0" => auto_apply = Some(AutoApplyArg::Off),
                        "toggle" => auto_apply = Some(AutoApplyArg::Toggle),
                        other => {
                            out.error = Some(format!(
                                "invalid --auto-apply value '{other}' (expected on, off, or toggle)"
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    if out.error.is_some() {
        return out;
    }

    if let Some(preset) = apply_preset {
        out.command = Some(CliCommand::Apply {
            preset,
            monitor,
            pin,
        });
    } else if reset {
        out.command = Some(CliCommand::Reset { monitor, clear_icc });
    } else if reapply {
        out.command = Some(CliCommand::Reapply);
    } else if let Some(mode) = auto_apply {
        out.command = Some(CliCommand::AutoApply(mode));
    }

    out
}

/// Match a preset by UUID, exact name (case-insensitive), or title after `" - "`.
pub fn resolve_preset<'a>(query: &str, presets: &'a [Preset]) -> Option<&'a Preset> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    if let Some(p) = presets.iter().find(|p| p.id == q) {
        return Some(p);
    }
    if let Some(p) = presets.iter().find(|p| p.name.eq_ignore_ascii_case(q)) {
        return Some(p);
    }
    // Fall back to matching the title portion of "Category - Title" if unambiguous
    let title_matches: Vec<&Preset> = presets
        .iter()
        .filter(|p| {
            p.name
                .split_once(" - ")
                .is_some_and(|(_, title)| title.eq_ignore_ascii_case(q))
        })
        .collect();
    if title_matches.len() == 1 {
        Some(title_matches[0])
    } else {
        None
    }
}

/// Resolve a `--monitor` specification into connected monitor EDIDs.
///
/// - `None` → foreground window's monitor if connected, else first connected monitor.
/// - `"all"` → every connected monitor.
/// - `"1"`, `"2"`, … → 1-based index into connected monitors.
/// - Otherwise matches alias, model, device name, or EDID (`key_match`).
pub fn resolve_monitor_targets(
    spec: Option<&str>,
    monitors: &[Monitor],
    foreground_edid: Option<&str>,
) -> Result<Vec<String>, String> {
    let connected: Vec<&Monitor> = monitors.iter().filter(|m| m.connected).collect();
    if connected.is_empty() {
        return Err("no connected monitors found".into());
    }

    let Some(raw) = spec.map(str::trim).filter(|s| !s.is_empty()) else {
        if let Some(fg) = foreground_edid {
            if let Some(m) = connected
                .iter()
                .find(|m| crate::monitor::key_match(&m.edid_id, fg))
            {
                return Ok(vec![m.edid_id.clone()]);
            }
        }
        return Ok(vec![connected[0].edid_id.clone()]);
    };

    if raw.eq_ignore_ascii_case("all") {
        return Ok(connected.iter().map(|m| m.edid_id.clone()).collect());
    }

    if let Ok(index) = raw.parse::<usize>() {
        if index >= 1 && index <= connected.len() {
            return Ok(vec![connected[index - 1].edid_id.clone()]);
        }
        return Err(format!(
            "monitor index {index} out of range (1..={})",
            connected.len()
        ));
    }

    if let Some(m) = connected.iter().find(|m| {
        (!m.alias.is_empty() && m.alias.eq_ignore_ascii_case(raw))
            || m.model.eq_ignore_ascii_case(raw)
            || m.device_name.eq_ignore_ascii_case(raw)
            || crate::monitor::key_match(&m.edid_id, raw)
    }) {
        return Ok(vec![m.edid_id.clone()]);
    }

    Err(format!("no connected monitor matched '{raw}'"))
}

/// Resolve the EDID of the monitor currently holding the foreground window.
pub fn foreground_monitor_edid(monitors: &[Monitor]) -> Option<String> {
    let fg = crate::foreground::foreground_window()?;
    let device = fg.device_name?;
    monitors
        .iter()
        .find(|m| m.connected && m.device_name.eq_ignore_ascii_case(&device))
        .map(|m| m.edid_id.clone())
}

/// Execute a parsed CLI command silently against the running app state.
pub fn execute_cli(app: &AppHandle, cmd: CliCommand) -> Result<String, String> {
    let store_state = app
        .try_state::<AppStore>()
        .ok_or_else(|| "store unavailable".to_string())?;

    match cmd {
        CliCommand::AutoApply(mode) => {
            let current = store_state
                .0
                .lock()
                .map_err(|e| e.to_string())?
                .auto_apply_enabled();
            let next = match mode {
                AutoApplyArg::On => true,
                AutoApplyArg::Off => false,
                AutoApplyArg::Toggle => !current,
            };
            auto_apply_runtime::set_auto_apply(app, next)?;
            crate::tray::refresh_tray(app);
            Ok(format!("auto-apply {}", if next { "on" } else { "off" }))
        }
        CliCommand::Reapply => {
            let _op = auto_apply_runtime::DISPLAY_OPERATION
                .lock()
                .map_err(|e| e.to_string())?;
            let (pins, presets) = {
                let s = store_state.0.lock().map_err(|e| e.to_string())?;
                (s.list_pins(), s.list_presets())
            };
            let api = crate::color::RealColorApi;
            let nv = crate::nvapi::RealNvapi;
            let reader = crate::enforce::RealStateReader;
            let events = crate::enforce::check_once(&api, &nv, &reader, &pins, &presets, true);
            let applied = events.iter().filter(|e| e.applied).count();
            Ok(format!("reapplied {applied}/{}", events.len()))
        }
        CliCommand::Apply {
            preset: query,
            monitor,
            pin,
        } => {
            let _op = auto_apply_runtime::DISPLAY_OPERATION
                .lock()
                .map_err(|e| e.to_string())?;
            let (preset, profiles_dir, names) = {
                let s = store_state.0.lock().map_err(|e| e.to_string())?;
                let presets = s.list_presets();
                let found = resolve_preset(&query, &presets)
                    .cloned()
                    .ok_or_else(|| format!("preset not found: '{query}'"))?;
                let dir = s.data_dir().join("profiles").to_string_lossy().into_owned();
                (found, dir, s.list_monitor_names())
            };
            let mut monitors = crate::monitor::list_monitors();
            for m in &mut monitors {
                if let Some(a) = names.get(&m.edid_id) {
                    m.alias = a.clone();
                }
            }
            let fg_edid = foreground_monitor_edid(&monitors);
            let targets = resolve_monitor_targets(monitor.as_deref(), &monitors, fg_edid.as_deref())?;
            let api = crate::color::RealColorApi;
            let nv = crate::nvapi::RealNvapi;

            let mut applied_count = 0usize;
            for edid in &targets {
                auto_apply_runtime::relinquish_manual(app, edid);
                let outcome =
                    crate::color::apply_preset_detailed(&api, &nv, &preset, edid, &profiles_dir);
                if outcome.result.error.is_none() {
                    applied_count += 1;
                    if let Ok(mut s) = store_state.0.lock() {
                        let _ = s.record_applied(edid, &preset.id);
                        if pin {
                            let _ = s.pin_preset(edid, &preset.id);
                        }
                    }
                }
            }
            crate::tray::refresh_tray(app);
            let _ = app.emit(crate::tray::STORE_CHANGED_EVENT, ());
            Ok(format!(
                "applied '{}' to {applied_count}/{} monitor(s)",
                preset.name,
                targets.len()
            ))
        }
        CliCommand::Reset { monitor, clear_icc } => {
            let _op = auto_apply_runtime::DISPLAY_OPERATION
                .lock()
                .map_err(|e| e.to_string())?;
            let (names, profiles_dir) = {
                let s = store_state.0.lock().map_err(|e| e.to_string())?;
                (s.list_monitor_names(), s.data_dir().join("profiles"))
            };
            let mut monitors = crate::monitor::list_monitors();
            for m in &mut monitors {
                if let Some(a) = names.get(&m.edid_id) {
                    m.alias = a.clone();
                }
            }
            let fg_edid = foreground_monitor_edid(&monitors);
            let is_all = monitor
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("all"));
            let targets =
                resolve_monitor_targets(monitor.as_deref(), &monitors, fg_edid.as_deref())?;
            let icc_paths = if clear_icc {
                crate::color::managed_icc_paths(&profiles_dir)
            } else {
                Vec::new()
            };
            let api = crate::color::RealColorApi;
            let nv = crate::nvapi::RealNvapi;

            let mut reset_count = 0usize;
            for edid in &targets {
                auto_apply_runtime::relinquish_manual(app, edid);
                let res = crate::color::reset_monitor_with_icc(&api, &nv, edid, &icc_paths);
                if res.error.is_none() {
                    reset_count += 1;
                    if let Ok(mut s) = store_state.0.lock() {
                        let _ = s.clear_applied(edid);
                        let _ = s.unpin_monitor(edid);
                    }
                }
            }
            if is_all {
                if let Ok(mut s) = store_state.0.lock() {
                    let _ = s.clear_all_pins();
                }
            }
            crate::tray::refresh_tray(app);
            let _ = app.emit(crate::tray::STORE_CHANGED_EVENT, ());
            Ok(format!("reset {reset_count}/{} monitor(s)", targets.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| x.to_string()).collect()
    }

    fn mon(edid: &str, model: &str, alias: &str, connected: bool) -> Monitor {
        Monitor {
            edid_id: edid.into(),
            model: model.into(),
            serial: String::new(),
            connected,
            device_name: r"\\.\DISPLAY1".into(),
            alias: alias.into(),
            instance_path: String::new(),
            edid_present: true,
            edid_len: 128,
        }
    }

    fn preset(id: &str, name: &str) -> Preset {
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
            attached_programs: vec![],
            shortcut: None,
        }
    }

    #[test]
    fn parses_apply_with_monitor_and_pin() {
        let parsed = parse_cli_args(&s(&[
            "chromadeck.exe",
            "--apply",
            "FPS Vibrance",
            "--monitor",
            "2",
            "--pin",
        ]));
        assert_eq!(parsed.error, None);
        assert_eq!(
            parsed.command,
            Some(CliCommand::Apply {
                preset: "FPS Vibrance".into(),
                monitor: Some("2".into()),
                pin: true,
            })
        );
    }

    #[test]
    fn parses_equals_syntax_and_headless_flag() {
        let parsed = parse_cli_args(&s(&[
            "chromadeck.exe",
            "--headless",
            "--apply=Warm",
            "-m",
            "all",
        ]));
        assert!(parsed.headless);
        assert_eq!(
            parsed.command,
            Some(CliCommand::Apply {
                preset: "Warm".into(),
                monitor: Some("all".into()),
                pin: false,
            })
        );
    }

    #[test]
    fn parses_reset_reapply_and_auto_apply() {
        assert_eq!(
            parse_cli_args(&s(&["chromadeck.exe", "--reset-all", "--clear-icc"])).command,
            Some(CliCommand::Reset {
                monitor: Some("all".into()),
                clear_icc: true,
            })
        );
        assert_eq!(
            parse_cli_args(&s(&["chromadeck.exe", "--reapply"])).command,
            Some(CliCommand::Reapply)
        );
        assert_eq!(
            parse_cli_args(&s(&["chromadeck.exe", "--auto-apply", "toggle"])).command,
            Some(CliCommand::AutoApply(AutoApplyArg::Toggle))
        );
    }

    #[test]
    fn resolves_preset_by_id_name_or_unambiguous_title() {
        let list = [
            preset("id-1", "Movie - Warm"),
            preset("id-2", "Gaming - Competitive"),
        ];
        assert_eq!(resolve_preset("id-2", &list).map(|p| p.id.as_str()), Some("id-2"));
        assert_eq!(
            resolve_preset("movie - warm", &list).map(|p| p.id.as_str()),
            Some("id-1")
        );
        assert_eq!(
            resolve_preset("warm", &list).map(|p| p.id.as_str()),
            Some("id-1")
        );
        assert!(resolve_preset("missing", &list).is_none());
    }

    #[test]
    fn resolves_monitor_targets_by_foreground_index_alias_and_all() {
        let monitors = [
            mon("EDID:1", "LG UltraGear", "Main", true),
            mon("EDID:2", "AOC 2403", "Side", true),
            mon("EDID:3", "BenQ", "", false),
        ];
        // None -> prefers foreground display
        assert_eq!(
            resolve_monitor_targets(None, &monitors, Some("EDID:2")).unwrap(),
            vec!["EDID:2"]
        );
        // 1-based index
        assert_eq!(
            resolve_monitor_targets(Some("1"), &monitors, None).unwrap(),
            vec!["EDID:1"]
        );
        // Alias (case-insensitive)
        assert_eq!(
            resolve_monitor_targets(Some("side"), &monitors, None).unwrap(),
            vec!["EDID:2"]
        );
        // All connected (skips offline BenQ)
        assert_eq!(
            resolve_monitor_targets(Some("all"), &monitors, None).unwrap(),
            vec!["EDID:1", "EDID:2"]
        );
        // Out of range index errors cleanly
        assert!(resolve_monitor_targets(Some("3"), &monitors, None).is_err());
    }
}
