import { describe, it, expect, beforeEach } from "vitest";
import { consumeStagger, resetStagger, staggerCss, prefersReducedMotion, REDUCED_MOTION } from "./motion";

describe("motion utilities (server-safe)", () => {
  beforeEach(() => {
    resetStagger();
  });

  it("consumeStagger returns true when no window (test env)", () => {
    // In node environment typeof window === "undefined",
    // so consumeStagger returns true immediately.
    // It ALSO returns true on every subsequent call (no window to set flag on).
    expect(consumeStagger()).toBe(true);
  });

  it("consumeStagger always true in server env (no window)", () => {
    expect(consumeStagger()).toBe(true);
    expect(consumeStagger()).toBe(true); // no window to persist flag
  });

  it("resetStagger is safe to call in server env", () => {
    resetStagger(); // should not throw
    expect(consumeStagger()).toBe(true);
  });

  it("REDUCED_MOTION is false when no window", () => {
    // In node environment window is undefined
    expect(REDUCED_MOTION).toBe(false);
  });

  it("prefersReducedMotion returns false in server env", () => {
    expect(prefersReducedMotion()).toBe(false);
  });

  it("staggerCss explicit baseMs overrides default", () => {
    const result = staggerCss(3, 80);
    expect(result).toEqual({ "--stagger-ms": "240ms" });
  });

  it("staggerCss defaults baseMs to 80", () => {
    const result = staggerCss(2);
    expect(result).toEqual({ "--stagger-ms": "160ms" });
  });

  it("staggerCss index 0 gives 0ms", () => {
    const result = staggerCss(0);
    expect(result).toEqual({ "--stagger-ms": "0ms" });
  });
});