import { describe, it, expect } from "vitest";
import { getStoredTheme, toggleTheme, applyTheme } from "./theme";

describe("getStoredTheme", () => {
  it("always returns dark", () => {
    expect(getStoredTheme()).toBe("chromadeck-dark");
  });
});

describe("toggleTheme", () => {
  it("always returns dark", () => {
    expect(toggleTheme("chromadeck-dark")).toBe("chromadeck-dark");
  });
});

describe("applyTheme", () => {
  it("sets data-theme attribute", () => {
    Object.defineProperty(globalThis, "document", {
      value: {
        documentElement: {
          dataset: {} as Record<string, string>,
        },
      },
      writable: true,
      configurable: true,
    });
    applyTheme("chromadeck-dark");
    expect(document.documentElement.dataset.theme).toBe("chromadeck-dark");
  });
});