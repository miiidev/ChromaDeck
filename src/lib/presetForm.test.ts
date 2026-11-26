import { describe, it, expect } from "vitest";
import {
  DEFAULT_PRESET_INPUT,
  applyCapturedState,
  clampSliderValue,
  presetToFormValues,
  resetSliderValues,
  withClearedIcc,
  withSelectedIcc,
} from "./presetForm";
import type { Preset } from "./types";

const PRESET: Preset = {
  id: "p1",
  name: "Movie - Warm",
  icc_hash: "abc",
  icc_filename: "warm.icc",
  brightness: 42,
  contrast: 58,
  rgb_gains: [1.1, 0.95, 1.0],
  gamma: 2.2,
  vibrance: 61,
  saturation: 73,
  hue_deg: 12,
  color_tag: "#ff5533",
  tag_shape: "square",
};

describe("ICC selection", () => {
  it("includes the selected profile in new and edited preset payloads", () => {
    for (const form of [DEFAULT_PRESET_INPUT, presetToFormValues(PRESET)]) {
      const selected = withSelectedIcc(form, "C:\\profiles\\new.icc");
      expect(selected.icc_path).toBe("C:\\profiles\\new.icc");
      expect(selected.name).toBe(form.name);
      expect(form.icc_path).toBeUndefined();
    }
  });
});

describe("preset editor saturation round-trip", () => {
  it("defaults a new preset to neutral saturation", () => {
    expect(DEFAULT_PRESET_INPUT.saturation).toBe(50);
  });

  it("carries a stored saturation value into the form unchanged", () => {
    expect(presetToFormValues(PRESET).saturation).toBe(73);
  });

  it("round-trips every slider field from preset to form", () => {
    const form = presetToFormValues(PRESET);
    expect(form).toEqual({
      name: PRESET.name,
      brightness: PRESET.brightness,
      contrast: PRESET.contrast,
      rgb_gains: PRESET.rgb_gains,
      gamma: PRESET.gamma,
      vibrance: PRESET.vibrance,
      saturation: PRESET.saturation,
      hue_deg: PRESET.hue_deg,
      color_tag: PRESET.color_tag,
      tag_shape: PRESET.tag_shape,
      attached_programs: [],
      shortcut: "",
    });
  });

  it("carries the tag shape into the form and defaults to pill", () => {
    expect(presetToFormValues(PRESET).tag_shape).toBe("square");
    expect(DEFAULT_PRESET_INPUT.tag_shape).toBe("pill");
    expect(presetToFormValues({ ...PRESET, tag_shape: undefined }).tag_shape).toBe("pill");
  });

  it("restores saturation to neutral on reset without touching name or tag", () => {
    const form = { ...presetToFormValues(PRESET), saturation: 8 };
    const reset = resetSliderValues(form);
    expect(reset.saturation).toBe(50);
    expect(reset.name).toBe("Movie - Warm");
    expect(reset.color_tag).toBe("#ff5533");
  });
});

describe("ICC clearing, capture & slider clamping", () => {
  it("marks clear_icc when detaching and unmarks when selecting a new ICC", () => {
    const base = presetToFormValues(PRESET);
    const cleared = withClearedIcc(base);
    expect(cleared.clear_icc).toBe(true);
    expect(cleared.icc_path).toBeUndefined();

    const reselected = withSelectedIcc(cleared, "C:\\profiles\\next.icc");
    expect(reselected.clear_icc).toBe(false);
    expect(reselected.icc_path).toBe("C:\\profiles\\next.icc");
  });

  it("applies captured NVCP values and resets rgb_gains to neutral", () => {
    const form = presetToFormValues(PRESET);
    const next = applyCapturedState(form, {
      brightness: 62.4,
      contrast: 48.7,
      gamma: 1.34,
      vibrance: 75.2,
      hue_deg: 18.6,
    });
    expect(next.brightness).toBe(62);
    expect(next.contrast).toBe(49);
    expect(next.gamma).toBe(1.34);
    expect(next.vibrance).toBe(75);
    expect(next.hue_deg).toBe(19);
    expect(next.rgb_gains).toEqual([1, 1, 1]);
    // Untouched metadata
    expect(next.name).toBe(form.name);
    expect(next.saturation).toBe(form.saturation);
  });

  it("clamps slider values into range at step precision", () => {
    expect(clampSliderValue(3.5, 0.3, 2.8, 0.05)).toBe(2.8);
    expect(clampSliderValue(-5, 0, 100, 1)).toBe(0);
    expect(clampSliderValue(1.126, 0.5, 1.5, 0.01)).toBe(1.13);
  });
});
