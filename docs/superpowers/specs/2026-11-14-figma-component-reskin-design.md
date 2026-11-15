# ChromaDeck — Figma-only component reskin

## Intent and approved direction
Improve readability and remove the nested double-border appearance without redesigning the application. User selected single-border cards and authorized a restrained direction for other components.

## Goals
- First transfer the current working-tree UI faithfully into the existing Figma file `vfF5g0hTALNbScBM4Nz1Y1` as an editable baseline.
- Preserve that baseline and create a separate reskin copy.
- Keep one subtle outer card border; remove the decorative inner panel border, preserving content coordinates and card footprints. Meaningful input/drop-target borders remain.
- Increase frequently read 9–11px labels and actions toward 12–13px where existing bounds permit. Preserve large preset titles. Verify actual fit; report conflicts rather than moving content, removing information, or expanding cards silently.
- Refine buttons, badges, inputs, sliders, and dialogs conservatively using the existing dark/mint palette, Geist and JetBrains Mono, and current assets.
- Retain distinct applied, pinned, offline, disabled, focus, destructive, and loading states, including preset tag identities.

## Non-goals and constraints
No application source, functionality, navigation, state, Tauri integration, content, layout, section positioning, component footprint, or spacing changes. No blanket scaling, new visual identity, decorative shadows, or invented controls. Existing Figma work and uncommitted repository changes must not be overwritten. Planning documents are the only repository additions in this phase.

## Approaches
1. Single-border cards: selected and recommended; removes redundant framing while maintaining card boundaries and state indicators.
2. Borderless cards: considered but not selected; weaker separation on the existing dark surfaces.

## Design foundation
Baseline values come from the current rendered app and source, not the stale Figma screen. Existing base palette: background #060608, surface #0d0e11, raised #15161a, hover #1b1d22, foreground #f4f1ea, primary #5eead4. Preserve current layout and typography families. Resolve reusable Figma components, variables, and styles before constructing equivalents.

## Verification
Compare the editable baseline with a browser-rendered reference at identical viewport dimensions. Compare baseline and reskin geometry, visible text, and controls. Inspect larger text for clipping and overlap, especially footer, card actions, badges, and dialogs. Confirm semantic editable layers rather than a complete-screen raster. Report any unavailable desktop-only state or verification limitation honestly.

## Builder discretion
Choose restrained surface, border, and weight refinements within the existing palette. Determine exact text sizes from measured space, targeting 12–13px for frequent reading without breaking frozen geometry. Flag any unresolved readability-versus-fit conflict to the user.
