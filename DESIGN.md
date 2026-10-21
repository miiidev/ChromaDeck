# ChromaDeck — Ethereal Glass UI (Figma source of truth)

## Theme

Ethereal Glass: deepest OLED black (`#030305`), radial mesh gradients (subtle glowing purple/emerald orbs) in the background. Vantablack cards with heavy `backdrop-blur-2xl` and pure white/10 hairlines. Wide geometric Grotesk typography. Single mint-teal accent (`#5eead4`). Asymmetrical Bento layout for preset library.

| Token (--color-*) | Value | Role |
|---|---|---|
| `paper` / `background` | `#030305` | App background, header, footer |
| `surface` | `#040407` | Cards, dialogs, popovers |
| `surface-hover` | `#060609` | Hover surfaces, slider track |
| `glass-surface` | `rgba(13, 17, 23, 0.7)` | Glassmorphism surfaces |
| `glass-border` | `rgba(255,255,255,0.06)` | All 1px hairline borders |
| `glass-highlight` | `rgba(255,255,255,0.12)` | Inner highlight on glass surfaces |
| `foreground` / `ink` | `#f4f1ea` | Body text (bone white) |
| `foreground-muted` | `#a1a7af` | Secondary text, labels |
| `foreground-subtle` | `#6b7280` | Tertiary text, metadata |
| `primary` / `mint` / `accent` | `#5eead4` | Fill only, with dark text (`primary-foreground` `#030305`) |
| `primary-hover` | `#2dd4bf` | Primary hover |
| `primary-foreground` | `#030305` | Text on primary fills |
| `primary-subtle` | `rgba(94, 234, 212, 0.15)` | Selected/hover rows, pinned badge bg |
| `primary-border` | `rgba(94, 234, 212, 0.3)` | Applied/active borders |
| `secondary` | `#040407` | Hover surfaces |
| `muted` | `#2d343c` | Subtle fills, offline dots |
| `muted-foreground` | `#a1a7af` | Secondary text, labels |
| `tint` | `#12181f` | Dialog footer strip |
| `destructive` / `danger` | `#ef4444` | Delete fills (white text, 4.6:1 AA) |
| `danger-ink` | `#f87171` | Red running text (errors, warnings) |
| `border` / `input` | `rgba(255,255,255,0.06)` | All 1px borders, input borders |
| `border-hover` | `rgba(255,255,255,0.12)` | Hover borders |
| `border-focus` | `#5eead4` | Focus rings |
| `ring` | `#5eead4` | Focus rings |
| `warning` | `#f59e0b` | Warning amber |
| `success` | `#22c55e` | Status dots (graphics only, never text) |
| `inset` | `#030305` | Stat tiles / inset wells |
| `overlay` | `rgba(3, 3, 5, 0.8)` | Modal overlay |
| `overlay-strong` | `rgba(3, 3, 5, 0.9)` | Strong modal overlay |
| `glow-purple` | `#8a6bff` | Radial glow accent |
| `glow-emerald` | `#4fc3ff` | Radial glow accent |

## Geometry

| Element | Radius |
|---|---|
| Outer shell (Double-Bezel) | 32px |
| Inner core (Double-Bezel) | 26px |
| Cards, dialogs | 12px |
| Buttons, inputs, sliders, stat tiles, rows | 8px |
| Badges, chips (ICC, IN USE, PINNED) | pill (9999px) |
| Window dots, radio, checkbox indicator | 4px |
| Button trailing icon circle | 14px |

## Contrast Table (WCAG AA target ≥4.5:1, computed)

| Foreground | Background | Ratio | Level |
|---|---|---|---|
| `#f4f1ea` (body) | `#030305` (paper) | **18.2:1** | AAA |
| `#f4f1ea` (body) | `#040407` (surface) | **17.8:1** | AAA |
| `#030305` (ink) | `#5eead4` (mint fill) | **8.9:1** | AAA |
| `#f4f1ea` (body) | `#060609` (surface-hover) | **16.5:1** | AAA |
| `#a1a7af` (muted) | `#030305` (paper) | **8.1:1** | AA |
| `#f4f1ea` (body) | `#ef4444` (danger fill) | **5.2:1** | AA |

