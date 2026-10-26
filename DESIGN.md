# ChromaDeck — Flat Dark Minimal UI (Figma source of truth)

## Theme

Flat Dark Minimal: near-black canvas with a four-step elevation ladder — no glassmorphism, no radial glow orbs, no drop shadows (the only exception is the drag ghost). Single mint-teal accent (`#5eead4`) for primary actions and active state; each preset can carry its own tag colour as its identity. Wide geometric Grotesk typography (Geist). A horizontal, snap-scrolling preset carousel.

| Token (--color-*) | Value | Role |
|---|---|---|
| `background` | `#060608` | Page, header, footer |
| `surface` | `#0d0e11` | Cards, monitor pads, inner slots |
| `surface-raised` | `#15161a` | Dialogs, popovers/dropdowns, drag ghost, undo toast |
| `surface-hover` | `#1b1d22` | Hover fills, slider track |
| `border` | `rgba(255,255,255,0.06)` | All 1px hairline borders |
| `border-hover` | `rgba(255,255,255,0.12)` | Hover borders |
| `ring` | `#5eead4` | Focus rings (3px) |
| `foreground` | `#f4f1ea` | Body text (bone white) |
| `foreground-muted` / `muted-foreground` | `#a1a7af` | Secondary text, labels |
| `subtle-fg` | `#858b94` | Tertiary text, metadata (AA on every surface) |
| `primary` | `#5eead4` | Primary fills and active state, with dark text |
| `primary-foreground` | `#060608` | Text on primary fills |
| `primary-subtle` | `rgba(94, 234, 212, 0.12)` | Applied badge bg, undo toast bg |
| `primary-border` | `rgba(94, 234, 212, 0.3)` | Mint accent borders |
| `destructive` | `#ef4444` | Delete fills (white text) |
| `destructive-foreground` | `#ffffff` | Text on destructive fills |
| `warning` | `#fbbf24` | Attention states (graphics only) |

Red *text* uses `#f87171` (the fill red is below AA as text on raised surfaces).

### Preset tag identity

Every preset may carry a colour tag (`color_tag`, `#rrggbb`) and a marker shape (`tag_shape`: pill, round, square, bar, diamond, hexagon, triangle, ring). The tag renders as one solid marker (`PresetTag`, `data-slot="preset-tag"`) at 8×14 (cards) / 10×16 (monitor slot). Components expose it as a `--tag` CSS variable so state styling can reuse it: an applied card's border, a monitor pad's pinned border, the drop-target glow and the apply sweep all use `var(--tag, var(--color-primary))`. Untagged presets fall back to mint.

## Geometry

| Element | Radius |
|---|---|
| Cards, buttons, dialogs, inputs | 8px (`rounded-[8px]`) |
| Preset tag marker | per shape — pill/bar/ring/round (full), square 3–4px, diamond 2px, hexagon/triangle clipped |
| Badges | full (`rounded-full`) |
| Checkbox indicator | 4px |
| Button trailing icon circle | 32px (disc, inherently circular) |

Header, footer, cards, dialogs and buttons use `rounded-[8px]`; tags and badges are the only rounded-full shapes. Bezel padding removed; cards use `p-2` outer / `p-4` inner.


## Contrast Table (WCAG AA target ≥4.5:1, computed)

| Foreground | Background | Ratio | Level |
|---|---|---|---|
| `#f4f1ea` (body) | `#060608` (background) | **17.9:1** | AAA |
| `#f4f1ea` (body) | `#0d0e11` (surface) | **17.1:1** | AAA |
| `#f4f1ea` (body) | `#15161a` (raised) | **16.0:1** | AAA |
| `#a1a7af` (muted) | `#0d0e11` (surface) | **8.0:1** | AAA |
| `#858b94` (subtle) | `#0d0e11` (surface) | **5.6:1** | AA |
| `#858b94` (subtle) | `#1b1d22` (hover) | **4.9:1** | AA |
| `#060608` (ink) | `#5eead4` (mint fill) | **13.7:1** | AAA |
| `#f87171` (danger text) | `#15161a` (raised) | **6.5:1** | AA |

Mint fill on dark is legible; mint text is reserved for active/`IN USE` labels. Focus is a 3px solid mint ring. Every text token clears AA 4.5:1 on every surface in the ladder.

## Components

UI components in `src/components/ui/` follow shadcn `base-nova` conventions (data-slot API, cva variants) built on Base UI React primitives:
- Button, Badge, Card, Dialog, Input, Label, Select, Checkbox, Radio Group, Skeleton (Base UI); Slider (Radix); WindowDots (custom three-dot motif for panel headers)
- Figma-drawn glyphs live in `src/components/ui/icons.tsx` (monitor, reset, probe, identify, apply arrow, trailing plus)
- Icon library: Lucide for generic actions (pencil, duplicate, trash, pin)

