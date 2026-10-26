import { describe, it, expect } from "vitest";
import {
  ALL_CATEGORIES,
  NO_CATEGORY,
  filterPresets,
  hasUncategorisedPresets,
  presetCategories,
} from "./presetSearch";
import type { Preset } from "./types";

function preset(id: string, name: string): Preset {
  return {
    id,
    name,
    icc_hash: "",
    icc_filename: "",
    brightness: 50,
    contrast: 50,
    rgb_gains: [1, 1, 1],
    gamma: 1,
    vibrance: 50,
    saturation: 50,
    hue_deg: 0,
  };
}

const PRESETS: Preset[] = [
  preset("a", "Movie - Warm Cinema"),
  preset("b", "Photo - sRGB Standard"),
  preset("c", "Night - Low Blue"),
  preset("d", "Photo - Vibrant"),
  preset("e", "Uncategorised"),
];

const names = (list: Preset[]) => list.map((p) => p.name);

describe("filterPresets — text", () => {
  it("returns the list unchanged for an empty query", () => {
    expect(names(filterPresets(PRESETS, ""))).toEqual(names(PRESETS));
  });

  it("treats a whitespace-only query as empty", () => {
    expect(names(filterPresets(PRESETS, "   "))).toEqual(names(PRESETS));
  });

  it("matches a title fragment case-insensitively", () => {
    expect(names(filterPresets(PRESETS, "warm"))).toEqual(["Movie - Warm Cinema"]);
    expect(names(filterPresets(PRESETS, "WARM"))).toEqual(["Movie - Warm Cinema"]);
  });

  it("matches the category prefix", () => {
    expect(names(filterPresets(PRESETS, "photo"))).toEqual([
      "Photo - sRGB Standard",
      "Photo - Vibrant",
    ]);
  });

  it("requires every whitespace-separated token, in any order", () => {
    expect(names(filterPresets(PRESETS, "photo vibrant"))).toEqual(["Photo - Vibrant"]);
    expect(names(filterPresets(PRESETS, "vibrant photo"))).toEqual(["Photo - Vibrant"]);
  });

  it("returns nothing when a token does not match", () => {
    expect(filterPresets(PRESETS, "photo night")).toEqual([]);
    expect(filterPresets(PRESETS, "zzz")).toEqual([]);
  });

  it("does not match on non-name fields", () => {
    const tagged = [{ ...preset("f", "Movie - Warm"), color_tag: "#ff0000" }];
    expect(filterPresets(tagged, "#ff0000")).toEqual([]);
  });
});

describe("filterPresets — category", () => {
  it("lists distinct categories sorted", () => {
    expect(presetCategories(PRESETS)).toEqual(["Movie", "Night", "Photo"]);
  });

  it("detects uncategorised presets", () => {
    expect(hasUncategorisedPresets(PRESETS)).toBe(true);
    expect(hasUncategorisedPresets([preset("a", "Movie - Warm")])).toBe(false);
  });

  it("filters by an exact category", () => {
    expect(names(filterPresets(PRESETS, "", "Photo"))).toEqual([
      "Photo - sRGB Standard",
      "Photo - Vibrant",
    ]);
  });

  it("filters uncategorised presets", () => {
    expect(names(filterPresets(PRESETS, "", NO_CATEGORY))).toEqual(["Uncategorised"]);
  });

  it("ALL_CATEGORIES keeps everything", () => {
    expect(names(filterPresets(PRESETS, "", ALL_CATEGORIES))).toEqual(names(PRESETS));
  });

  it("combines category and text filters", () => {
    expect(names(filterPresets(PRESETS, "vibrant", "Photo"))).toEqual(["Photo - Vibrant"]);
    expect(filterPresets(PRESETS, "warm", "Photo")).toEqual([]);
  });
});
