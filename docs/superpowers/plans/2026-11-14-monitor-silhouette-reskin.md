# Monitor Silhouette Reskin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Reshape the monitor drop-target card into the approved flat monitor silhouette (screen bezel + neck + stand) with the approved state treatments and drop/apply motion, purely visually.

**Architecture:** Extract the card's state derivation (phase, accent colour, tint, labels) into a pure `src/lib/monitorSilhouette.ts` module that is unit-tested, then re-render `MonitorCard` in `src/components/MonitorSidebar.tsx` as a silhouette using that module. Motion is CSS-only in `src/App.css` using the existing motion-token system.

**Tech Stack:** React 19 + TypeScript, Tailwind v4 tokens in `src/App.css` (`@theme`), Vitest (node env, `src/lib/*.test.ts` pattern), Playwright for visual QA.

**Spec:** `docs/superpowers/specs/2026-11-14-figma-component-reskin-design.md` + approved Figma concept frames:
- State components `97:201` (Empty), `97:211` (Drag over), `97:222` (Applying), `97:233` (Applied), `97:245` (Pinned), `97:258` (Offline), `97:268` (Error) on page `84:185`, file `vfF5g0hTALNbScBM4Nz1Y1`.
- In-context concept frame `97:357`.

## Global Constraints

- **Visual only.** No changes to apply logic, navigation, persistence, drag-and-drop state machine, Tauri integrations, or component props/API of `MonitorSidebar`.
- **No commits.** `src/components/MonitorSidebar.tsx`, `src/App.tsx` etc. carry the user's uncommitted work (`git status` confirmed). Changes stay uncommitted; the user commits.
- **Footprint fixed.** Card grid cell stays 453.3×160 (compact: 146 high); the grid in `MonitorSidebar` is untouched.
- **Preserve every existing control and behavior:** rename (hover pencil), Reset (with message), Probe (with report), Unpin, drag-from-filled-slot, offline drop rejection + shake, auto-badge, applied chip, entrance stagger.
- **Approved design values (from Figma masters):** bezel radius 6, bezel border `--color-border` 1px (accent = tag colour during drag-over), screen fill `--color-surface`, neck 21×13 centred, foot 67×3 radius 1, tint = tag colour at 10% (drag-over) / 8% (applied), name 15px `font-display` semibold, serial 11px mono muted, status label 10px mono uppercase, empty prompt 15px muted, applied title 20px semibold, summary 11px, utility labels 12px sans.
- **Colour is never the only signal:** every tag-coloured state keeps its text label (status label / prompt / error note).
- **Reduced motion:** looping wash becomes a static tint; sweep stays suppressed per the existing `prefers-reduced-motion` block.
- Verification for every task: `npm run build` and `npm run test` must pass.

## Review Focus

1. **Drag-from-filled-slot breaks** if new tint/wash overlays intercept pointer events → overlays must be `pointer-events-none`; visual check in Pass 2.
2. **Compact mode (5+ monitors)** squeezes the 102px screen — name, serial, status, and slot content must still fit; visual check in Pass 2.
3. **Offline monitor with a pinned preset** must read OFFLINE (muted, status text), never look successfully applied.
4. **Error with no previously assigned preset** must still show "Apply failed" + message, not a blank screen.
5. **Long preset/monitor names** truncate inside the bezel without shifting the stand geometry (fixed bezel height, truncate classes).

---

### Task 1: `monitorSilhouette` state derivation (lib, TDD)

**Files:**
- Create: `src/lib/monitorSilhouette.ts`
- Test: `src/lib/monitorSilhouette.test.ts`

