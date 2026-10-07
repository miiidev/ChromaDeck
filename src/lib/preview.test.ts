import { describe, it, expect } from "vitest";
import { applyPreviewPixel, type PreviewParams } from "./preview";

function identityParams(overrides?: Partial<PreviewParams>): PreviewParams {
  return {
    brightness: 50,
    contrast: 50,
    gamma: 1.0,
    rgb_gains: [1, 1, 1],
    vibrance: 50,
    hue_deg: 0,
    ...overrides,
  };
}

describe("applyPreviewPixel", () => {
  // ── Identity / neutrality ──────────────────────────────────────────────

  it("identity params are a no-op for black", () => {
    const result = applyPreviewPixel({ r: 0, g: 0, b: 0 }, identityParams());
    expect(result.r).toBe(0);
    expect(result.g).toBe(0);
    expect(result.b).toBe(0);
  });

  it("identity params are a no-op for mid-gray", () => {
    const result = applyPreviewPixel({ r: 0.5, g: 0.5, b: 0.5 }, identityParams());
    expect(result.r).toBeCloseTo(0.5, 10);
    expect(result.g).toBeCloseTo(0.5, 10);
    expect(result.b).toBeCloseTo(0.5, 10);
  });

  it("identity params are a no-op for white", () => {
    const result = applyPreviewPixel({ r: 1, g: 1, b: 1 }, identityParams());
    expect(result.r).toBe(1);
    expect(result.g).toBe(1);
    expect(result.b).toBe(1);
  });

  it("identity params are a no-op for a colour pixel", () => {
    const result = applyPreviewPixel({ r: 0.3, g: 0.7, b: 0.1 }, identityParams());
    expect(result.r).toBeCloseTo(0.3, 10);
    expect(result.g).toBeCloseTo(0.7, 10);
    expect(result.b).toBeCloseTo(0.1, 10);
  });

  // ── Gamma neutrality (1.0) ─────────────────────────────────────────────

  it("gamma=1.0 is a no-op for mid-gray", () => {
    const result = applyPreviewPixel({ r: 0.5, g: 0.5, b: 0.5 }, identityParams({ gamma: 1.0 }));
    expect(result.r).toBeCloseTo(0.5, 10);
  });

  // ── Gains neutrality ───────────────────────────────────────────────────

  it("gains=1 are a no-op for mid-gray", () => {
    const result = applyPreviewPixel({ r: 0.5, g: 0.5, b: 0.5 }, identityParams({ rgb_gains: [1, 1, 1] }));
    expect(result.r).toBeCloseTo(0.5, 10);
  });

  // ── Hue=0 neutrality ──────────────────────────────────────────────────

  it("hue_deg=0 is a no-op for a colour pixel", () => {
    const result = applyPreviewPixel({ r: 0.8, g: 0.2, b: 0.6 }, identityParams({ hue_deg: 0 }));
    expect(result.r).toBeCloseTo(0.8, 10);
    expect(result.g).toBeCloseTo(0.2, 10);
    expect(result.b).toBeCloseTo(0.6, 10);
  });

  // ── Vibrance=50 neutrality ─────────────────────────────────────────────

  it("vibrance=50 is a no-op for a colour pixel", () => {
    const result = applyPreviewPixel({ r: 0.3, g: 0.7, b: 0.1 }, identityParams({ vibrance: 50 }));
    expect(result.r).toBeCloseTo(0.3, 10);
    expect(result.g).toBeCloseTo(0.7, 10);
    expect(result.b).toBeCloseTo(0.1, 10);
  });

  // ── Output clamped 0..1 on extremes ────────────────────────────────────

  it("clamps negative output from gamma=2.8 on dark pixel", () => {
    // Gamma > 1 on a very dark pixel can push values down, but 0^2.8 = 0
    const result = applyPreviewPixel({ r: 0, g: 0, b: 0 }, identityParams({ gamma: 2.8 }));
    expect(result.r).toBe(0);
    expect(result.g).toBe(0);
    expect(result.b).toBe(0);
  });

  it("clamps overflow from gains=2 on mid-gray", () => {
    const result = applyPreviewPixel({ r: 0.5, g: 0.5, b: 0.5 }, identityParams({ rgb_gains: [2, 2, 2] }));
    // 0.5^1.0 = 0.5; 0.5 * 2 = 1.0
    expect(result.r).toBe(1);
    expect(result.g).toBe(1);
    expect(result.b).toBe(1);
  });

  it("clamps overflow from gains=2 on white", () => {
    const result = applyPreviewPixel({ r: 1, g: 1, b: 1 }, identityParams({ rgb_gains: [2, 2, 2] }));
    expect(result.r).toBe(1);
    expect(result.g).toBe(1);
    expect(result.b).toBe(1);
  });

  it("clamps negative brightness at 0", () => {
    const result = applyPreviewPixel({ r: 0, g: 0, b: 0 }, identityParams({ brightness: 0 }));
    // offset = (0-50)/100 = -0.5
    // 0 + (-0.5) = -0.5 → clamped to 0
    expect(result.r).toBe(0);
    expect(result.g).toBe(0);
    expect(result.b).toBe(0);
  });

  it("clamps overflow from brightness at 100", () => {
    const result = applyPreviewPixel({ r: 0.8, g: 0.8, b: 0.8 }, identityParams({ brightness: 100 }));
    // offset = (100-50)/100 = 0.5
    // 0.8 + 0.5 = 1.3 → clamped to 1
    expect(result.r).toBe(1);
    expect(result.g).toBe(1);
    expect(result.b).toBe(1);
  });

  it("clamps extremes from gamma=0.3 on bright pixel", () => {
    // 1.0^0.3 = 1.0 (no clamp needed, but test extreme gamma doesn't break)
    const result = applyPreviewPixel({ r: 1, g: 1, b: 1 }, identityParams({ gamma: 0.3 }));
    expect(result.r).toBe(1);
  });

  it("brightness=0 contrast=0 yields mid-centered value", () => {
    // brightness offset happens BEFORE contrast multiplication:
    // 0.3 + (0-50)/100 = -0.2, then (-0.2-0.5)*0 + 0.5 = 0.5
    const result = applyPreviewPixel({ r: 0.3, g: 0.3, b: 0.3 }, identityParams({ brightness: 0, contrast: 0 }));
    expect(result.r).toBeCloseTo(0.5, 10);
  });

  it("brightness=100 contrast=100 on mid-gray clamps at 1", () => {
    // 0.4 + 0.5 = 0.9 → (0.9-0.5)*2 + 0.5 = 1.3 → clamped to 1
    const result = applyPreviewPixel({ r: 0.4, g: 0.4, b: 0.4 }, identityParams({ brightness: 100, contrast: 100 }));
    expect(result.r).toBe(1);
    expect(result.g).toBe(1);
    expect(result.b).toBe(1);
  });

  it("gain=0 yields black", () => {
    const result = applyPreviewPixel({ r: 0.9, g: 0.9, b: 0.9 }, identityParams({ rgb_gains: [0, 0, 0] }));
    expect(result.r).toBe(0);
    expect(result.g).toBe(0);
    expect(result.b).toBe(0);
  });

  // ── Gamma > 1 darkens mid-gray ─────────────────────────────────────────

  it("gamma>1 darkens mid-gray (0.5^2.2 < 0.5)", () => {
    const result = applyPreviewPixel({ r: 0.5, g: 0.5, b: 0.5 }, identityParams({ gamma: 2.2 }));
    // 0.5^2.2 ≈ 0.2176
    expect(result.r).toBeLessThan(0.5);
    expect(result.r).toBeCloseTo(0.5 ** 2.2, 5);
  });

  it("gamma>1 preserves black and white", () => {
    const black = applyPreviewPixel({ r: 0, g: 0, b: 0 }, identityParams({ gamma: 2.2 }));
    expect(black.r).toBe(0);
    const white = applyPreviewPixel({ r: 1, g: 1, b: 1 }, identityParams({ gamma: 2.2 }));
    expect(white.r).toBe(1);
  });

  // ── Hue rotation ──────────────────────────────────────────────────────

  it("hue rotation changes pixel colour", () => {
    const result = applyPreviewPixel({ r: 1, g: 0, b: 0 }, identityParams({ hue_deg: 120 }));
    // Red (h=0) rotated 120° → Green (h=1/3)
    expect(result.r).toBeLessThan(0.01);
    expect(result.g).toBeGreaterThan(0.9);
  });

  it("hue rotation wraps at 360°", () => {
    const h90 = applyPreviewPixel({ r: 1, g: 0, b: 0 }, identityParams({ hue_deg: 90 }));
    const h450 = applyPreviewPixel({ r: 1, g: 0, b: 0 }, identityParams({ hue_deg: 450 }));
    expect(h90.r).toBeCloseTo(h450.r, 5);
    expect(h90.g).toBeCloseTo(h450.g, 5);
    expect(h90.b).toBeCloseTo(h450.b, 5);
  });

  // ── Vibrance at extreme values ─────────────────────────────────────────

  it("vibrance=0 desaturates a colour pixel", () => {
    const result = applyPreviewPixel({ r: 0.9, g: 0.1, b: 0.1 }, identityParams({ vibrance: 0 }));
    // Should be closer to gray
    const chroma = Math.max(result.r, result.g, result.b) - Math.min(result.r, result.g, result.b);
    expect(chroma).toBeLessThan(0.8); // desaturated
  });

  it("vibrance=100 saturates a colour pixel", () => {
    const result = applyPreviewPixel({ r: 0.4, g: 0.35, b: 0.3 }, identityParams({ vibrance: 100 }));
    // Should push colours away from gray
    const chroma = Math.max(result.r, result.g, result.b) - Math.min(result.r, result.g, result.b);
    expect(chroma).toBeGreaterThan(0.1);
  });

  // ── Contrast extremes ──────────────────────────────────────────────────

  it("contrast=0 flattens to mid-gray", () => {
    const result = applyPreviewPixel({ r: 0.8, g: 0.2, b: 0.5 }, identityParams({ contrast: 0 }));
    // (0.8-0.5)*0 + 0.5 = 0.5
    expect(result.r).toBeCloseTo(0.5, 10);
    expect(result.g).toBeCloseTo(0.5, 10);
    expect(result.b).toBeCloseTo(0.5, 10);
  });

  it("contrast=100 maximises difference from 0.5", () => {
    const result = applyPreviewPixel({ r: 0.8, g: 0.4, b: 0.6 }, identityParams({ contrast: 100 }));
    // (0.8-0.5)*2 + 0.5 = 1.1 → clamped to 1
    expect(result.r).toBe(1);
    // (0.4-0.5)*2 + 0.5 = 0.3
    expect(result.g).toBeCloseTo(0.3, 10);
  });
});