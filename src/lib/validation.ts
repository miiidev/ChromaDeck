/// Pure validation logic — testable without Tauri runtime.
import type { PresetInput } from "./types";

export interface ValidationErrors {
  name?: string;
  gamma?: string;
  brightness?: string;
  contrast?: string;
  rgb_gains?: string;
  icc_path?: string;
}

/** Validate a partial PresetInput and return per-field error strings. */
export function validatePreset(input: Partial<PresetInput>): ValidationErrors {
  const errors: ValidationErrors = {};

  if (input.name !== undefined) {
    if (input.name.trim().length === 0) {
      errors.name = "Name is required";
    }
  }

  if (input.gamma !== undefined) {
    if (typeof input.gamma !== "number" || input.gamma < 1.0 || input.gamma > 3.0) {
      errors.gamma = "Gamma must be between 1.0 and 3.0";
    }
  }

  if (input.brightness !== undefined) {
    if (typeof input.brightness !== "number" || input.brightness < 0 || input.brightness > 1) {
      errors.brightness = "Brightness must be between 0 and 1";
    }
  }

  if (input.contrast !== undefined) {
    if (typeof input.contrast !== "number" || input.contrast < 0 || input.contrast > 1) {
      errors.contrast = "Contrast must be between 0 and 1";
    }
  }

  if (input.rgb_gains !== undefined) {
    if (
      !Array.isArray(input.rgb_gains) ||
      input.rgb_gains.length !== 3 ||
      input.rgb_gains.some((v) => typeof v !== "number" || v < 0)
    ) {
      errors.rgb_gains = "RGB gains must be non-negative values";
    }
  }

  return errors;
}

/** Full form validation before save. Returns true if valid, populates errors. */
export function validatePresetForm(
  input: PresetInput,
  errors: ValidationErrors,
): boolean {
  // Check every field
  const all = validatePreset({
    name: input.name,
    gamma: input.gamma,
    brightness: input.brightness,
    contrast: input.contrast,
    rgb_gains: input.rgb_gains,
  });

  Object.assign(errors, all);

  if (!input.edid_id) {
    errors.name = "Monitor is required";
  }

  return Object.keys(errors).length === 0;
}