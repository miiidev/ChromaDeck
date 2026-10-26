# ChromaDeck Design Token / Component Mapping

This document maps Figma design tokens to ChromaDeck codebase design tokens, and Figma components to code components.

## Source of Truth

- **Figma** (file key: `vfF5g0hTALNbScBM4Nz1Y1`) — authoritative for appearance
- **ChromaDeck codebase** — authoritative for behavior

## Design Token Mapping

### Color Tokens

| Figma Token | Figma Value | Code Token (`--color-*`) | Code Value | File |
|-------------|-------------|-------------------------|------------|------|
| paper/background | #0b0f12 | `background` / `paper` | #030305 | src/App.css @theme |
| card/popover | #11161b | `surface` | #040407 | src/App.css @theme |
| muted | #2d343c | `muted` | #2d343c | src/App.css @theme |
| mint/primary/accent | #60a5fa | `primary` | #5eead4 | src/App.css @theme |
| ink/foreground | #f0f1f2 | `foreground` / `ink` | #f4f1ea | src/App.css @theme |
| muted-foreground | #929aa3 | `foreground-muted` | #a1a7af | src/App.css @theme |
| border | #242a30 | `border` / `input` | rgba(255,255,255,0.06) | src/App.css @theme |

**Note:** Figma values are dark theme; codebase values (from DESIGN.md) are light theme. Figma is source of truth — codebase should converge toward Figma dark theme values. Update `src/App.css` @theme block when implementing Figma designs.

### Geometry Tokens

| Figma Element | Figma Value | Code Token (`--radius-*`) | Code Value | File |
|---------------|-------------|--------------------------|------------|------|
| Outer shell (Double-Bezel) | - | `radius-card` | 12px | src/App.css @theme |
| Inner core | - | - | calc(2rem-6px) | src/App.css |
| Cards, dialogs | - | `radius-md`/`radius-lg` | 8px/12px | src/App.css @theme |
| Buttons, inputs, sliders | - | `radius-sm` | 6px | src/App.css @theme |
| Badges, chips | pill | `radius-full` | 9999px | src/App.css @theme |

### Motion Tokens

| Figma Token | Figma Value | Code Token (`--motion-*`) | Code Value | File |
|-------------|-------------|--------------------------|------------|------|
| micro | 150ms | `motion-micro` | 80ms | src/App.css @theme / src/lib/motion.ts |
| fast | 250ms | `motion-fast` | 150ms | src/App.css @theme / src/lib/motion.ts |
| standard | 350ms | `motion-standard` | 250ms | src/App.css @theme / src/lib/motion.ts |
| slow | 600ms | `motion-slow` | 400ms | src/App.css @theme / src/lib/motion.ts |

### Typography Tokens

| Figma Element | Code Token | Code Value | File |
|---------------|------------|------------|------|
| Display headings | `--font-display` | "Geist Display", "Montserrat", ... | src/App.css @theme |
| Mono readouts | `--font-mono` | "JetBrains Mono", ui-monospace | src/App.css @theme |
| Body text | `--font-body` | system-ui, "Segoe UI", sans-serif | src/App.css @theme |

## Component Mapping

| Figma Component | Code Component | File | data-slot | cva Variants |
|-----------------|---------------|------|-----------|--------------|
| Button | Button | src/components/ui/Button.tsx | data-slot="button" | default, outline, destructive, ghost, secondary |
| Badge | Badge | src/components/ui/Badge.tsx | data-slot="badge" | default, outline, destructive, ghost, secondary, pinned |
| Card | Card | src/components/ui/Card.tsx | data-slot="card" | default, brutal |
| Dialog | Dialog | src/components/ui/Dialog.tsx | data-slot="dialog-overlay"/"dialog-content" | - |
| Input | Input | src/components/ui/Input.tsx | data-slot="input" | - |
| Select | Select | src/components/ui/Select.tsx | data-slot="select-trigger"/"select-content"/"select-item" | - |
| Checkbox | Checkbox | src/components/ui/Checkbox.tsx | data-slot="checkbox" | - |
| Radio | Radio | src/components/ui/Radio.tsx | data-slot="radio-group-item" | - |
| Slider | Slider | src/components/ui/Slider.tsx | data-slot="slider"/"slider-thumb" | - |
| StatTile | StatTile | src/components/ui/StatTile.tsx | data-slot="stat-tile" | - |
| MonitorCard | MonitorCard | src/components/MonitorCard.tsx | data-slot="card" | - |
| PresetCard | PresetCard | src/components/PresetCard.tsx | data-slot="card" | - |
| MonitorSidebar | MonitorSidebar | src/components/MonitorSidebar.tsx | data-slot="sidebar" | - |
| MonitorList | MonitorList | src/components/MonitorList.tsx | data-slot="library" | - |

## Code Connect (Optional)

Code Connect maps Figma nodes to code components. Available via `figma_get_code_connect_suggestions` and `figma_send_code_connect_mappings`. Not mandatory — use when component mapping is ambiguous.

## Token Update Procedure

When implementing a Figma design that introduces new values:

1. Retrieve actual Figma values via figma-inspect skill.
2. Add new CSS custom properties to `src/App.css` `@theme` block.
3. Update component styles to use `var(--color-*`, `var(--radius-*`, etc.
4. Update DESIGN.md with new token values if they become project-wide standards.
5. Run `npm run build` to verify CSS syntax.
6. Run visual-qa to confirm visual fidelity.
