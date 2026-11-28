/// Tauri bridge — typed invoke wrappers.
// Mirror the Rust backend commands defined in src-tauri/src/monitor.rs and store.rs.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DEMO_APPLIED, DEMO_MONITORS, DEMO_PINS, DEMO_PRESETS } from "./demoData";

/** Browser preview only: no Tauri IPC available. */
const isBrowserPreview = () => !isTauri();
import type {
  ApplyResult,
  CapturedState,
  DdcciProbe,
  DiagnosticsExportSummary,
  DisplayCapabilities,
  EnforceEvent,
  ExportSummary,
  ImportSummary,
  Monitor,
  MonitorProbe,
  Preset,
  PresetInput,
  ScheduleRule,
} from "./types";

/** Fetch the list of connected monitors with their EDID identifiers. */
export async function listMonitors(): Promise<Monitor[]> {
  if (isBrowserPreview()) return DEMO_MONITORS;
  return invoke<Monitor[]>("list_monitors_cmd");
}

/**
 * Show a numbered overlay on each connected monitor to identify them.
 * Returns the 1‑based monitor numbers that were shown (may be fewer than
 * connected if some cannot be located via Win32).
 */
export async function identifyMonitors(): Promise<number[]> {
  if (isBrowserPreview()) return [];
  return invoke<number[]>("identify_monitors_cmd");
}

/** Overlay payload for a 1-based monitor number: name, total, chain. */
export interface IdentifyInfo {
  name: string;
  total: number;
  /** Raw identity chain line (machine-parsable diagnostics evidence). */
  chain?: string;
}

/**
 * Resolve overlay info for the 1-based monitor number `n` (same ordering
 * as `identifyMonitors`). Called by identify overlay windows on mount.
 */
export async function getIdentifyInfo(n: number): Promise<IdentifyInfo> {
  return invoke<IdentifyInfo>("identify_info_cmd", { n });
}

// ── Preset store ───────────────────────────────────────────────────────────

/** Fetch all stored presets. */
export async function listPresets(): Promise<Preset[]> {
  if (isBrowserPreview()) return DEMO_PRESETS;
  return invoke<Preset[]>("list_presets_cmd");
}

/** Create a new preset from user-supplied settings. */
export async function createPreset(input: PresetInput): Promise<Preset> {
  if (isBrowserPreview()) {
    return {
      ...input,
      id: `demo-${Date.now()}`,
      icc_hash: "",
      icc_filename: "",
    };
  }
  return invoke<Preset>("create_preset_cmd", { input });
}

/** Update an existing preset identified by its UUID. */
export async function updatePreset(
  id: string,
  input: PresetInput,
): Promise<Preset> {
  if (isBrowserPreview()) {
    return { ...input, id, icc_hash: "", icc_filename: "" };
  }
  return invoke<Preset>("update_preset_cmd", { id, input });
}

/** Delete a preset by its UUID.  Returns the deleted preset. */
export async function deletePreset(id: string): Promise<Preset> {
  if (isBrowserPreview()) {
    return DEMO_PRESETS.find((p) => p.id === id) ?? DEMO_PRESETS[0];
  }
  return invoke<Preset>("delete_preset_cmd", { id });
}

/**
 * Import an ICC profile from the filesystem into the app store.
 * Returns the SHA-256 hex hash of the copied profile.
 */
export async function importIcc(srcPath: string): Promise<string> {
  return invoke<string>("import_icc_cmd", { srcPath });
}

// ── Preset apply ───────────────────────────────────────────────────────────

/**
 * Apply a colour preset to a target monitor.
 * Returns an ApplyResult indicating which steps succeeded, plus additive
 * per-adjustment outcomes (applied/skipped/failed) when the backend reports
 * them. Legacy fields keep their original meaning.
 */
export async function applyPreset(id: string, targetEdid: string): Promise<ApplyResult> {
  if (isBrowserPreview()) {
    return { icc_applied: false, gamma_applied: true, vibrance_applied: true };
  }
  return invoke<ApplyResult>("apply_preset_cmd", { id, targetEdid });
}

/**
 * Apply transient slider values from the editor onto a connected display
 * without persisting or changing ICC profile associations.
 */
export async function previewPreset(input: PresetInput, targetEdid: string): Promise<ApplyResult> {
  if (isBrowserPreview()) {
    return { icc_applied: false, gamma_applied: true, vibrance_applied: true };
  }
  return invoke<ApplyResult>("preview_preset_cmd", { input, targetEdid });
}