**Interfaces:**
- Consumes: existing `Preset` type fields `color_tag` (string), `name`; `applyStatus` shape `{ phase: "applying" | "applied" | "error"; presetName: string; error?: string }` (read-only).
- Produces (used by Task 2/3):
  ```ts
  export type SilhouettePhase =
    | "dragover" | "applying" | "error" | "pinned" | "auto" | "applied"
    | "empty" | "offline";
  export interface SilhouetteState {
    phase: SilhouettePhase;
    /** Hex accent colour for bezel border + screen tint, or null. */
    accent: string | null;
    /** Screen tint: accent at this opacity (drag-over 0.10, applied/pinned/auto 0.08). */
    tintOpacity: number | null;
    /** Uppercase status label, top-right of the screen. */
    statusLabel: string;
    /** Main screen line (title / prompt). */
    primaryLabel: string;
    /** Secondary line under primaryLabel, or null. */
    secondaryLabel: string | null;
    /** True when the offline treatment (muted screen + stand) applies. */
    muted: boolean;
  }
  export function deriveSilhouetteState(input: {
    connected: boolean;
    isDropHover: boolean;
    samePresetApplied: boolean;
    draggedTag: string | null;
    slotPreset: { name: string; color_tag: string } | null;
    isPinned: boolean;
    isAuto: boolean;
    wasApplied: boolean;
    statusPhase: "applying" | "applied" | "error" | undefined;
  }): SilhouetteState
  export const SILHOUETTE = { screenH: 116, screenHCompact: 102, neckW: 21, neckH: 13, footW: 67, footH: 3, tintDrag: 0.10, tintApplied: 0.08 } as const;
  ```

Derivation rules (pin these):
- `!connected` → `offline` (muted, statusLabel `OFFLINE`, accent = null even if pinned).
- `statusPhase === "applying"` → `applying`, accent = draggedTag (drag-over) ?? slot tag.
- `isDropHover && connected && !samePresetApplied` → `dragover`, accent = draggedTag, tintDrag.
- `statusPhase === "error"` → `error`, accent = slot tag if slotPreset still visible (previous preset retained), statusLabel `APPLY FAILED`.
- `isPinned` → `pinned`; else `isAuto` → `auto`; else `wasApplied` (appliedMap hit) → `applied`; else `empty`.
- Applied/pinned/auto: statusLabel = preset category uppercase fallback `ASSIGNED`, tintApplied.
- Empty: statusLabel `DEFAULT`, primaryLabel `Drop preset here` (or `Release to apply` while dragging — caller passes via input? No: caller already renders drag prompt; keep `empty` primaryLabel `Drop preset here` and let the component swap while `isDragging`).
- Invalid `color_tag` (not `/^#[0-9a-fA-F]{6}$/`) → accent null.

- [x] **Step 1: Write the failing test** covering: offline-with-pin, dragover tint/opacity, applying, error-retains-preset, pinned/auto/applied, empty, invalid colour tag.
- [x] **Step 2: Run `npx vitest run src/lib/monitorSilhouette.test.ts`** — expect FAIL (module missing).
- [x] **Step 3: Implement `deriveSilhouetteState` + `SILHOUETTE`** per rules above.
- [x] **Step 4: Run test — expect PASS; then `npm run build`.**

### Task 2: Silhouette card structure (`MonitorCard`)

**Files:**
- Modify: `src/components/MonitorSidebar.tsx` (MonitorCard render body only; all handlers/props unchanged)

**Interfaces:**
- Consumes: `deriveSilhouetteState`, `SILHOUETTE` from Task 1; existing `PresetTag`, `UtilityButton`, `MonitorNameEditor`, drop/apply status as today.
- Produces: same external behaviour; DOM gains `[data-slot="monitor-screen"]`, `[data-slot="monitor-stand"]` for visual QA selectors.

