/**
 * Pure preset <-> editor-form mapping.
 *
 * Kept out of the component so the saturation field (and the neutral reset)
 * can be tested without a DOM testing stack.
 */

import type { CapturedState, Preset, PresetInput } from "./types";
import { NEUTRAL } from "./presetCard";

/** Fresh editor state for a new preset. */
export const DEFAULT_PRESET_INPUT: PresetInput = {
  name: "",
  brightness: 50,
  contrast: 50,
  rgb_gains: [1.0, 1.0, 1.0],
  gamma: 1.0,
  vibrance: 50,
  saturation: 50,
  hue_deg: 0,
  color_tag: "",
  tag_shape: "pill",
  attached_programs: [],
  shortcut: "",
  hardware_brightness: undefined,
};

export function withSelectedIcc(form: PresetInput, path: string): PresetInput {
  return { ...form, icc_path: path, clear_icc: false };
}

/** Explicitly detach any existing or newly picked ICC profile on save. */
export function withClearedIcc(form: PresetInput): PresetInput {
  const { icc_path: _removed, ...rest } = form;
  return { ...rest, clear_icc: true };
}

/** Populate the editor form from a stored preset (edit mode). */
export function presetToFormValues(preset: Preset): PresetInput {
  return {
    name: preset.name,
    brightness: preset.brightness,
    contrast: preset.contrast,
    rgb_gains: [...preset.rgb_gains] as [number, number, number],
    gamma: preset.gamma,
    vibrance: preset.vibrance,
    saturation: preset.saturation,
    hue_deg: preset.hue_deg,
    color_tag: preset.color_tag ?? "",
    tag_shape: preset.tag_shape ?? "pill",
    attached_programs: preset.attached_programs ?? [],
    shortcut: preset.shortcut ?? "",
    hardware_brightness: preset.hardware_brightness ?? undefined,
  };
}

/**
 * Restore every colour slider to its neutral default. Name, ICC selection,
 * tag colour, and capture source are left untouched.
 */
export function resetSliderValues(form: PresetInput): PresetInput {
  return {
    ...form,
    brightness: NEUTRAL.brightness,
    contrast: NEUTRAL.contrast,
    rgb_gains: [1.0, 1.0, 1.0],
    gamma: NEUTRAL.gamma,
    vibrance: NEUTRAL.vibrance,
    saturation: NEUTRAL.saturation,
    hue_deg: 0,
    hardware_brightness: undefined,
  };
}

/** Clamp and round a numeric value to the step precision of a slider field. */
export function clampSliderValue(raw: number, min: number, max: number, step: number): number {
  const clamped = Math.min(max, Math.max(min, raw));
  const decimals = step < 1 ? Math.ceil(-Math.log10(step)) : 0;
  const factor = Math.pow(10, decimals);
  return Math.round(clamped * factor) / factor;
}

/**
 * Apply live captured NVCP state into the form.
 * RGB gains reset to [1, 1, 1] because NVCP's floating-point ramp has no
 * separate per-channel gain representation.
 */
export function applyCapturedState(form: PresetInput, captured: CapturedState): PresetInput {
  return {
    ...form,
    brightness: clampSliderValue(captured.brightness, 0, 100, 1),
    contrast: clampSliderValue(captured.contrast, 0, 100, 1),
    gamma: clampSliderValue(captured.gamma, 0.3, 2.8, 0.05),
    vibrance: clampSliderValue(captured.vibrance, 0, 100, 1),
    hue_deg: clampSliderValue(captured.hue_deg, 0, 359, 1),
    rgb_gains: [1.0, 1.0, 1.0],
  };
}
