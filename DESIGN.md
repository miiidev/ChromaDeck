# ChromaDeck — Neobrutalist Dark Theme (post-v0.5.0)

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
| Buttons, inputs, badges | `rounded-lg` (8px) |
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

All UI components from `src/components/ui/` are stock shadcn `base-nova` style:
- Button, Card, Badge, Dialog, Input, Label, Select (native `<select>`), Skeleton
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
[logo]  ChromaDeck  [v0.4.0 badge]  [reapply] [autostart]
```

Single row (`flex justify-between`): left cluster (logo + title + version), right cluster (actions). No theme toggle — dark-only.

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