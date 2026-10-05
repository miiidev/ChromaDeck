import { describe, it, expect, beforeEach, vi } from "vitest";
import { getStoredTheme, setStoredTheme, toggleTheme, applyTheme } from "./theme";

// ── localStorage mock ──────────────────────────────────────────────────────

function mockLocalStorage() {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = value;
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key];
    }),
    clear: vi.fn(() => {
      store = {};
    }),
  };
}

const mockLs = mockLocalStorage();

beforeEach(() => {
  mockLs.clear();
  Object.defineProperty(globalThis, "localStorage", {
    value: mockLs,
    writable: true,
    configurable: true,
  });
});

// ── Tests ──────────────────────────────────────────────────────────────────

describe("getStoredTheme", () => {
  it("returns light by default when nothing stored", () => {
    expect(getStoredTheme()).toBe("bauhaus-light");
  });

  it("returns light by default when stored value is invalid", () => {
    localStorage.setItem("chromadeck-theme", "invalid");
    expect(getStoredTheme()).toBe("bauhaus-light");
  });

  it("reads persisted dark theme", () => {
    localStorage.setItem("chromadeck-theme", "bauhaus-dark");
    expect(getStoredTheme()).toBe("bauhaus-dark");
  });

  it("reads persisted light theme", () => {
    localStorage.setItem("chromadeck-theme", "bauhaus-light");
    expect(getStoredTheme()).toBe("bauhaus-light");
  });

  it("falls back to light when localStorage throws", () => {
    // Simulate privacy mode
    const throwLs = {
      getItem: vi.fn(() => { throw new Error("no access"); }),
    };
    Object.defineProperty(globalThis, "localStorage", {
      value: throwLs,
      writable: true,
      configurable: true,
    });
    expect(getStoredTheme()).toBe("bauhaus-light");
  });
});

describe("setStoredTheme", () => {
  it("persists dark theme", () => {
    setStoredTheme("bauhaus-dark");
    expect(localStorage.getItem("chromadeck-theme")).toBe("bauhaus-dark");
  });

  it("persists light theme", () => {
    setStoredTheme("bauhaus-light");
    expect(localStorage.getItem("chromadeck-theme")).toBe("bauhaus-light");
  });

  it("overwrites previous value", () => {
    localStorage.setItem("chromadeck-theme", "bauhaus-dark");
    setStoredTheme("bauhaus-light");
    expect(localStorage.getItem("chromadeck-theme")).toBe("bauhaus-light");
  });
});

describe("toggleTheme", () => {
  it("toggles light → dark", () => {
    expect(toggleTheme("bauhaus-light")).toBe("bauhaus-dark");
  });

  it("toggles dark → light", () => {
    expect(toggleTheme("bauhaus-dark")).toBe("bauhaus-light");
  });
});

describe("applyTheme", () => {
  beforeEach(() => {
    // Mock document.documentElement
    Object.defineProperty(globalThis, "document", {
      value: {
        documentElement: {
          dataset: {} as Record<string, string>,
        },
      },
      writable: true,
      configurable: true,
    });
  });

  it("sets data-theme attribute on document element", () => {
    applyTheme("bauhaus-light");
    expect(document.documentElement.dataset.theme).toBe("bauhaus-light");
  });

  it("overrides with dark", () => {
    applyTheme("bauhaus-dark");
    expect(document.documentElement.dataset.theme).toBe("bauhaus-dark");
  });
});

describe("round-trip", () => {
  it("stores and retrieves", () => {
    setStoredTheme("bauhaus-dark");
    expect(getStoredTheme()).toBe("bauhaus-dark");
  });

  it("toggles and persists", () => {
    const current = getStoredTheme(); // light
    const next = toggleTheme(current); // dark
    setStoredTheme(next);
    expect(getStoredTheme()).toBe("bauhaus-dark");
  });
});