Badges are mono 9px, `px-2 py-[3px]`, rounded-[8px] (`border-hover` hairline + muted text; mint subtle + mint text for `pinned` / `in-use`).
StatTile is a bare label/value stack — no well, no border.
Dialogs use `rounded-[8px]`, border `border-border`, bg `surface-raised`; controls use `rounded-[8px]`; every border is a 1px hairline (`border`). No shadows, no blur.

Drag-and-drop (pointer-based, portal ghost): preset cards are draggable; monitor cards are drop targets. Drag ghost renders a miniature preset card (`scale(0.98)`, `rotate(-0.5deg)`, `opacity 0.9`, raised surface) in a portal on `document.body`. Source card dims to 40% opacity / scale 0.97 while dragging. "All monitors" drop target appears as a mint pill at the top of the monitors panel only during a drag; drop on library background cancels. Apply-in-progress shows a spinner in the slot; applied state shows the badge plus a one-shot tag-coloured sweep; error shows destructive text.

## Color Roles

| Element | Classes |
|---|---|
| APPLY / primary action button | `bg-primary text-primary-foreground` (dark text on mint) |
| + Create / empty-state CTA | mint primary — the view's main forward action |
| Select / Cancel / outline | `border-border bg-surface text-foreground hover:bg-surface-hover` |
| DELETE / destructive | `bg-destructive text-white` |
| Applied / active card | 1px border in the preset's tag colour (`var(--tag)`, mint when untagged) |
| Pinned monitor pad | 1px border in the pinned preset's tag colour; slot tinted ~8% tag colour |
| PINNED tag | `pinned` variant: `bg-primary-subtle text-primary` |
| IN USE badge | mint subtle, mint text |
| ICC chip | outline, muted mono text |
| Stat | label mono 9px muted, value mono 17px (mint when non-neutral, bone when neutral) |
| UNPIN | icon-only ghost |
| Monitor identity | 24px monitor glyph (mint connected / subtle offline) + APPLIED/DEFAULT/OFFLINE badge — no separate status dot |
| Offline row | dashed `border` on paper, muted name, actions at 40% opacity |
| Warning | `--color-warning` (`#fbbf24`), graphics only |
| Focus | 3px `ring` (mint) |
| Footer | flat strip (`h-14`, `border-t border-border`, `bg-background`), mono 10px counts |

## Dark surfaces

The app is dark-only. No light-mode sections. The Identify overlay is a transient, transparent full-monitor window: a centred raised card (8px radius, 70% background alpha) with the monitor number in `font-display` mint, the name in bone, and a mono caption; type scales with `vmin` (number `clamp(96px, 22vmin, 360px)`) and the window is click-through.

## Header Layout

```
[22px seven-tile mark]  ChromaDeck  |  v{package.json}
```

Flat strip (`h-12`, `border-b border-border`, `bg-background`, `px-6`): left cluster (mark + wordmark 18px Geist SemiBold, tracking -0.54px, then a 1px × 16px divider and the mono 10px version). Right cluster intentionally empty — actions live in the footer status bar. No theme toggle — dark-only. The version badge renders `v{version}` imported from `package.json` (never hardcoded); the mark is `src/assets/chromadeck-brand.svg` at 22px (`.logo-lockup`).

## Monitor dashboard

`MONITORS (n)` section heading (28px): WindowDots (26 × 6) + mono 11px label on the left, an `Identify` glyph + mono 10px muted action on the right. Below, a grid (`gap-4`) of 160px-tall monitor cards (breakpoints: <760px 1 col, ≥760px 2 cols, ≥1000px 3 cols):

```
Outer: h-[160px] rounded-[8px] p-2 border border-border bg-surface
  Inner: rounded-[8px] p-4 border border-border-hover bg-surface
    Identity: 24px monitor glyph (mint connected / subtle offline) + 17px name + 9px mono serial + APPLIED/DEFAULT/OFFLINE badge
    Slot: drop target for preset drag-and-drop. Empty: dashed border border-border + "Drop preset here" label. Filled: PresetTag marker + preset name (mono 10px) + category (mono 9px); background tinted 8% of the tag colour.
    Actions: Apply-to-target appears only on hover (rounded-[8px] border button). Reset / Probe utility actions (13px glyph + mono 10px label).
```

While a preset is dragged over a valid monitor the pad scales to 1.01 and its border plus inner ring take the dragged preset's tag colour; dropping settles it back over 150ms. An offline monitor shakes instead.

## Footer status bar