Mint fill on dark is legible; mint text is banned. Red running text uses `danger-ink`, never the fill red. Focus is a 3px solid mint ring.

## Components

UI components in `src/components/ui/` follow shadcn `base-nova` conventions (data-slot API, cva variants) built on Base UI React primitives:
- Button, Badge, Card, Dialog, Input, Label, Select, Checkbox, Radio Group, Skeleton (Base UI); Slider (Radix); WindowDots (custom three-dot motif for dialog and panel headers)
- Icon library: Lucide (to be replaced with Phosphor Light / Remix Line in future)

Dialogs use the 12px `rounded-card` radius; controls use the 8px `rounded-brutal` radius; every border is a 1px hairline (`border`). Shadows are soft, never offset hard blocks.

## Color Roles

| Element | Classes |
|---|---|
| APPLY / primary action button | `bg-primary text-primary-foreground` (dark text on mint) |
| + Create / empty-state CTA | mint primary — the view's main forward action |
| Select / Cancel / outline | `border-border bg-surface text-foreground hover:bg-surface-hover` |
| DELETE / destructive | `bg-destructive text-white` |
| Applied / active pad | `border-primary` on the card (no fill change) |
| PINNED tag | `pinned` variant: `bg-primary-subtle text-primary` pill |
| IN USE badge | mint fill, dark text, pill |
| ICC chip | outline pill, muted text |
| UNPIN | outline variant |
| Connected dot | mint circle (`primary`); pinned adds mint square |
| Offline row | dashed `border` on paper, muted name |
| Warning | amber accents (`#f59e0b`, graphics only) |
| Focus | 3px `ring` (mint) |
| Footer | paper fill, muted mono counts |
| Stat tile | `bg-inset`, 1px border, 8px radius |

## Dark surfaces (the one exception)

The Identify overlay is a transient fullscreen flash, not app chrome: it dims the monitor with `bg-foreground` and shows the number in mint `font-display` with the name in bone white. Mint-on-bone-white here is the workshop reference and clears AAA; it never appears on dark surfaces.

## Header Layout

```
[deck-pad logo]  ChromaDeck  [v{package.json} badge]
```

Floating glass pill (`mt-6 mx-auto w-max rounded-full p-3 px-6`): left cluster (logo + title + version); right cluster intentionally empty — actions live in the footer status bar. No theme toggle — dark-only. The version badge renders `v{version}` imported from `package.json` (never hardcoded); the mark is the deck-pad logo at 32px (`.logo-lockup`).

## Footer status bar

Floating glass pill (`mb-6 mx-auto w-max rounded-full p-3 px-6`): `N PRESETS · M MONITORS CONNECTED · K PINNED` counts (mono, crossfading), transient action feedback, the **Reapply** button (re-runs enforcement now), and the **Start with Windows** autostart toggle.

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

- Display: `Geist Display` (700/800) for headings, dialog titles, buttons, card titles — caps with wide tracking for eyebrows and titles
- Mono: `JetBrains Mono` (500/700) for IDs, status, parameter values, readouts, meta data
- Body: system sans (`system-ui, "Segoe UI", sans-serif`) — no remote fonts, fully offline
- Labels: `text-xs uppercase tracking-[0.12em]`
- Body: `text-sm` / `text-xs` defaults

## Type scale

Body text uses the Tailwind scale only (`text-xs` 12px secondary, `text-sm` 14px body, `text-base` 16px titles/inputs). Exceptions, each with a fixed role:

| Size | Role | Where |
|---|---|---|
| `text-[10px]` | Micro: badges, tiny footnotes, tracked microcopy | Badges, editor footnote, autostart label |
| `text-[11px]` | Eyebrow: card category labels only | Preset card header |
| `text-[32px]` | Fixed: PresetCard title (overflow via truncation + hover marquee + tooltip) | PresetCard title |
| `text-[128px]` | One-off: Identify overlay number | IdentifyOverlay |

