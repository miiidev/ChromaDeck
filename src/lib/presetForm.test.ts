import { describe, it, expect } from "vitest";
import { DEFAULT_PRESET_INPUT, presetToFormValues, resetSliderValues } from "./presetForm";
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
