/// Pure validation logic — testable without Tauri runtime.
import type { PresetInput } from "./types";

export interface ValidationErrors {
  name?: string;
  gamma?: string;
  brightness?: string;
  contrast?: string;
  rgb_gains?: string;
  icc_path?: string;
  vibrance?: string;
  hue_deg?: string;
  nvcp?: string;
  color_tag?: string;
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
    if (typeof input.gamma !== "number" || input.gamma < 0.3 || input.gamma > 2.8) {
      errors.gamma = "Gamma must be between 0.3 and 2.8";
    }
  }

  if (input.brightness !== undefined) {
    if (typeof input.brightness !== "number" || input.brightness < 0 || input.brightness > 100) {
      errors.brightness = "Brightness must be between 0 and 100";
    }
  }

  if (input.contrast !== undefined) {
    if (typeof input.contrast !== "number" || input.contrast < 0 || input.contrast > 100) {
      errors.contrast = "Contrast must be between 0 and 100";
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

  if (input.vibrance !== undefined) {
    if (typeof input.vibrance !== "number" || input.vibrance < 0 || input.vibrance > 100) {
      errors.vibrance = "Vibrance must be between 0 and 100";
    }
  }

  if (input.hue_deg !== undefined) {
    if (typeof input.hue_deg !== "number" || input.hue_deg < 0 || input.hue_deg > 359) {
      errors.hue_deg = "Hue must be between 0 and 359";
    }
  }

  if (input.color_tag !== undefined) {
    if (input.color_tag !== "" && !/^#[0-9a-fA-F]{6}$/.test(input.color_tag)) {
      errors.color_tag = "Color tag must be a hex color like #rrggbb or empty";
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
    vibrance: input.vibrance,
    hue_deg: input.hue_deg,
    color_tag: input.color_tag,
  });

  Object.assign(errors, all);

  return Object.keys(errors).length === 0;
}