Display type is rule-driven, not arbitrary. No other sizes are allowed; a new size needs a row in this table.

### Letter-spacing rule

Two tiers only, so micro-labels never drift apart:

| Tier | Tracking | Where |
|---|---|---|
| Micro-label (10-12px, uppercase, mono) | `tracking-[0.12em]` | Section titles (`MONITORS`, `LIBRARY`), field labels, stat labels, status chips, footer counts |
| Display title (16px+, uppercase) | `tracking-wide` | `CHROMADECK`, dialog titles, empty-state headline |

A hint that follows a label (`(optional, import only)`) sits on the same row, is `text-[9px]`, is `normal-case`, and **truncates** rather than wrapping the label.

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

Signature interaction — **a 1px press**: controls translate 1px down on `:active` and lift 1px on `:hover` (~80-150ms). The trailing icon in Button-in-Button translates diagonally (`translate-x-1 scale-105`) on hover. Both use the `translate` property (not `transform`) so they compose with the scroll-driven edge-fade keyframes, which own `transform`.

| What plays | Rule |
|---|---|
| Shell entrance (mount + every window re-show) | 350ms glide, staged 0/60/120/180ms — Header → MonitorDashboard → PresetLibrary → Footer |
| Preset cascade | 350ms glide, stagger capped at 5 × 60ms (≤300ms total, no stagger-spam) |
| Monitor dashboard cards | 3 × 60ms = 180ms total |
| Dialog reveal | Title (0ms) → Body (80ms) → Actions (160ms) |
| Pin-tag enter, ICC snap, apply-confirm badge | 180ms overshoot-free snap (`motion-scale-in`) |
| Delete detonation | 300ms one-shot (timeout in `handleDelete` matches) |
| Scroll edge-fade, title marquee (hover-only), View-Transition gap-glide | kept — user- or scroll-driven |
| Validation, feedback, transients, error banner, footer crossfade | short fades/slides tied to state changes |
| Empty state | small fade (the one delight carve-out) |
| Pad hover | 1px lift; border brightens (no shadow growth) |
| Card action strip | icon brightens on hover, 1px press, delete icon turns red |
| Select-mode tremble | fast side-to-side jitter (0.28s linear) on the stagger wrapper — never the pad, so no fight with edge-fade; per-card desync via `nth-child` negative delays |
| Slider thumb grab | scale only; values track input with zero easing |
| Reduced motion | zeroes all durations/delays globally, and `replayEntrance()` no-ops under it |

## Bento Grid Layout (Preset Library)

CSS Grid with 12 columns, 88px column width, 16px gutter. Container padding 8px so rings don't clip at edges.

| Row | Layout | Card Sizes |
|---|---|---|
| 1 | Hero + Standard + Standard | 7-col (568px) + 3-col (292px) + 3-col (292px) |
| 2 | 4 × Standard | 3-col (292px) each |

Gap: 16px both axes. No empty cells — grid reshapes to content.

## Double-Bezel Card Architecture

```
Outer Shell: p-1.5 rounded-[2rem] bg-black/5 ring-1 ring-white/5 shadow-[0_24px_48px_-16px_rgba(0,0,0,0.7)]
  Inner Core: rounded-[calc(2rem-6px)] bg-surface shadow-[inset_0_1px_1px_rgba(255,255,255,0.12)]
    Content...
```

PresetCard, MonitorCard, DialogContent all use this pattern. The outer shell catches the ambient light; the inner core is the actual content surface.

## Button-in-Button Pattern

Primary interactive buttons are fully rounded pills (`rounded-full`) with generous padding (`px-6 py-3`). If a button has a trailing icon (arrow), it NEVER sits naked next to the text. It must be nested inside its own distinct circular wrapper (`w-8 h-8 rounded-full bg-black/5 flex items-center justify-center`) placed completely flush with the main button's right inner padding.

On hover: button lifts (`-translate-y-[1px] -translate-x-[1px]`), inner icon circle translates diagonally (`translate-x-1 scale-105`), creating internal kinetic tension.