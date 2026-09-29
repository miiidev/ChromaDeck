/// Tauri bridge — typed invoke wrappers.
// Mirror the Rust backend commands defined in src-tauri/src/monitor.rs et al.

import { invoke } from "@tauri-apps/api/core";
import type { Monitor } from "./types";

/** Fetch the list of connected monitors with their EDID identifiers. */
export async function listMonitors(): Promise<Monitor[]> {
  return invoke<Monitor[]>("list_monitors_cmd");
}