/**
 * Restore a display after ending live editor preview: re-applies its active
 * pinned/applied preset if one exists, otherwise resets to defaults.
 */
export async function restorePreview(targetEdid: string): Promise<ApplyResult> {
  if (isBrowserPreview()) {
    return { icc_applied: false, gamma_applied: true, vibrance_applied: true };
  }
  return invoke<ApplyResult>("restore_preview_cmd", { targetEdid });
}

/**
 * Reset a monitor to system defaults (identity gamma ramp).
 * ICC associations are left untouched unless `clearIcc` is true.
 * Writes are verified by read-back: a panel that doesn't reflect the reset
 * surfaces an error instead of reporting false success.
 */
export async function resetMonitor(edidId: string, clearIcc = false): Promise<ApplyResult> {
  if (isBrowserPreview()) {
    return { icc_applied: false, gamma_applied: true, vibrance_applied: true };
  }
  // Tauri v2 maps camelCase JS keys to snake_case Rust params
  // (cf. importIcc/srcPath); the backend expects `edidId` / `clearIcc`.
  return invoke<ApplyResult>("reset_monitor_cmd", { edidId, clearIcc });
}

/** Wire shape of reset_all_monitors_cmd's outcome (snake_case on the wire).
 *  Each result carries its monitor key flattened next to the ApplyResult. */
export interface ResetAllOutcome {
  results: Array<ApplyResult & { edid_id: string }>;
  pins_removed: number;
}

/**
 * Reset every connected screen to default colors and remove all pins.
 * ICC associations are left untouched unless `clearIcc` is true.
 * Pins clear even when individual monitor resets fail (enforcer must never
 * repaint afterwards).
 */
export async function resetAllMonitors(clearIcc = false): Promise<ResetAllOutcome> {
  if (isBrowserPreview()) {
    return {
      pins_removed: Object.keys(DEMO_PINS).length,
      results: DEMO_MONITORS.filter((m) => m.connected).map((m) => ({
        edid_id: m.edid_id,
        icc_applied: false,
        gamma_applied: true,
        vibrance_applied: true,
      })),
    };
  }
  return invoke<ResetAllOutcome>("reset_all_monitors_cmd", { clearIcc });
}

/**
 * Read live hardware state for a monitor without changing anything:
 * gamma engine, vibrance/hue levels (when supported), OS ramp peak
 * and whether the ramp is identity. Answers "did it land?".
 */
export async function probeMonitor(edidId: string): Promise<MonitorProbe> {
  if (isBrowserPreview()) {
    return {
      engine: "preview",
      backend_id: "preview",
      vibrance_supported: false,
      vibrance: null,
      hue: null,
      ramp_peak: 1.0,
      ramp_identity: true,
    };
  }
  return invoke<MonitorProbe>("probe_monitor_cmd", { edidId });
}

/**
 * Probe whether a monitor supports NVIDIA vibrance/hue control.
 * Resolves by EDID; arg name must be camelCase (Tauri v2 convention).
 */
export async function vibranceSupported(edidId: string): Promise<boolean> {
  if (isBrowserPreview()) return false;
  return invoke<boolean>("vibrance_supported_cmd", { edidId });
}

/**
 * Per-adjustment capability report for a monitor: which backend applies the
 * preset and which adjustments it supports (with reasons when it does not).
 * Resolves by EDID; arg name must be camelCase (Tauri v2 convention).
 */
export async function getDisplayCapabilities(edidId: string): Promise<DisplayCapabilities> {
  if (isBrowserPreview()) {
    return {
      backend_id: "preview",
      backend_label: "Browser preview",
      adjustments: [],
    };
  }
  return invoke<DisplayCapabilities>("get_display_capabilities", { edidId });
}

/** Probe whether a monitor exposes DDC/CI luminance (VCP 0x10) control. */
export async function probeDdcci(edidId: string): Promise<DdcciProbe> {
  if (isBrowserPreview()) {
    const mon = DEMO_MONITORS.find((m) => m.edid_id === edidId);
    if (mon && mon.connected) {
      return { supported: true, current_percent: 65, reason: null };
    }
    return {
      supported: false,
      current_percent: null,
      reason: "Monitor is offline or does not expose DDC/CI",
    };
  }
  return invoke<DdcciProbe>("probe_ddcci_cmd", { edidId });
}

// ── Pin enforcement ─────────────────────────────────────────────────────────

