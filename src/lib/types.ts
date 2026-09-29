/// Shared types for ChromaDeck presets and monitors.
// These mirror the Rust structs used in the Tauri backend.

export interface Monitor {
  edid_id: string;
  model: string;
  serial: string;
  connected: boolean;
  device_name: string;
}

export interface GammaRamp {
  red: number[];   // 256 values
  green: number[];
  blue: number[];
}

export interface Preset {
  id: string;
  name: string;
  edid_id: string;
  icc_hash: string;
  icc_filename: string;
  brightness: number;   // 0.0 – 1.0
  contrast: number;     // 0.0 – 1.0
  rgb_gains: [number, number, number]; // per-channel multiplier
  gamma: number;        // 1.0 – 3.0
  vibrance: number;     // 0 – 100, 50 neutral
  hue_deg: number;      // 0 – 359 degrees
}

export interface PresetInput {
  name: string;
  edid_id: string;
  icc_path?: string;
  brightness: number;
  contrast: number;
  rgb_gains: [number, number, number];
  gamma: number;
  vibrance: number;
  hue_deg: number;
}

export interface ApplyResult {
  icc_applied: boolean;
  gamma_applied: boolean;
  vibrance_applied: boolean;
  error?: string;
}

export interface EnforceEvent {
  edid_id: string;
  preset_id: string;
  applied: boolean;
  error?: string;
}