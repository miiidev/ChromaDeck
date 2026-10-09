# ChromaDeck — Dark Theme (v0.7.0)

## Theme

Uses Tailwind v4 `@theme` directive in `src/App.css` with shadcn `base-nova` style.
The v0.6.0 mint migration replaced the blue accent family (`#2f8bff`) with mint green (`#34D399`)
and tightened the success-green to `#16A34A` for clear visual separation from action elements.

| Token (--color-*) | Value | Role |
|---|---|---|
| `background` | `#08090F` | Page background |
| `foreground` | `#E8EAF0` | Primary body text |
| `card` | `#0E1118` | Card/surface background |
| `card-foreground` | `#E8EAF0` | Card text |
| `popover` | `#151A26` | Popover/dialog/section bg |
| `popover-foreground` | `#E8EAF0` | Popover text |
| `primary` | `#34D399` | Primary/accent action (APPLY, focus, EDIT, PINNED) |
| `primary-foreground` | `#0A1A33` | Primary/accent button text (dark) |
| `secondary` | `#34D399` | Same mint as primary (collapsed) |
| `muted` | `#1A1E2A` | Muted/hover background |
| `muted-foreground` | `#8B8FA3` | Secondary/muted text |
| `accent` | `#34D399` | Same mint as primary (collapsed) |
| `accent-soft` | `#0A1A33` | Soft accent (dark) |
| `destructive` | `#FF2E7E` | Destructive action (DELETE) |
| `destructive-foreground` | `#FFFFFF` | Destructive button text |
| `border` | `#272B38` | Standard borders |
| `input` | `rgba(255,255,255,0.08)` | Input border |
| `ring` | `#34D399` | Focus ring |
| `success` | `#16A34A` | Status connected/active indicator (non-accent, darker than mint action elements) |
| `warning` | `#F5A623` | Warning amber |

## Geometry

| Element | Radius |
|---|---|
| Buttons, inputs | `rounded-lg` (8px) |
| Cards, dialogs, popovers | `rounded-2xl` (16px) |
| Badges | `rounded-4xl` (pill) |

## Contrast Table (WCAG AA target ≥4.5:1)

All body/muted text pairs clear AA. Dark foreground (`#0A1A33`) on mint (`#34D399`) passes AAA
at 9.04:1 — well above the 4.5:1 AA bar. White-on-mint fails at 1.92:1 but is not used
anywhere (mint action elements use dark text per `primary-foreground`). White-on-destructive
and white-on-success pass A (≥3:1) but not AA — acceptable for small indicators and
destructive labels where dark text would be ambiguous.

| Foreground | Background | Ratio | Level |
|---|---|---|---|
| `#E8EAF0` (body) | `#08090F` (page) | **16.53:1** | AAA |
| `#E8EAF0` (body) | `#0E1118` (card) | **15.70:1** | AAA |
| `#E8EAF0` (body) | `#151A26` (popover) | **14.46:1** | AAA |
| `#E8EAF0` (body) | `#1A1E2A` (muted) | **13.82:1** | AAA |
| `#8B8FA3` (muted) | `#08090F` (page) | **6.21:1** | AA |
| `#8B8FA3` (muted) | `#0E1118` (card) | **5.90:1** | AA |
| `#8B8FA3` (muted) | `#151A26` (popover) | **5.43:1** | AA |
| `#8B8FA3` (muted) | `#1A1E2A` (muted) | **5.19:1** | AA |
| `#0A1A33` (prim-fg) | `#34D399` (primary) | **9.04:1** | AAA |
| `#06202e` (IN USE fg) | `#34D399` (primary) | **8.71:1** | AAA |
| `#FFFFFF` (white) | `#34D399` (primary) | **1.92:1** | FAIL — not used; mint uses dark-fg |
| `#FFFFFF` (white) | `#16A34A` (success) | **3.30:1** | A — small indicator dots only |
| `#FFFFFF` (destr-fg) | `#FF2E7E` (destructive) | **3.53:1** | A |

## Components

UI components in `src/components/ui/` follow shadcn `base-nova` conventions
(data-slot API, cva variants) built on Base UI React primitives:
- Button, Badge, Card, Dialog, Input, Label, Select, Checkbox, Radio Group, Skeleton (Base UI); Slider (Radix)
- Icon library: Lucide

Dialogs and popovers use `rounded-2xl` (16px); all other geometry matches the table above.

## Color Roles

| Element | Classes |
|---|---|
| APPLY / primary action button | `bg-primary text-primary-foreground` |
| EDIT / CREATE button | `bg-secondary text-primary-foreground` |
| DUP / secondary actions | `border-border text-foreground bg-card` (outline variant) |
| DEL / destructive | `bg-destructive/10 text-destructive` (destructive variant) |
| UNPIN | outline variant |
| PINNED badge | outline variant with `text-accent border-accent` |
| ICC badge | outline variant |
| Connected dot | `●` primary |
| Pinned / attention | `■` primary |
| Warning / offline | `▲` amber (hardcoded) |
| Focus ring | `ring-primary` (shadcn base-nova default) |
| Card state | via Badge variant + Card border accent |
| Side rail (DELETED) | State communicated via Badge + Card border |

