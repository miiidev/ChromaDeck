# ChromaDeck — Light neobrutalist theme

## Theme

Light-only: paper background, white surfaces, 2px black borders, hard offset
shadows, lime accent. No dark variants anywhere (`dark:` selectors were
removed, not maintained). Lime is a **fill with black text** — never lime
text or thin lime lines on a light surface. Preview and swatch areas
(`PreviewStrip`, stat tiles) stay neutral so the chrome never competes
with the colours being judged.

| Token (--color-*) | Value | Role |
|---|---|---|
| `paper` / `background` | `#FBF7F3` | App background |
| `foreground` / `ink` | `#000000` | Body text, borders, focus rings |
| `card` / `popover` | `#FFFFFF` | Cards, dialogs, popovers |
| `primary` / `lime` / `accent` | `#D4E84F` | Fill only, always with black text |
| `secondary` | `#FFFFFF` | White buttons (black border via classes) |
| `muted` | `#ECE7DC` | Muted surfaces, skeleton shimmer |
| `muted-foreground` | `#555555` | Secondary text |
| `tint` / `accent-soft` | `#F1F3DB` | Dialog footer strip, selected rows |
| `destructive` / `danger` | `#E5484D` | Delete fills (black text) |
| `danger-ink` | `#B91C1C` | Red running text (errors, warnings) |
| `border` / `input` / `ring` | `#000000` | Borders, input borders, focus rings |
| `warning` | `#F5A623` | Warning amber |
| `success` | `#16A34A` | Status dots (graphics only, never text) |
| `--border-width-brutal` | `2px` | Every border |
| `--shadow-brutal-sm` | `2px 2px 0 #000` | Buttons, chips, small surfaces |
| `--shadow-brutal` | `4px 4px 0 #000` | Pads, dialogs |
| `--radius-brutal` | `12px` | Cards, dialogs, buttons |

## Geometry

| Element | Radius |
|---|---|
| Buttons, inputs, cards, dialogs | 12px (brutal) |
| Badges | pill |
| Small chips (ICC, IN USE) | 6px |

## Contrast Table (WCAG AA target ≥4.5:1, computed)

| Foreground | Background | Ratio | Level |
|---|---|---|---|
| `#000000` (body) | `#FBF7F3` (paper) | **19.7:1** | AAA |
| `#000000` (body) | `#FFFFFF` (card) | **21.0:1** | AAA |
| `#000000` (body) | `#D4E84F` (lime fill) | **15.5:1** | AAA |
| `#000000` (body) | `#F1F3DB` (tint) | **18.6:1** | AAA |
| `#000000` (body) | `#ECE7DC` (muted) | **17.1:1** | AAA |
| `#555555` (muted) | `#FBF7F3` (paper) | **7.0:1** | AA |
| `#555555` (muted) | `#FFFFFF` (card) | **7.5:1** | AA |
| `#000000` (body) | `#E5484D` (delete fill) | **5.3:1** | AA |
| `#B91C1C` (red text) | `#FFFFFF` (card) | **6.5:1** | AA |
| `#16A34A` (success dot) | `#FFFFFF` | 3.3:1 | graphics only, never text |

Lime text on light fails and is banned outright; red running text uses
`danger-ink`, never the fill red. Focus is a 3px solid black ring.

## Components

UI components in `src/components/ui/` follow shadcn `base-nova` conventions
(data-slot API, cva variants) built on Base UI React primitives:
- Button, Badge, Card, Dialog, Input, Label, Select, Checkbox, Radio Group, Skeleton (Base UI); Slider (Radix)
- Icon library: Lucide

Dialogs and popovers use the 12px brutal radius; all other geometry matches the table above.

## Color Roles

| Element | Classes |
|---|---|
| APPLY / primary action button | `bg-lime text-ink border-ink shadow-brutal-sm` |
| + Create / empty-state CTA | lime primary — the view's main forward action |
| Select / Cancel / outline | `bg-white text-ink border-ink shadow-brutal-sm` |
| DELETE / destructive | `bg-danger text-ink border-ink shadow-brutal-sm` |
| Applied pad | full `bg-lime` fill (black text, neutral stat tiles) |
| PINNED tag | `pinned` variant: black tag, lime text |
| IN USE badge | lime fill, black text + border |
| ICC chip | white chip, black border |
| UNPIN | outline variant |
| Connected dot | green circle; pinned adds lime square + black tag |
| Offline row | dashed black border, paper fill, muted name |
| Warning | amber accents (graphics only) |
| Focus | 3px solid black ring |
| Footer | white, black top rule, mono counts |

