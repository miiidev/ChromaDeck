import { describe, it, expect } from "vitest";
import { resetAllSummary } from "./resetAllSummary";
import type { ApplyResult } from "./types";

const ok = { error: null } as unknown as ApplyResult;

describe("resetAllSummary", () => {
  it("all ok, one pin", () => {
    expect(resetAllSummary([ok], 1)).toBe("Reset all: 1 OK · 0 failed · 1 pins removed");
  });
  it("failure counted, pins plural", () => {
    expect(
      resetAllSummary([ok, { error: "monitor is offline" } as unknown as ApplyResult], 3)
    ).toBe("Reset all: 1 OK · 1 failed · 3 pins removed");
  });
  it("nothing reset", () => {
    expect(resetAllSummary([], 0)).toBe("Reset all: 0 OK · 0 failed · 0 pins removed");
  });
});