Flat strip (`h-14`, `border-t border-border`, `bg-background`, `pl-6 pr-4`): health light + `N PRESETS · M MONITORS CONNECTED · K PINNED` (mono 10px), a transient reapply message, a 1px × 20px divider, the **Autostart** checkbox (14px mint-checked box + mono 10px label), and the **Reapply** ghost button (rounded-[8px], mono 10px uppercase).

The health light is 6px: mint when at least one monitor is connected and the last Reapply had no errors, red with a continuous pulse after a Reapply error, muted while loading or with no monitors. It plays one expanding ring whenever its state changes.

## Logo system

Seven-tile "C" mark: graphite flat surface with one mint accent (Figma `Brand/Logo`, transparent background).

| Asset | Used for |
|---|---|
| `src/assets/chromadeck-brand.svg` | header lockup (22px) — exported from Figma |
| `src/assets/chromadeck-icon.svg` | legacy deck-pad mark (32px) |
| `src/assets/chromadeck-icon-1024.png` | source for `npx tauri icon` regeneration |
| `src/assets/chromadeck-icon-small.svg`, `chromadeck.ico` | source material |
| `public/logo.svg` | favicon (clean drawing, no provenance metadata) |
| `src-tauri/icons/*` | window, tray, taskbar, and installer icons (generated) |

Regenerate with `npx tauri icon src/assets/chromadeck-icon-1024.png`. `src-tauri/build.rs` pins `cargo:rerun-if-changed` on the bundled icons so a swapped logo always reaches the exe — cargo otherwise keeps linking previously compiled resources and the new mark silently never ships.

## Typography

- Display: `Geist` (400/600/700 via `@fontsource/geist`) for the wordmark, library title, card titles, monitor names, button labels
- Mono: `JetBrains Mono` (500/700) for IDs, status, parameter labels/values, readouts, meta data
- Body: `Geist`, then system sans fallback — no remote fonts, fully offline
- Mono labels: `text-[9px]` … `text-[11px]`, no added tracking
- Body: `text-sm` / `text-xs` defaults in dialogs

## Type scale

Body text uses the Tailwind scale only. Exceptions, each with a fixed role:

| Size | Role | Where |
|---|---|---|
| `text-[9px]` | Micro: badges, stat labels, serials, pinned row | Badges, StatTile, monitor identity |
| `text-[10px]` | Meta: version, footer counts, utility labels, apply action | Header, footer, monitor actions, PresetCard action strip |
| `text-[11px]` | Section eyebrow | `MONITORS (n)` |
| `text-[13px]` | Button labels | Every Button |
| `text-[17px]` | Card stat values, monitor names | StatTile, MonitorCard |
| `text-[18px]` | Wordmark | Header |
| `text-2xl` (24px) | Section title | `Your library`, selection count |
| `text-[32px]` | Fixed: PresetCard title (overflow via truncation + hover marquee + tooltip) | PresetCard title |
| `text-[128px]` | One-off: Identify overlay number | IdentifyOverlay |

Display type is rule-driven, not arbitrary. No other sizes are allowed; a new size needs a row in this table.

### Letter-spacing rule

| Tier | Tracking | Where |
|---|---|---|
| Display title (18px+) | `-3%` … `-4%` | `CHROMADECK` (-0.54px @18), `Your library` (-0.72px @24), card titles (-1.28px @32) |
| Mono micro-label (9-11px, uppercase) | `0` | Badges, section eyebrows, stat labels, footer counts |

Uppercase mono labels carry no artificial tracking — the typeface supplies the rhythm.

### Numerals

Any value that changes in place uses `tabular-nums` so digits do not jitter: stat values, the footer count strip, editor slider readouts, and apply-result rows.

## Motion system

CSS-driven, in `src/App.css` with helpers in `src/lib/motion.ts`. Emil-first: **no animation** unless it clarifies a state change or gives tactile feedback. Transform + opacity only (WebView2 performance) — never layout properties or shadow blur. Custom easings, never bare `ease`.

| Token | Value | Role |
|---|---|---|
| `--motion-micro` | 80ms | exits, validation, transients |
| `--motion-fast` | 150ms | rows, cards, banners, dialogs |
| `--motion-standard` | 250ms | shell + staggered entrances |
| `--motion-slow` | 400ms | slower-than-std tier |
| `--motion-slower` | 600ms | full page transitions |
| `--motion-shell` | 350ms | staged shell entrance |
| `--ease-fluid` | `cubic-bezier(0.32, 0.72, 0, 1)` | **primary easing** — all entrances, scroll reveals |
| `--ease-out` | `cubic-bezier(0.22, 1, 0.36, 1)` | standard entrances |
| `--ease-spring` | `cubic-bezier(0.34, 1.56, 0.64, 1)` | press, pop-in, thumb grab |
| `--ease-standard` | `cubic-bezier(0.4, 0, 0.2, 1)` | standard state changes |
| `--ease-exit` | `cubic-bezier(0.4, 0, 1, 1)` | exits |

