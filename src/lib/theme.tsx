/// Bauhaus theme provider — dual light/dark with localStorage persistence.
/// Pure functions (testable without DOM) + React Provider/Context.

import { useState, useEffect, useCallback, createContext, useContext, type ReactNode } from "react";

// ── Types ──────────────────────────────────────────────────────────────────

export type BauhausTheme = "bauhaus-light" | "bauhaus-dark";

// ── Storage key ─────────────────────────────────────────────────────────────

const STORAGE_KEY = "chromadeck-theme";

// ── Pure functions (testable in node) ───────────────────────────────────────

/** Read persisted theme; fall back to light when unavailable or invalid. */
export function getStoredTheme(): BauhausTheme {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "bauhaus-light" || stored === "bauhaus-dark") return stored;
  } catch {
    // privacy mode or missing localStorage
  }
  return "bauhaus-light";
}

/** Persist theme choice (best-effort). */
export function setStoredTheme(theme: BauhausTheme): void {
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // privacy mode — in-memory only
  }
}

/** Toggle between light and dark. */
export function toggleTheme(theme: BauhausTheme): BauhausTheme {
  return theme === "bauhaus-light" ? "bauhaus-dark" : "bauhaus-light";
}

/** Apply theme to document root for CSS variable switching. */
export function applyTheme(theme: BauhausTheme): void {
  document.documentElement.dataset.theme = theme;
}

// ── React context ──────────────────────────────────────────────────────────

export interface ThemeCtx {
  theme: BauhausTheme;
  toggle: () => void;
}

const ThemeCtxImpl = createContext<ThemeCtx>({
  theme: "bauhaus-light",
  toggle: () => {},
});

/**
 * Wrap your top-level App element.
 * Initialises `data-theme` from localStorage, provides toggle via `useTheme()`.
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<BauhausTheme>(getStoredTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const toggle = useCallback(() => {
    const next = toggleTheme(theme);
    setTheme(next);
    setStoredTheme(next);
  }, [theme]);

  return <ThemeCtxImpl.Provider value={{ theme, toggle }}>{children}</ThemeCtxImpl.Provider>;
}

/** Consume theme state and toggle in child components. */
export function useTheme(): ThemeCtx {
  return useContext(ThemeCtxImpl);
}