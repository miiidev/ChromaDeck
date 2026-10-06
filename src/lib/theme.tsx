/// Dark-only theme — stubbed for compat.
/// shadcn stock theme handles all color tokens via Tailwind @theme in App.css.

import type { ReactNode } from "react";

export type ChromaDeckTheme = "chromadeck-dark";

export function getStoredTheme(): ChromaDeckTheme {
  return "chromadeck-dark";
}

export function setStoredTheme(_theme: ChromaDeckTheme): void {
  /* no-op */
}

export function toggleTheme(_theme: ChromaDeckTheme): ChromaDeckTheme {
  return "chromadeck-dark";
}

export function applyTheme(_theme: ChromaDeckTheme): void {
  document.documentElement.dataset.theme = "chromadeck-dark";
}

/** Minimal compat provider — always dark, no toggle. */
export function ThemeProvider({ children }: { children: ReactNode }) {
  applyTheme("chromadeck-dark");
  return <>{children}</>;
}

/** Always dark, toggle is a no-op. */
export function useTheme() {
  return { theme: "chromadeck-dark" as ChromaDeckTheme, toggle: () => {} };
}