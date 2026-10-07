import { describe, it, expect } from "vitest";
import { splitCategory, formatGamma, formatInt, formatStat, isNeutral } from "./presetCard";

describe("splitCategory", () => {
  it("splits on first \" - \" separator", () => {
    const r = splitCategory("Movie - Warm");
    expect(r.category).toBe("Movie");
    expect(r.title).toBe("Warm");
  });

  it("returns whole name as title when no separator", () => {
    const r = splitCategory("Warm");
    expect(r.category).toBeUndefined();
    expect(r.title).toBe("Warm");
  });

  it("handles multiple separators — splits only the first", () => {
    const r = splitCategory("A - B - C");
    expect(r.category).toBe("A");
    expect(r.title).toBe("B - C");
  });

  it("handles empty string", () => {
    const r = splitCategory("");
    expect(r.category).toBeUndefined();
    expect(r.title).toBe("");
  });

  it("handles separator at start", () => {
    const r = splitCategory(" - Title");
    expect(r.category).toBe("");
    expect(r.title).toBe("Title");
  });

  it("handles separator at end", () => {
    const r = splitCategory("Category - ");
    expect(r.category).toBe("Category");
    expect(r.title).toBe("");
  });
});

describe("formatGamma", () => {
  it("formats 1.0 as '1.00'", () => {
    expect(formatGamma(1.0)).toBe("1.00");
  });

  it("formats 2.2 as '2.20'", () => {
    expect(formatGamma(2.2)).toBe("2.20");
  });

  it("formats 0.3 as '0.30'", () => {
    expect(formatGamma(0.3)).toBe("0.30");
  });

  it("formats 1.756 as '1.76'", () => {
    expect(formatGamma(1.756)).toBe("1.76");
  });
});

describe("formatInt", () => {
  it("formats 50 as '50'", () => {
    expect(formatInt(50)).toBe("50");
  });

  it("formats 0 as '0'", () => {
    expect(formatInt(0)).toBe("0");
  });

  it("formats 100 as '100'", () => {
    expect(formatInt(100)).toBe("100");
  });

  it("formats 75.3 as '75'", () => {
    expect(formatInt(75.3)).toBe("75");
  });
});

describe("formatStat", () => {
  it("formats gamma with 2 decimals", () => {
    expect(formatStat("gamma", 1.0)).toBe("1.00");
    expect(formatStat("gamma", 2.2)).toBe("2.20");
  });

  it("formats brightness as integer", () => {
    expect(formatStat("brightness", 50)).toBe("50");
  });

  it("formats contrast as integer", () => {
    expect(formatStat("contrast", 75)).toBe("75");
  });

  it("formats vibrance as integer", () => {
    expect(formatStat("vibrance", 100)).toBe("100");
  });
});

describe("isNeutral", () => {
  it("gamma 1.0 is neutral", () => {
    expect(isNeutral("gamma", 1.0)).toBe(true);
  });

  it("gamma 1.0 - 1e-12 is neutral (within epsilon)", () => {
    expect(isNeutral("gamma", 1.0 - 1e-12)).toBe(true);
  });

  it("gamma 1.0 + 1e-12 is neutral (within epsilon)", () => {
    expect(isNeutral("gamma", 1.0 + 1e-12)).toBe(true);
  });

  it("gamma 1.1 is not neutral", () => {
    expect(isNeutral("gamma", 1.1)).toBe(false);
  });

  it("gamma > EPS threshold is not neutral", () => {
    expect(isNeutral("gamma", 1.0 + 1e-8)).toBe(false);
  });

  it("brightness 50 is neutral", () => {
    expect(isNeutral("brightness", 50)).toBe(true);
  });

  it("brightness 51 is not neutral", () => {
    expect(isNeutral("brightness", 51)).toBe(false);
  });

  it("contrast 50 is neutral", () => {
    expect(isNeutral("contrast", 50)).toBe(true);
  });

  it("contrast 0 is not neutral", () => {
    expect(isNeutral("contrast", 0)).toBe(false);
  });

  it("vibrance 50 is neutral", () => {
    expect(isNeutral("vibrance", 50)).toBe(true);
  });

  it("vibrance 100 is not neutral", () => {
    expect(isNeutral("vibrance", 100)).toBe(false);
  });
});