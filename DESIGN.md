# ChromaDeck — Stock shadcn/ui Dark Theme

## Theme

Uses Tailwind v4 `@theme` directive in `src/App.css` with shadcn `base-nova` style.

| Token (--color-*) | Value | Role |
|---|---|---|
| `background` | `#08090F` | Page background |
| `foreground` | `#E8EAF0` | Primary body text |
| `card` | `#10131D` | Card/surface background |
| `card-foreground` | `#E8EAF0` | Card text |
| `popover` | `#151925` | Popover/dialog/section bg |
| `popover-foreground` | `#E8EAF0` | Popover text |
| `primary` | `#22D3EE` | Primary action accent (APPLY, focus) |
| `primary-foreground` | `#FFFFFF` | Primary button text |
| `secondary` | `#1683FF` | Informational accent (EDIT, info) |
| `secondary-foreground` | `#FFFFFF` | Secondary button text |
| `muted` | `#1A1E2A` | Muted/hover background |
| `muted-foreground` | `#8B8FA3` | Secondary/muted text |
| `accent` | `#7C3AED` | Pinned/attention accent |
| `accent-foreground` | `#FFFFFF` | Accent button text |
| `destructive` | `#EC4899` | Destructive action (DELETE) |
| `destructive-foreground` | `#FFFFFF` | Destructive button text |
| `border` | `rgba(255,255,255,0.12)` | Standard borders |
| `input` | `rgba(255,255,255,0.08)` | Input border |
| `ring` | `#22D3EE` | Focus ring |

## Geometry (shadcn defaults)

| Element | Radius |
|---|---|
| Buttons, inputs, badges | `rounded-lg` (8px) |
| Cards, dialogs, popovers | `rounded-xl` (12px) |
| Badges | `rounded-4xl` (pill) |

## Components

All UI components from `src/components/ui/` are stock shadcn `base-nova` style:
- Button, Card, Badge, Dialog, Input, Label, Select (native `<select>`), Skeleton
- Icon library: Lucide

## Color Roles

| Element | Classes |
|---|---|
| APPLY / primary action button | `bg-primary text-primary-foreground` |
| EDIT / CREATE button | `bg-secondary text-secondary-foreground` |
| DUP / secondary actions | `border-border text-foreground bg-card` (outline variant) |
| DEL / destructive | `bg-destructive/10 text-destructive` (destructive variant) |
| UNPIN | outline variant |
| PINNED badge | outline variant with `text-accent border-accent` |
| ICC badge | outline variant |
| Connected dot | `●` primary |
| Pinned / attention | `■` accent |
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

- Sans: `Inter` (400/500/600/700) with system-ui fallback
- Mono: `JetBrains Mono` (400/500) for IDs, status, parameter values, meta data
- Labels: `text-xs uppercase tracking-widest`
- Body: `text-sm` / `text-xs` shadcn defaults