import { describe, it, expect } from "vitest";
import { validatePreset } from "./validation";

describe("vibrance validation", () => {
  it("rejects vibrance out of range", () => {
    expect(validatePreset({ vibrance: 101 }).vibrance).toContain("0 and 100");
    expect(validatePreset({ vibrance: -1 }).vibrance).toBeDefined();
    expect(validatePreset({ vibrance: 50 }).vibrance).toBeUndefined();
  });

  it("rejects hue out of range", () => {
    expect(validatePreset({ hue_deg: 360 }).hue_deg).toContain("0 and 359");
    expect(validatePreset({ hue_deg: 0 }).hue_deg).toBeUndefined();
  });
});

describe("validatePreset", () => {
  it("rejects gamma=5 (out of range)", () => {
    const errors = validatePreset({ gamma: 5 });
    expect(errors.gamma).toBeDefined();
    expect(errors.gamma).toContain("Gamma");
  });

  it("accepts gamma=2.2 (in range)", () => {
    const errors = validatePreset({ gamma: 2.2 });
    expect(errors.gamma).toBeUndefined();
  });

  it("rejects gamma boundary low (0.2)", () => {
    const errors = validatePreset({ gamma: 0.2 });
    expect(errors.gamma).toBeDefined();
  });

  it("rejects gamma boundary high (2.9)", () => {
    const errors = validatePreset({ gamma: 2.9 });
    expect(errors.gamma).toBeDefined();
  });

  it("accepts gamma boundary low (1.0)", () => {
    const errors = validatePreset({ gamma: 1.0 });
    expect(errors.gamma).toBeUndefined();
  });

  it("accepts gamma boundary high (2.8)", () => {
    const errors = validatePreset({ gamma: 2.8 });
    expect(errors.gamma).toBeUndefined();
  });

  it("rejects empty name", () => {
    const errors = validatePreset({ name: "" });
    expect(errors.name).toBeDefined();
  });

  it("accepts valid name", () => {
    const errors = validatePreset({ name: "My Preset" });
    expect(errors.name).toBeUndefined();
  });

  it("rejects brightness > 100", () => {
    const errors = validatePreset({ brightness: 101 });
    expect(errors.brightness).toBeDefined();
  });

  it("rejects brightness < 0", () => {
    const errors = validatePreset({ brightness: -0.1 });
    expect(errors.brightness).toBeDefined();
  });

  it("accepts brightness=50", () => {
    const errors = validatePreset({ brightness: 50 });
    expect(errors.brightness).toBeUndefined();
  });

  it("rejects brightness/contrast outside 0-100", () => {
    expect(validatePreset({ brightness: 101 }).brightness).toContain("0 and 100");
    expect(validatePreset({ contrast: -1 }).contrast).toBeDefined();
    expect(validatePreset({ brightness: 50 }).brightness).toBeUndefined();
  });

  it("rejects negative RGB gains", () => {
    const errors = validatePreset({ rgb_gains: [-1, 0, 0] });
    expect(errors.rgb_gains).toBeDefined();
  });

  it("accepts zero RGB gains", () => {
    const errors = validatePreset({ rgb_gains: [0, 0, 0] });
    expect(errors.rgb_gains).toBeUndefined();
  });

  it("rejects wrong-length rgb_gains array", () => {
    const errors = validatePreset({ rgb_gains: [1, 2] as unknown as [number, number, number] });
    expect(errors.rgb_gains).toBeDefined();
  });

it("returns empty errors when all fields are valid", () => {
    const errors = validatePreset({
      name: "Test",
      gamma: 2.2,
      brightness: 50,
      contrast: 50,
      rgb_gains: [1.0, 1.0, 1.0],
    });
    expect(Object.keys(errors)).toHaveLength(0);
  });
});

describe("color_tag validation", () => {
  it("accepts empty string (untagged)", () => {
    const errors = validatePreset({ color_tag: "" });
    expect(errors.color_tag).toBeUndefined();
  });

  it("accepts valid #rrggbb hex color", () => {
    const errors = validatePreset({ color_tag: "#06b6d4" });
    expect(errors.color_tag).toBeUndefined();
  });

  it("accepts uppercase hex", () => {
    const errors = validatePreset({ color_tag: "#EF4444" });
    expect(errors.color_tag).toBeUndefined();
  });

  it("rejects non-hex string", () => {
    const errors = validatePreset({ color_tag: "not-a-color" });
    expect(errors.color_tag).toBeDefined();
  });

  it("rejects 3-digit hex", () => {
    const errors = validatePreset({ color_tag: "#fff" });
    expect(errors.color_tag).toBeDefined();
  });

  it("rejects missing hash prefix", () => {
    const errors = validatePreset({ color_tag: "06b6d4" });
    expect(errors.color_tag).toBeDefined();
  });

  it("rejects invalid hex chars", () => {
    const errors = validatePreset({ color_tag: "#zz0000" });
    expect(errors.color_tag).toBeDefined();
  });

  it("undefined color_tag passes validation (untagged)", () => {
    const errors = validatePreset({});
    expect(errors.color_tag).toBeUndefined();
  });
});
