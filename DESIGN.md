# ChromaDeck — Bauhaus Dual-Theme Design

## Tokens

| Token | Light | Dark | Role |
|-------|-------|------|------|
| Paper | `#F4F1EA` | `#1E1E1E` | Card/section surfaces |
| Page bg | `#F4F1EA` | `#141414` | Main background |
| Ink | `#111111` | `#F4F1EA` | Text, borders, rules (inverted in dark) |
| Ink secondary | `#555555` | `#C0BBA0` | Secondary text, labels |
| Ink muted | `#999999` | `#8A8678` | Placeholder, disabled, hints |
| Red | `#E30613` | `#E30613` | APPLY / destructive / error |
| Blue | `#0066B3` | `#0066B3` | EDIT / info / connected |
| Yellow | `#FFCC00` | `#FFCC00` | Warning / brightness / UNPIN |

## Grid & Spacing

- Base grid unit: 8px
- Content max-width: 12-column layout
- Section gap: 20px (`gap-5` in Tailwind)
- Container padding: `p-6` (24px)
- Card padding: `p-3` (12px)

## Typography

- Headings: `Archivo` weight 600/700, uppercase, `tracking-[0.12em]`
- Body: `Inter` with system sans fallback
- Numerals: `ui-monospace` (kept from current design)
- Labels: `text-[10px]`, uppercase, `tracking-widest`

## Geometry

- Blocks/cards/buttons: `border-radius: 0`
- Circle motifs / status dots: `rounded-full`
- Shadow light: `4px 4px 0 #111` (ink)
- Shadow dark: `4px 4px 0 #000`
- Small button shadow: `2px 2px 0`

## Color Roles (binding — prevents Mondrian noise)

| Element | Colors |
|---------|--------|
| APPLY button | `bg-red` + white text |
| EDIT / CREATE button | `bg-blue` + white text |
| DUP button | `bg-paper` + `border-ink` |
| DEL button | `bg-ink` + `text-red` + `border-red` |
| UNPIN button | `bg-yellow` + `text-ink` |
| PINNED badge | `border-blue` + `text-blue` |
| ICC badge | `border-ink` + `text-secondary` |
| RETRY / REAPPLY | `border-ink` + `text-secondary` |
| Connected dot | `●` blue |
| Pinned dot | `■` red |
| Warning | `▲` yellow |

## Slider Thumb Shapes

| Slider | Shape | Color | CSS |
|--------|-------|-------|-----|
| Gamma | Square | Blue | `data-thumb="square"` |
| Brightness | Circle | Yellow | `data-thumb="circle"` |
| Contrast | Triangle | Red | `data-thumb="triangle"` `clip-path` |
| RGB gains | Square | Per-channel | `data-thumb="r"` / `"g"` / `"b"` |
| Vibrance/Hue | Square | Ink | (default) |

## Interaction States

- `:hover` on surface → `--surface-hover` (4% darker)
- `:focus-visible` → 2px blue outline, offset 3px
- `:active` on button → `translate-y-0.5`, no shadow
- `:disabled` → gray (`#777`), `opacity-0.5`, `cursor-not-allowed`
- `prefers-reduced-motion` → disable active translate

## Header Layout

```
[28px-square split red/blue/yellow + circle + triangle SVG]  ChromaDeck  [v0.1.0 badge]  [●/◐ LIGHT|DARK]
```

Asymmetric grid: left cluster (logo + title + version), right cluster (theme toggle).