/** Pin a preset as the enforced default for its monitor. */
export async function pinPreset(edidId: string, presetId: string): Promise<void> {
  if (isBrowserPreview()) return;
  return invoke<void>("pin_preset_cmd", { edidId, presetId });
}

/** Remove enforcement for a monitor (no-op when unpinned). */
export async function unpinMonitor(edidId: string): Promise<void> {
  if (isBrowserPreview()) return;
  return invoke<void>("unpin_monitor_cmd", { edidId });
}

/** Map of edid_id -> preset_id for pinned monitors. */
export async function listPins(): Promise<Record<string, string>> {
  if (isBrowserPreview()) return DEMO_PINS;
  return invoke<Record<string, string>>("list_pins_cmd");
}

/** Map of edid_id -> preset_id for last manual applies. Persisted across restarts. */
export async function listApplied(): Promise<Record<string, string>> {
  if (isBrowserPreview()) return DEMO_APPLIED;
  return invoke<Record<string, string>>("list_applied_cmd");
}

/** Set a monitor's display name (empty clears back to default). */
export async function setMonitorName(edidId: string, alias: string): Promise<void> {
  if (isBrowserPreview()) return;
  return invoke<void>("set_monitor_name_cmd", { edidId, alias });
}

/** Force a full enforce pass now; returns per-pin outcomes. */
export async function reapplyNow(): Promise<EnforceEvent[]> {
  if (isBrowserPreview()) {
    return DEMO_MONITORS.filter((m) => m.connected).map((m) => ({
      edid_id: m.edid_id,
      preset_id: DEMO_PINS[m.edid_id] ?? "",
      applied: true,
    }));
  }
  return invoke<EnforceEvent[]>("reapply_now_cmd");
}

/** Read NVCP/driver live state into editor-fillable values. */
export async function captureNvcp(edidId: string): Promise<CapturedState> {
  if (isBrowserPreview()) {
    return {
      brightness: 54,
      contrast: 52,
      gamma: 1.05,
      vibrance: 62,
      hue_deg: 0,
    };
  }
  return invoke<CapturedState>("capture_nvcp_cmd", { edidId });
}

// ── Auto-apply ──────────────────────────────────────────────────────────────

/** Whether the auto-apply master switch is on (persisted, default off). */
export async function getAutoApplyEnabled(): Promise<boolean> {
  if (isBrowserPreview()) return false;
  return invoke<boolean>("get_auto_apply_enabled_cmd");
}

/** Turn auto-apply on or off. Persists and syncs the tray checkbox. */
export async function setAutoApplyEnabled(enabled: boolean): Promise<void> {
  if (isBrowserPreview()) return;
  return invoke<void>("set_auto_apply_enabled_cmd", { enabled });
}

/** All monitor/preset pairs currently auto-applied. In-memory only. */
export async function listAutoApplied(): Promise<Array<{ edid_id: string; preset_id: string }>> {
  if (isBrowserPreview()) return [];
  return invoke<Array<{ edid_id: string; preset_id: string }>>("list_auto_applied_cmd");
}

/** List currently open user-facing program executables (lowercased base names). */
export async function listRunningPrograms(): Promise<string[]> {
  if (isBrowserPreview()) return ["chrome.exe", "code.exe", "cs2.exe", "vlc.exe"];
  return invoke<string[]>("list_running_programs_cmd");
}

// ── Global hotkeys ──────────────────────────────────────────────────────────

export interface GlobalHotkeySettings {
  reapply_shortcut?: string | null;
  cycle_shortcut?: string | null;
}

export interface HotkeyEvent {
  action: string;
  message: string;
  error?: string | null;
}

/** Temporarily suspend OS global hotkeys while recording a new chord. */
export async function setHotkeysSuspended(suspended: boolean): Promise<void> {
  if (isBrowserPreview()) return;
  return invoke<void>("set_hotkeys_suspended_cmd", { suspended });
}

/** Fetch persisted global hotkey bindings (Reapply / Cycle preset). */
export async function getGlobalHotkeys(): Promise<GlobalHotkeySettings> {
  if (isBrowserPreview()) return { reapply_shortcut: null, cycle_shortcut: null };
  return invoke<GlobalHotkeySettings>("get_global_hotkeys_cmd");
}

/** Persist and re-register global hotkey bindings. */
export async function setGlobalHotkeys(
  hotkeys: GlobalHotkeySettings,
): Promise<GlobalHotkeySettings> {
  if (isBrowserPreview()) return hotkeys;
  return invoke<GlobalHotkeySettings>("set_global_hotkeys_cmd", { hotkeys });
}

