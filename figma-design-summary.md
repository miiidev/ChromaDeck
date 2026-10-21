# Figma Design Summary - ChromaDeck (vfF5g0hTALNbScBM4Nz1Y1)

## Color Tokens (Dark Theme)

| Token | Value | Usage |
|-------|-------|-------|
| `paper` / `background` | `#0b0f12` | App bg, card bg alternate, dialog overlay |
| `card` / `popover` | `#11161b` | Cards, dialogs, popovers |
| `muted` | `#2d343c` | Muted surfaces, borders, disabled states |
| `mint` / `primary` / `accent` | `#60a5fa` | Primary buttons, active states, IN USE badges |
| `ink` / `foreground` | `#f0f1f2` | Light body text, main copy |
| `muted-foreground` | `#929aa3` | Secondary/placeholder text |
| `border` | `#242a30` | Card borders, dividers, focus rings |

## Component Structures

### Button (variants, `rounded-brutal` 12px, `shadow-brutal-sm`)
- `default`: `bg-mint text-ink border-ink shadow-brutal-sm hover:brightness-95`
- `outline`: `border-ink bg-white text-ink shadow-brutal-sm hover:bg-muted`
- `secondary`: same as outline
- `destructive`: `border-ink bg-danger text-ink shadow-brutal-sm hover:brightness-95`
- `ghost`: `hover:bg-muted hover:text-foreground`

### Badge (variants, `rounded-4xl` pills, `rounded-[6px]` tags)
- `default`: `border-ink bg-mint text-ink`
- `secondary`: `border-ink bg-white text-ink`
- `destructive`: `border-ink bg-danger text-ink`
- `outline`: `border-ink text-ink`
- `ghost`: `hover:bg-muted hover:text-muted-foreground`
- `pinned`: `border-ink bg-ink text-mint`

### MonitorCard
- `bg-[#101419]` with `border-[#60a5fa]` when active
- `bg-[#101419]` with `border-[#2d343c]` when inactive
- Name: `text-[#f0f1f2]` or `text-[#929aa3]`
- Pin dot: `bg-[#60a5fa]` mint fill
- Badge: `bg-[#101419]` with mint/gray border

### PresetCard
- Header: `bg-[#101419]` with `border-[#2d343c]`
- Stat grid: `bg-paper` (`--color-paper`)
- IN USE badge: `bg-mint` with `text-ink`
- Category: `text-[#929aa3]` muted-foreground
- Title: `text-[#f0f1f2]` ink

### Library Header
- `border-[#242a30]` with buttons `bg-[#101419]` or `bg-[#60a5fa]` (mint, white text)

### IdentifyOverlay
- `fixed inset-0 flex items-center justify-center bg-ink/60`
- Mint `text-[128px]` number, white supporting text

## Design System Variables

- **CSS Custom Properties**: `--color/paper`, `--color/muted`, `--color/card`, `--color/ink`, `--color/mint`
- **Motion Tokens**: `--motion-micro` (150ms), `--motion-fast` (250ms), `--motion-std` (350ms), `--motion-slow` (600ms)
- **Easing**: `--ease-out` (cubic-bezier(0.22, 1, 0.36, 1)), `--ease-spring` (cubic-bezier(0.34, 1.56, 0.64, 1))
- **Geometry**: `12px` brutal radius, `6px` for small chips, `pill` for badges

## Key: Figma (Dark) vs Codebase (Light)

Existing `DESIGN.md` documents **light theme** (`paper: #FBF7F3`, `ink: #000000`, `card: #FFFFFF`, `mint: #3DDC97`).  
Figma design is **dark theme** (`paper: #0b0f12`, `ink: #f0f1f2`, `card: #11161b`, `mint: #60a5fa`).

Codebase must be updated to support dark theme from Figma, using CSS variables (`var(--color/...)`) rather than hardcoded hex values.