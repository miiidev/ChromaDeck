import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { consumeStagger, resetStagger, staggerCss, prefersReducedMotion, REDUCED_MOTION, replayEntrance } from "./motion";

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

describe("replayEntrance (fake DOM)", () => {
  const g = globalThis as any;
  let realWindow: unknown;
  let realDocument: unknown;

  interface FakeEl {
    style: { animation: string; setProperty: (k: string, v: string) => void; getPropertyValue: (k: string) => string };
    classList: { add: (c: string) => void; contains: (c: string) => boolean };
    offsetHeight: number;
    __vars: Record<string, string>;
    __classes: Set<string>;
  }

  function makeEl(): FakeEl {
    const vars: Record<string, string> = {};
    const classes = new Set<string>();
    return {
      style: {
        animation: "",
        setProperty: (k: string, v: string) => {
          vars[k] = v;
        },
        getPropertyValue: (k: string) => vars[k] ?? "",
      },
      classList: {
        add: (c: string) => {
          classes.add(c);
        },
        contains: (c: string) => classes.has(c),
      },
      offsetHeight: 24,
      __vars: vars,
      __classes: classes,
    };
  }

  function installDom(shellEls: FakeEl[], cardEls: FakeEl[], reduced: boolean) {
    g.window = { matchMedia: () => ({ matches: reduced }) };
    g.document = {
      querySelectorAll: (sel: string) =>
        sel.includes("preset-deck") ? cardEls : shellEls,
    };
  }

  beforeEach(() => {
    realWindow = g.window;
    realDocument = g.document;
    g.window = undefined;
    g.document = undefined;
  });

  afterEach(() => {
    g.window = realWindow;
    g.document = realDocument;
  });

  it("is a no-op without DOM globals", () => {
    expect(() => replayEntrance()).not.toThrow();
  });

  it("restarts shell regions and staggers deck cards", () => {
    const shell = makeEl();
    const cards = [makeEl(), makeEl(), makeEl()];
    installDom([shell], cards, false);

    replayEntrance();

    // Restart trick leaves no inline animation override behind.
    expect(shell.style.animation).toBe("");
    for (const [i, card] of cards.entries()) {
      expect(card.classList.contains("enter-stagger")).toBe(true);
      expect(card.style.getPropertyValue("--stagger-ms")).toBe(`${i * 80}ms`);
      expect(card.style.animation).toBe("");
    }
  });

  it("skips everything under reduced motion", () => {
    const shell = makeEl();
    const cards = [makeEl()];
    installDom([shell], cards, true);

    replayEntrance();

    expect(shell.style.animation).toBe("");
    expect(cards[0].classList.contains("enter-stagger")).toBe(false);
    expect(Object.keys(cards[0].__vars)).toHaveLength(0);
  });
});