Structure (replaces outer `p-2` card + inner border panel — the double border dissolves into the bezel):
```
card (grid cell, 160/146 high, entrance stagger kept, no own border/fill)
├─ screen  (h 116/102, radius 6, bg-surface, 1px border-border; borderColour = accent ?? border)
│   ├─ tint overlay (absolute, accent @ tintOpacity, pointer-events-none)   [Task 3 classes]
│   ├─ identity row: name 15px semibold + hover pencil, serial 11px mono, status label 10px upper right, AUTO badge
│   ├─ slot content (centred): applying spinner+label / error-note row / PresetTag + title 20px + summary 11px + Applied chip / prompt 15px
│   └─ success sweep span (existing apply-sweep, kept)
├─ neck  (21×13, mx-auto, bg-border)
├─ foot  (67×3, mx-auto, bg-border, radius 1)
└─ actions row (h-22: Reset / Probe / Unpin — UtilityButton labels → 12px sans)
```

- [x] **Step 1: Rework the render body** exactly to the structure above; every existing conditional branch (applying / error / slotTitle / empty prompt / offline / dragOrigin dim / unpinError) keeps its trigger condition.
- [x] **Step 2: Error branch change:** when `statusPhase === "error"` and `slotPreset` exists, keep rendering the preset row + tint, and render the destructive error note (existing `validation-slide`, truncated, `title=status.error`) beneath the title. No preset → current two-line error block.
- [x] **Step 3: Delete the now-unused inner-panel wrapper + old chip styling; keep `chipLabel` only as `statusLabel` from Task 1.**
- [x] **Step 4: Verify:** `npm run build` passes; dev-server visual check: empty card, pinned card, offline card, compact grid — screenshot vs Figma `97:357`.
- [x] **Step 5: `npm run test` — all pass.**

### Task 3: Motion polish (`App.css`)

**Files:**
- Modify: `src/App.css` (motion section, ~lines 130–430)

**Interfaces:**
- Consumes: existing tokens `--motion-fast` 150ms, `--motion-standard` 250ms, `--motion-slow` 400ms, `--ease-out`, `--ease-fluid`; existing `.apply-sweep`, `.monitor-drop-target`, `.validation-slide`.
- Produces: `.screen-tint` (fade/transition opacity), `.applying-wash` (looping gradient sweep), `.reset-drain` (tint fade-out), plus their `prefers-reduced-motion` fallbacks.

Keyframes/classes:
- `.screen-tint`: `transition: opacity var(--motion-fast) var(--ease-out)`; mounts with opacity 0 → target when phase gains accent (component sets inline opacity).
- `.applying-wash`: `@keyframes applying-wash { from translateX(-100%) to translateX(200%) }`, a 30%-width accent gradient at ~0.12 alpha, `1.4s linear infinite`, inside the screen, `pointer-events-none`.
- `.reset-drain`: one-shot `motion-fade-in reverse`-style opacity fade of the tint over `--motion-standard` when the applied entry is removed (component keys it).
- Reduced-motion block: `.applying-wash { animation: none; opacity: 0.08 }`, `.screen-tint { transition: none }`.

- [x] **Step 1: Add keyframes + classes + reduced-motion rules.**
- [x] **Step 2: Wire in `MonitorCard`:** drag-over tint uses `.screen-tint`; applying state adds `.applying-wash` layer (replaces nothing else); reset success (applied entry cleared) renders `.reset-drain` once via key.
- [x] **Step 3: Verify:** `npm run build`; `npm run test` (incl. existing `src/lib/motion.test.ts`); visual check of drag-over → applying → applied sequence using browser demo data.

### Task 4: Final visual QA + sign-off evidence

**Files:**
- Create (temp only, outside repo): Playwright screenshots in `C:/Users/Ahmad Syahmi/AppData/Local/Temp/opencode/`

- [x] **Step 1:** Screenshot the running app (demo data, 1440×900): empty/default grid, a monitor with pinned preset, offline monitor, compact (>4 monitors) grid.
- [x] **Step 2:** Compare against Figma `97:357` + state board `97:278`; list any mismatch and fix in `MonitorSidebar.tsx`/`App.css` only.
- [x] **Step 3:** Final `npm run build` + `npm run test`; report honestly which checks ran.
