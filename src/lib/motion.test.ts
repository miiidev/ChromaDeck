import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { consumeStagger, resetStagger, staggerCss, prefersReducedMotion, REDUCED_MOTION, replayEntrance, maskedRevealClass, valueSwapClass } from "./motion";

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

  it("staggerCss defaults baseMs to 60", () => {
    const result = staggerCss(2);
    expect(result).toEqual({ "--stagger-ms": "120ms" });
  });

  it("staggerCss index 0 gives 0ms", () => {
    const result = staggerCss(0);
    expect(result).toEqual({ "--stagger-ms": "0ms" });
  });
});

describe("maskedRevealClass / valueSwapClass", () => {
  it("maskedRevealClass returns class + style with delay", () => {
    const r = maskedRevealClass(40);
    expect(r.cls).toBe("enter-reveal");
    expect(r.style).toEqual({ "--reveal-delay": "40ms" });
  });

  it("maskedRevealClass defaults delay to 0", () => {
    const r = maskedRevealClass();
    expect(r.style).toEqual({ "--reveal-delay": "0ms" });
  });

  it("valueSwapClass returns class in normal env", () => {
    // In test env (no DOM), shouldAnimate returns false → empty string
    const g = globalThis as any;
    const realWindow = g.window;
    g.window = { matchMedia: () => ({ matches: false }) };
    expect(valueSwapClass()).toBe("value-swap");
    g.window = realWindow;
  });

  it("valueSwapClass returns empty under reduced motion", () => {
    const g = globalThis as any;
    const realWindow = g.window;
    g.window = { matchMedia: () => ({ matches: true }) };
    expect(valueSwapClass()).toBe("");
    g.window = realWindow;
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

  it("restarts shell regions and staggers up to 6 deck cards", () => {
    const shell = makeEl();
    const cards = [makeEl(), makeEl(), makeEl(), makeEl(), makeEl(), makeEl(), makeEl(), makeEl()];
    installDom([shell], cards, false);

    replayEntrance();

    // Restart trick leaves no inline animation override behind.
    expect(shell.style.animation).toBe("");
    // First 6 cards get stagger
    for (const [i, card] of cards.slice(0, 6).entries()) {
      expect(card.classList.contains("enter-stagger")).toBe(true);
      expect(card.style.getPropertyValue("--stagger-ms")).toBe(`${i * 60}ms`);
      expect(card.style.animation).toBe("");
    }
    // Cards beyond index 5 are NOT restarted
    for (const card of cards.slice(6)) {
      expect(card.classList.contains("enter-stagger")).toBe(false);
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