## Header Layout

```
[deck-pad logo]  ChromaDeck  [v{package.json} badge]  [reapply] [autostart]
```

Single row (`flex justify-between`): left cluster (logo + title + version), right cluster (actions). No theme toggle — dark-only. The version badge renders `v{version}` imported from `package.json` (never hardcoded); the mark is the deck-pad logo at 32px (`.logo-lockup`).

## Logo system

Deck-pad mark with mint accent: dark rounded square, staggered pad grid, mint active pad.

| Asset | Used for |
|---|---|
| `src/assets/chromadeck-icon.svg` | header lockup (32px) |
| `src/assets/chromadeck-icon-1024.png` | source for `npx tauri icon` regeneration |
| `src/assets/chromadeck-icon-small.svg`, `chromadeck.ico` | source material |
| `public/logo.svg` | favicon (clean drawing, no provenance metadata) |
| `src-tauri/icons/*` | window, tray, taskbar, and installer icons (generated) |

Regenerate with `npx tauri icon src/assets/chromadeck-icon-1024.png`. `src-tauri/build.rs` pins `cargo:rerun-if-changed` on the bundled icons so a swapped logo always reaches the exe — cargo otherwise keeps linking previously compiled resources and the new mark silently never ships.

## Typography

- Display: `Archivo` (500/600/700/800, bundled via `@fontsource/archivo`) for headings, dialog titles, buttons, and preset card titles
- Mono: `JetBrains Mono` (500/700, bundled via `@fontsource/jetbrains-mono`) for IDs, status, parameter values, meta data
- Body: system sans (`system-ui, "Segoe UI", sans-serif`) — Inter was removed along with the Google Fonts link; no remote fonts, fully offline
- Labels: `text-xs uppercase tracking-widest`
- Body: `text-sm` / `text-xs` shadcn defaults

## Type scale

Body text uses the Tailwind scale only (`text-xs` 12px secondary, `text-sm` 14px body, `text-base` 16px titles/inputs). Two deliberate exceptions, each with a fixed role:

| Size | Role | Where |
|---|---|---|
| `text-[10px]` | Micro: badges, tiny footnotes, tracked microcopy | Badges, editor footnote, autostart label |
| `text-[11px]` | Eyebrow: card category labels only | Preset card header |

Display type is rule-driven, not arbitrary: card titles are fixed 32px (overflow via truncation + hover marquee + tooltip), stat values step 30 → 26 → 22px by digit count, and the identify overlay number is a one-off 128px display moment. No other sizes are allowed; a new size needs a row in this table.

## Motion system

CSS-driven, in `src/App.css` with helpers in `src/lib/motion.ts`. Transform + opacity only — no layout-property animation. Full entrances glide on `--ease-out`; small interactive transitions (presses, pops, lifts) use `--ease-spring` (gentle overshoot, tiny travel only — scale/2px lifts — never on full entrances, so nothing wobbles across the screen).

| Token | Value | Role |
|---|---|---|
| `--motion-micro` | 150ms | exits, micro feedback |
| `--motion-fast` | 250ms | rows, cards, banners |
| `--motion-std` | 350ms | standard entrances |
| `--motion-slow` | 600ms | shell + staggered entrances |
| `--ease-out` | `cubic-bezier(0.22, 1, 0.36, 1)` | all entrances |
| `--ease-spring` | `cubic-bezier(0.34, 1.56, 0.64, 1)` | presses, pops, lifts |

| Utility | Effect |
|---|---|
| `.shell-enter` (+ `--shell-delay` 0/60/120/180ms) | staged app-shell entrance: header → sidebar → library → footer (`motion-shell-up`) |
| `.enter-stagger` (+ `--stagger-ms`) | preset-card cascade (`motion-slide-up`); attached on first load only via `consumeStagger()` |
| `.monitor-row-enter` (+ `--stagger-ms`) | sidebar monitor-row cascade |
| `.sidebar-content-enter` | sidebar carousel strip entrance |
| `.enter-fade` / `.enter-slide-up` / `.enter-slide-down` / `.enter-scale` | one-shot entrances (empty states, banners, badges) |
| `.exit-fade` / `.exit-slide-up`, `.feedback-enter` / `.feedback-exit` | exits run ~30% faster than entrances |
| `.card-tremble` | looping "fear tremble" while in multi-select delete mode (per-card negative delays desync the loop) |
| `.card-explode` (+ `motion-explode`) | batch-delete detonation with per-card delay |
| `.preset-card` edge fade | scroll-driven `card-edge-vanish` on `animation-timeline: view()` where supported, with an IntersectionObserver fallback (`.no-view-timeline`, `--edge-o`) elsewhere |
| `.pulse-soft`, `.status-dot` blink, title marquee | looping accents |

Button hover grows the **label/icon inside a fixed-geometry box** (transform on `.btn-label`/svg — never font-size, which would shove neighbors); press dips the box with the label reset so the two transforms compose to exactly the press value.

The window hides (not unmounts) on close, so the backend emits `window-shown` on every re-show (tray Show, tray click, second launch) and `replayEntrance()` restarts the staged entrance in place — state, scroll, and open dialogs are preserved. Reduced motion zeroes all durations/delays globally.