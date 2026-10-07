import { describe, expect, it } from "vitest";
import { deviatingChips, type ChipParams } from "./presetChips";

const NEUTRAL: ChipParams = {
  gamma: 1.0,
  brightness: 50,
  contrast: 50,
  rgb_gains: [1.0, 1.0, 1.0],
  vibrance: 50,
  hue_deg: 0,
};

function withParams(partial: Partial<ChipParams>): ChipParams {
  return { ...NEUTRAL, ...partial };
}

describe("deviatingChips", () => {
  it("returns [] for an all-neutral preset", () => {
    expect(deviatingChips(NEUTRAL)).toEqual([]);
  });

  it("isolates a single gamma deviation", () => {
    const chips = deviatingChips(withParams({ gamma: 2.2 }));
    expect(chips).toHaveLength(1);
    expect(chips[0].key).toBe("gamma");
    expect(chips[0].label).toBe("γ2.20");
  });

  it("tracks RGB channels independently", () => {
    const chips = deviatingChips(withParams({ rgb_gains: [1.2, 1.0, 1.0] }));
    expect(chips).toHaveLength(1);
    expect(chips[0]).toMatchObject({ key: "gain-R", label: "R1.20" });
  });

  it("emits all three channels when all deviate", () => {
    const chips = deviatingChips(withParams({ rgb_gains: [1.1, 0.9, 1.2] }));
    expect(chips.map((c) => c.key)).toEqual(["gain-R", "gain-G", "gain-B"]);
  });

  it("covers brightness, contrast, vibrance, hue", () => {
    const chips = deviatingChips(
      withParams({ brightness: 65, contrast: 40, vibrance: 80, hue_deg: 15 }),
    );
    expect(chips.map((c) => c.key)).toEqual([
      "brightness",
      "contrast",
      "vibrance",
      "hue",
    ]);
    expect(chips.map((c) => c.label)).toEqual(["B65", "C40", "V80", "H15°"]);
  });

  it("ignores float noise at neutral", () => {
    const chips = deviatingChips(
      withParams({ gamma: 1 + 1e-12, rgb_gains: [1 + 1e-12, 1, 1] }),
    );
    expect(chips).toEqual([]);
  });

  it("keeps chip order stable: gamma, brightness, contrast, gains, vibrance, hue", () => {
    const chips = deviatingChips(
      withParams({
        hue_deg: 10,
        vibrance: 60,
        rgb_gains: [1, 1, 0.9],
        contrast: 55,
        brightness: 45,
        gamma: 1.8,
      }),
    );
    expect(chips.map((c) => c.key)).toEqual([
      "gamma",
      "brightness",
      "contrast",
      "gain-B",
      "vibrance",
      "hue",
    ]);
  });
});