/**
 * One auto-apply outcome. `kind` is applied | reverted | skipped | error |
 * cleared; `error` carries the skip reason or failure text.
 */
export interface AutoApplyEvent {
  kind: "applied" | "reverted" | "skipped" | "error" | "cleared";
  edid_id: string;
  preset_id: string;
  preset_name: string;
  monitor_name: string;
  program: string;
  error: string | null;
  /** Set on `reverted` when returning to a prior manually applied preset. */
  restored_preset_name?: string | null;
}

// ── Auto-updater ─────────────────────────────────────────────────────────────

export interface UpdateInfo {
  available: boolean;
  version: string;
  notes: string | null;
  date: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
  percent: number | null;
}

/** Check whether a newer version is available at the update endpoint. */
export async function checkForUpdate(): Promise<UpdateInfo> {
  if (isBrowserPreview()) {
    return {
      available: false,
      version: "",
      notes: null,
      date: null,
    };
  }
  return invoke<UpdateInfo>("check_for_update_cmd");
}

/** Download and install the available update. The plugin verifies the
 *  Ed25519 signature before installing; installation exits the app. */
export async function downloadAndInstallUpdate(): Promise<void> {
  if (isBrowserPreview()) return;
  return invoke<void>("download_and_install_cmd");
}

/** Subscribe to download progress events (`update://progress`). */
export function onUpdateProgress(
  callback: (progress: UpdateProgress) => void,
): (() => void) {
  if (isBrowserPreview()) return () => {};
  const unlisten = listen<UpdateProgress>("update://progress", (event) => {
    callback(event.payload);
  });
  return () => unlisten.then((fn) => fn());
}

// ── Preset bundles (.chromadeck.json) ───────────────────────────────────────

/** Export selected presets (or all presets when `ids` is null) to a bundle file. */
export async function exportPresets(
  ids: string[] | null,
  destPath: string,
): Promise<ExportSummary> {
  if (isBrowserPreview()) {
    const count = ids ? ids.length : DEMO_PRESETS.length;
    return { exported_count: count, icc_count: 0, dest_path: destPath };
  }
  return invoke<ExportSummary>("export_presets_cmd", { ids, destPath });
}

/** Import presets and embedded ICC blobs from a `.chromadeck.json` bundle. */
export async function importPresets(srcPath: string): Promise<ImportSummary> {
  if (isBrowserPreview()) {
    return {
      imported_count: 1,
      icc_imported_count: 0,
      renamed_collisions: [],
      stripped_conflicting_programs: [],
      stripped_conflicting_shortcuts: [],
    };
  }
  return invoke<ImportSummary>("import_presets_cmd", { srcPath });
}

// ── Time-of-day schedules ───────────────────────────────────────────────────

const PREVIEW_SCHEDULES_KEY = "chromadeck.previewSchedules";

/** Fetch persisted per-monitor schedule rules. */
export async function listSchedules(): Promise<ScheduleRule[]> {
  if (isBrowserPreview()) {
    try {
      const raw = localStorage.getItem(PREVIEW_SCHEDULES_KEY);
      return raw ? (JSON.parse(raw) as ScheduleRule[]) : [];
    } catch {
      return [];
    }
  }
  return invoke<ScheduleRule[]>("list_schedules_cmd");
}

/** Validate and save per-monitor schedule rules. */
export async function saveSchedules(
  schedules: ScheduleRule[],
): Promise<ScheduleRule[]> {
  if (isBrowserPreview()) {
    const normalized = schedules.map((s, idx) => ({
      ...s,
      id: s.id || `preview-sched-${Date.now()}-${idx}`,
    }));
    try {
      localStorage.setItem(PREVIEW_SCHEDULES_KEY, JSON.stringify(normalized));
    } catch {
      // ignore storage failures in private contexts
    }
    return normalized;
  }
  return invoke<ScheduleRule[]>("save_schedules_cmd", { schedules });
}

// ── Diagnostics report export ───────────────────────────────────────────────

/** Export a redacted hardware + log diagnostics report to `destPath`. */
export async function exportDiagnostics(
  destPath: string,
): Promise<DiagnosticsExportSummary> {
  if (isBrowserPreview()) {
    return {
      dest_path: destPath,
      bytes_written: 2048,
      monitors_count: DEMO_MONITORS.length,
    };
  }
  return invoke<DiagnosticsExportSummary>("export_diagnostics_cmd", { destPath });
}