## Dark surfaces (the one exception)

The Identify overlay is a transient fullscreen flash, not app chrome: it
dims the monitor with `bg-ink/60` and shows the number in lime
`font-display` with the name in white. Lime-on-black here is the workshop
reference and clears AAA; it never appears on light surfaces.

## Header Layout

```
[deck-pad logo]  ChromaDeck  [v{package.json} badge]
```

Single row (`flex justify-between`): left cluster (logo + title + version); the right cluster is intentionally empty — actions live in the footer status bar. No theme toggle — light-only. The version badge renders `v{version}` imported from `package.json` (never hardcoded); the mark is the deck-pad logo at 32px (`.logo-lockup`).

## Footer status bar

`N PRESETS · M MONITORS CONNECTED · K PINNED` counts (mono, crossfading), transient action feedback, the **Reapply** button (re-runs enforcement now), and the **Start with Windows** autostart toggle.

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

- Display: `Montserrat` (700/800, bundled via `@fontsource/montserrat`) for headings, dialog titles, buttons, card titles, and the identify number — caps with wide tracking for eyebrows and titles
- Mono: `JetBrains Mono` (500/700, bundled via `@fontsource/jetbrains-mono`) for IDs, status, parameter values, readouts, meta data
- Body: system sans (`system-ui, "Segoe UI", sans-serif`) — no remote fonts, fully offline
- Labels: `text-xs uppercase tracking-widest`
- Body: `text-sm` / `text-xs` defaults

## Type scale

Body text uses the Tailwind scale only (`text-xs` 12px secondary, `text-sm` 14px body, `text-base` 16px titles/inputs). Two deliberate exceptions, each with a fixed role:

| Size | Role | Where |
|---|---|---|
| `text-[10px]` | Micro: badges, tiny footnotes, tracked microcopy | Badges, editor footnote, autostart label |
| `text-[11px]` | Eyebrow: card category labels only | Preset card header |

Display type is rule-driven, not arbitrary: card titles are fixed 32px (overflow via truncation + hover marquee + tooltip), stat values step 30 → 26 → 22px by digit count, and the identify overlay number is a one-off 128px display moment. No other sizes are allowed; a new size needs a row in this table.

## Motion system

CSS-driven, in `src/App.css` with helpers in `src/lib/motion.ts`. Emil-first:
**no animation** unless it clarifies a state change or gives tactile
feedback. Transform + opacity only (WebView2 performance) — never layout
properties or shadow blur. Custom easings, never bare `ease`.

| Token | Value | Role |
|---|---|---|
| `--motion-micro` | 150ms | exits, validation, transients |
| `--motion-fast` | 250ms | rows, cards, banners, dialogs |
| `--motion-std` | 350ms | shell + staggered entrances |
| `--motion-slow` | 600ms | (retired from entrances; kept for reference) |
| `--motion-press` | 90ms | press-down (release settles slower) |
| `--ease-out` | `cubic-bezier(0.22, 1, 0.36, 1)` | all entrances |
| `--ease-spring` | `cubic-bezier(0.34, 1.56, 0.64, 1)` | thumb grab only |

Signature interaction — **press into the shadow**: pads translate 4px and
buttons 2px with the shadow dropping to zero, ~90ms ease-out. Uses the
`translate` property (not `transform`) so it composes with the
scroll-driven edge-fade keyframes, which own `transform`.

| What plays | Rule |
|---|---|
| Shell entrance (mount + every window re-show) | 350ms glide, staged 0/60/120/180ms — tightened for a tray utility |
| Preset cascade | 350ms glide, stagger capped at 5 × 60ms (≤300ms total, no stagger-spam) |
| Pin-tag enter, ICC snap, apply-confirm badge | 180ms overshoot-free snap (`motion-scale-in`) |
| Delete detonation | 300ms one-shot (timeout in `handleDelete` matches) |
| Scroll edge-fade, title marquee (hover-only), View-Transition gap-glide | kept — user- or scroll-driven |
| Validation, feedback, transients, error banner, footer crossfade | short fades/slides tied to state changes |
| Empty state | small fade (the one delight carve-out) |
| Slider thumb grab | scale only; values track input with zero easing |

Deliberately absent: hover lifts, label-grow, select tremble, status
blink, breathing pulses, bounce-in pops, overlay blur, uniform
mount fade-ins, sidebar-collapse rule. Checkbox/radio indicators appear
instantly. Reduced motion zeroes all durations/delays globally, and
`replayEntrance()` no-ops under it.