Signature interaction — **a 1px press**: buttons translate 1px down on `:active` (80–150ms). The trailing icon in Button-in-Button translates diagonally (`translate-x-1`) on hover. Both use the `translate` property (not `transform`) so they compose with the scroll-driven edge-fade keyframes, which own `transform`.

| What plays | Rule |
|---|---|
| Shell entrance (mount + every window re-show) | 350ms glide, staged 0/60/120/180ms — Header → MonitorDashboard → PresetLibrary → Footer |
| Preset cascade | 350ms glide, stagger capped at 5 × 60ms (≤300ms total, no stagger-spam) |
| Monitor dashboard cards | 3 × 60ms = 180ms total |
| Dialog reveal | Title (0ms) → Body (80ms) → Actions (160ms) |
| Dialog exit | The dialog keeps its local `open` state false until Base UI's `onOpenChangeComplete` fires, so the close animation plays before the parent unmounts it |
| Pin-tag enter, ICC snap, apply-confirm badge | 180ms overshoot-free snap (`motion-scale-in`) |
| Delete detonation | 300ms one-shot (timeout in `handleDelete` matches) |
| Scroll edge-fade, title marquee (hover-only), View-Transition gap-glide | kept — user- or scroll-driven |
| Validation, feedback, transients, error banner, footer crossfade | short fades/slides tied to state changes |
| Empty state | small fade (the one delight carve-out) |
| Card hover | border brightens only — no lift, no shadow |
| Card action strip | visible at 60% opacity, 100% on hover/focus-within; icon brightens, 1px press, delete icon turns red |
| Slider thumb grab | scale only; values track input with zero easing |
| Reduced motion | zeroes all durations/delays globally, and `replayEntrance()` no-ops under it |
| Drag ghost | scale 0.98, rotate -0.5deg, opacity 0.9; follows pointer via portal; static under reduced motion |
| Apply sweep | one 500ms tag-tinted wash left→right across the monitor slot when an apply succeeds |
| Drop target | pad scales to 1.01 while a valid preset hovers it; border + inner ring in the dragged preset's tag colour; settles over 150ms. Offline pads shake once (240ms) |
| Filter crossfade | search/category changes run inside a View Transition (150ms); rapid typing skips overlapping transitions |
| Footer health | one expanding ring (`health-pulse`) whenever the state changes; the error-state dot keeps its continuous pulse |
| Editor tag morph | the selected colour/shape scales to 1.1 and the selected shape preview snaps in (`motion-scale-in`); no shape tween |

## Preset Library (horizontal carousel)

The library is a single-row, snap-scrolling carousel of preset cards (`min-w-[288px]`, 16px gap), not a grid. Cards keep their natural height (~177px) and vertically centre in the available space; when the panel is shorter than that, it scrolls vertically rather than squashing the cards.

- Header row (`h-10`): `PRESETS (n)` (or `n of m` while filtering) on the left; category dropdown, search field (`/` or Ctrl/Cmd+F to focus, Esc to clear) and `+ Create preset` on the right.
- The carousel has 48px inline padding. A horizontal mask fades the first and last 48px so cards dissolve into the panel instead of clipping at the viewport edge.
- Arrows sit vertically centred on the visible panel, outside the mask; they step exactly one card and disable at each end.
- Loading state mirrors the carousel with a row of card skeletons.

The section heading is `PRESETS (n)` (mono 10px, uppercase, 0.14em tracking).

## Flat Card Architecture

```
Outer: rounded-[8px] p-2 border border-border bg-surface
  Inner: rounded-[8px] p-4 border border-border-hover bg-surface
    Content...
```

PresetCard and MonitorCard both use this pattern. The active/pinned card swaps the outer border to the preset's tag colour (`var(--tag, var(--color-primary))`). No glass fill, no inset highlight, no shadows.

PresetCard body: status badges (PresetTag marker + optional tag-colour dot + category + ICC + IN USE) → 32px title → mono summary line (always visible) → action strip (`Apply →` + edit/duplicate/delete/unpin icons, 60% opacity, 100% on hover/focus).

## Button-in-Button Pattern

Buttons are `rounded-[8px]` (`h-11`, `px-6`, text 13px bold). A trailing icon NEVER sits naked next to the text: it is nested in a distinct 32px circle, flush with the pill's right inner padding (`pl-6 pr-1.5`, gap 12px). Primary buttons use `bg-primary text-primary-foreground`; ghost/secondary use `border border-border bg-surface text-foreground hover:bg-surface-hover`.

On hover: the button lifts 1px and the inner icon circle translates diagonally (`translate-x-1 scale-105`), creating internal kinetic tension.
