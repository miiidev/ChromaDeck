/**
 * ChromaDeck motion/animation utilities.
 *
 * Thin helpers for stagger entrance, reduced-motion detection,
 * and CSS custom-property generation.  No animation engine —
 * all motion is CSS-driven (transform + opacity only).
 */

/* ── Stagger-once flag ──────────────────────────────────────────────── */

const STAGGER_KEY = "__chromadeck_stagger_done";

/**
 * Consume the once-per-session stagger allowance.
 * Returns `true` only once per app lifetime; `false` on subsequent calls
 * so that stagger animation plays on initial load only.
 */
export function consumeStagger(): boolean {
  if (typeof window === "undefined") return true;
  if ((window as any)[STAGGER_KEY]) return false;
  (window as any)[STAGGER_KEY] = true;
  return true;
}

/** Reset stagger flag (useful in tests). */
export function resetStagger(): void {
  if (typeof window !== "undefined") {
    delete (window as any)[STAGGER_KEY];
  }
}

/* ── Reduced-motion preference ──────────────────────────────────────── */

/** Live check of prefers-reduced-motion. */
export function prefersReducedMotion(): boolean {
  if (typeof window === "undefined") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)")?.matches ?? false;
}

/** Eager snapshot at import time (no runtime overhead in hot paths). */
export const REDUCED_MOTION: boolean =
  typeof window !== "undefined"
    ? prefersReducedMotion()
    : false;

/* ── CSS custom-property helpers ────────────────────────────────────── */

/**
 * Returns a CSS custom-property declaration for stagger animation delay.
 * Usage: `style={staggerCss(i)}` on an element with class `enter-stagger`.
 */
export function staggerCss(index: number, baseMs = 80): Record<string, string> {
  return { "--stagger-ms": `${index * baseMs}ms` };
}

/* ── Entrance replay (window re-show) ─────────────────────────────────── */

/**
 * Replay the staged load animation on the existing DOM without remounting.
 *
 * The main window is hidden — not destroyed — when closed to the tray, so
 * mount-time CSS animations would otherwise play once per app lifetime.
 * The backend emits `window-shown` every time the window is re-shown; the
 * frontend answers with this function, which force-restarts the entrance
 * animations in place. State, scroll position, and open dialogs are
 * preserved (only the `animation` inline override is touched, then
 * cleared so the stylesheet value applies again).
 */
export function replayEntrance(): void {
  if (typeof window === "undefined" || typeof document === "undefined") return;
  if (prefersReducedMotion()) return;

  const restart = (el: HTMLElement) => {
    el.style.animation = "none";
    // Force a reflow so the override above commits; clearing it then
    // restarts the stylesheet animation from frame 0 with its own delay.
    void el.offsetHeight;
    el.style.animation = "";
  };

  // Shell regions + sidebar content + monitor rows: their entrance classes
  // and stagger variables are always present, so a restart suffices.
  const regions = document.querySelectorAll<HTMLElement>(
    ".shell-enter, .sidebar-content-enter, .monitor-row-enter",
  );
  regions.forEach(restart);

  // Preset cards: the `enter-stagger` class is only attached on first load
  // (see consumeStagger), so re-attach it with per-card delays before
  // restarting.
  const cards = document.querySelectorAll<HTMLElement>(".preset-deck > *");
  cards.forEach((card, i) => {
    card.classList.add("enter-stagger");
    card.style.setProperty("--stagger-ms", `${i * 80}ms`);
    restart(card);
  });
}