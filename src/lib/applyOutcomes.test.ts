import { describe, it, expect } from "vitest";
import { adjustmentLabel, summarizeApply } from "./applyOutcomes";
import type { ApplyResult } from "./types";

const LEGACY_APPLIED: ApplyResult = {
  icc_applied: true,
  gamma_applied: true,
  vibrance_applied: false,
  error: undefined,
};

const DETAILED_APPLIED: ApplyResult = {
  icc_applied: true,
  gamma_applied: true,
  vibrance_applied: false,
  error: undefined,
  adjustments: [
    { adjustment: "icc", status: "applied", verified: false },
    { adjustment: "gamma", status: "applied", verified: false },
    { adjustment: "saturation", status: "skipped", verified: false, reason: "NVAPI has no desktop saturation control" },
  ],
};

describe("adjustmentLabel", () => {
  it("uses the established labels", () => {
    expect(adjustmentLabel("rgb_gains")).toBe("RGB gains");
    expect(adjustmentLabel("icc")).toBe("ICC");
  });

  it("degrades unknown keys to readable text", () => {
    expect(adjustmentLabel("custom_thing")).toBe("custom thing");
  });
});

describe("summarizeApply", () => {
  it("preserves the legacy boolean copy when no per-adjustment detail exists", () => {
    const s = summarizeApply(LEGACY_APPLIED);
    expect(s.rows).toEqual([]);
    expect(s.headline).toBe("ICC applied + gamma applied");
    expect(s.hasFailures).toBe(false);
    expect(s.verificationNote).toBeNull();
  });

  it("reports skipped adjustments instead of hiding them", () => {
    const s = summarizeApply(DETAILED_APPLIED);
    expect(s.rows).toHaveLength(3);
    expect(s.skippedCount).toBe(1);
    expect(s.headline).toBe("Applied with 1 skipped");
    expect(s.rows[2]).toMatchObject({
      adjustment: "saturation",
      label: "Saturation",
      status: "skipped",
      reason: "NVAPI has no desktop saturation control",
    });
  });

  it("flags failed adjustments and keeps their reason", () => {
    const s = summarizeApply({
      icc_applied: false,
      gamma_applied: true,
      vibrance_applied: false,
      error: "vibrance: DVC write failed",
      adjustments: [
        { adjustment: "gamma", status: "applied", verified: false },
        { adjustment: "vibrance", status: "failed", verified: false, reason: "vibrance: DVC write failed" },
      ],
    });
    expect(s.hasFailures).toBe(true);
    expect(s.failedCount).toBe(1);
    expect(s.headline).toBe("Applied with 1 failed");
    expect(s.rows[1].status).toBe("failed");
    expect(s.rows[1].reason).toContain("DVC write failed");
  });

  it("reports a total failure without claiming success", () => {
    const s = summarizeApply({
      icc_applied: false,
      gamma_applied: false,
      vibrance_applied: false,
      error: "monitor is offline",
      adjustments: [
        { adjustment: "gamma", status: "failed", verified: false, reason: "monitor is offline" },
      ],
    });
    expect(s.headline).toBe("Apply failed");
    expect(s.appliedCount).toBe(0);
    expect(s.hasFailures).toBe(true);
  });

  it("marks applied writes unverified with an honest footnote", () => {
    const s = summarizeApply(DETAILED_APPLIED);
    expect(s.unverifiedApplied).toBe(true);
    expect(s.verificationNote).toContain("not read-back verified");

    const verified = summarizeApply({
      icc_applied: false,
      gamma_applied: true,
      vibrance_applied: false,
      adjustments: [{ adjustment: "gamma", status: "applied", verified: true }],
    });
    expect(verified.unverifiedApplied).toBe(false);
    expect(verified.verificationNote).toBeNull();
  });

  it("lists applied adjustments in backend order when nothing was skipped", () => {
    const s = summarizeApply({
      icc_applied: false,
      gamma_applied: true,
      vibrance_applied: true,
      adjustments: [
        { adjustment: "gamma", status: "applied", verified: false },
        { adjustment: "vibrance", status: "applied", verified: false },
      ],
    });
    expect(s.headline).toBe("Applied - Gamma, Vibrance");
  });
});
