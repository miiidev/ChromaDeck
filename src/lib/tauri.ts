/// Tauri bridge — typed invoke wrappers.
// Mirror the Rust backend commands defined in src-tauri/src/monitor.rs and store.rs.

import { invoke } from "@tauri-apps/api/core";
import type { ApplyResult, CapturedState, EnforceEvent, Monitor, Preset, PresetInput } from "./types";

/** Fetch the list of connected monitors with their EDID identifiers. */
export async function listMonitors(): Promise<Monitor[]> {
  return invoke<Monitor[]>("list_monitors_cmd");
}

// ── Preset store ───────────────────────────────────────────────────────────

/** Fetch all stored presets. */
export async function listPresets(): Promise<Preset[]> {
  return invoke<Preset[]>("list_presets_cmd");
}

/** Create a new preset from user-supplied settings. */
export async function createPreset(input: PresetInput): Promise<Preset> {
  return invoke<Preset>("create_preset_cmd", { input });
}

/** Update an existing preset identified by its UUID. */
export async function updatePreset(
  id: string,
  input: PresetInput,
): Promise<Preset> {
  return invoke<Preset>("update_preset_cmd", { id, input });
}

/** Delete a preset by its UUID.  Returns the deleted preset. */
export async function deletePreset(id: string): Promise<Preset> {
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
 * Returns an ApplyResult indicating which steps succeeded.
 */
export async function applyPreset(id: string, targetEdid: string): Promise<ApplyResult> {
  return invoke<ApplyResult>("apply_preset_cmd", { id, targetEdid });
}

/**
 * Reset a monitor to system defaults (identity gamma ramp).
 * ICC associations are left untouched.
 */
export async function resetMonitor(edidId: string): Promise<ApplyResult> {
  // Tauri v2 maps camelCase JS keys to snake_case Rust params
  // (cf. importIcc/srcPath); the backend expects `edidId`.
  return invoke<ApplyResult>("reset_monitor_cmd", { edidId });
}

/**
 * Probe whether a monitor supports NVIDIA vibrance/hue control.
 * Resolves by EDID; arg name must be camelCase (Tauri v2 convention).
 */
export async function vibranceSupported(edidId: string): Promise<boolean> {
  return invoke<boolean>("vibrance_supported_cmd", { edidId });
}

// ── Pin enforcement ─────────────────────────────────────────────────────────

/** Pin a preset as the enforced default for its monitor. */
export async function pinPreset(edidId: string, presetId: string): Promise<void> {
  return invoke<void>("pin_preset_cmd", { edidId, presetId });
}

/** Remove enforcement for a monitor (no-op when unpinned). */
export async function unpinMonitor(edidId: string): Promise<void> {
  return invoke<void>("unpin_monitor_cmd", { edidId });
}

/** Map of edid_id -> preset_id for pinned monitors. */
export async function listPins(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("list_pins_cmd");
}

/** Set a monitor's display name (empty clears back to default). */
export async function setMonitorName(edidId: string, alias: string): Promise<void> {
  return invoke<void>("set_monitor_name_cmd", { edidId, alias });
}

/** Force a full enforce pass now; returns per-pin outcomes. */
export async function reapplyNow(): Promise<EnforceEvent[]> {
  return invoke<EnforceEvent[]>("reapply_now_cmd");
}

/** Read NVCP/driver live state into editor-fillable values. */
export async function captureNvcp(edidId: string): Promise<CapturedState> {
  return invoke<CapturedState>("capture_nvcp_cmd", { edidId });
}
