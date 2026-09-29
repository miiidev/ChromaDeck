/// Tauri bridge — typed invoke wrappers.
// Mirror the Rust backend commands defined in src-tauri/src/monitor.rs and store.rs.

import { invoke } from "@tauri-apps/api/core";
import type { ApplyResult, Monitor, Preset, PresetInput } from "./types";

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
 * Apply a colour preset to its target monitor.
 * Returns an ApplyResult indicating which steps succeeded.
 */
export async function applyPreset(id: string): Promise<ApplyResult> {
  return invoke<ApplyResult>("apply_preset_cmd", { id });
}

/**
 * Reset a monitor to system defaults (identity gamma ramp).
 * ICC associations are left untouched.
 */
export async function resetMonitor(edidId: string): Promise<ApplyResult> {
  // Note: arg name must match the Rust param `edid_id` exactly.
  return invoke<ApplyResult>("reset_monitor_cmd", { edid_id: edidId });
}