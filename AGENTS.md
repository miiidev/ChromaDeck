# ChromaDeck — Agent Instructions

This file establishes the mandatory rules for all agents working on ChromaDeck. It is read by OpenCode at session start.

## Design Authority

- **Figma is authoritative for appearance.** The live Figma file key is `vfF5g0hTALNbScBM4Nz1Y1`.
- **The existing application is authoritative for behavior** unless the user explicitly requests functional changes.
- A reskin changes visual presentation only — it does not change user flows, navigation, state, or Tauri integrations.
- Functional changes must be separately authorized by the user.

## Implementation Discipline

1. **Inspect before editing.** Never modify a component without first reading it and its Figma reference.
2. **Use structured Figma context and reference screenshots.** Retrieve actual design values via Figma MCP tools; do not approximate from memory.
3. **Prefer real assets and actual design tokens.** Map Figma colors, radii, shadows, typography to CSS custom properties in `src/App.css`.
4. **Preserve existing component and application architecture.** Follow `base-nova` conventions (`data-slot` API, `cva` variants, Base UI React primitives).
5. **Avoid arbitrary values when actual design values are available.** Use CSS variables, never magic numbers.
6. **Do not invent missing requirements.** If a Figma value is unavailable, report it as unknown rather than guessing.
7. **Avoid unrelated refactors.** Scope changes tightly to the requested design target.
8. **Do not overwrite user changes.** Respect existing working-tree modifications.
9. **Do not claim tests or tools were run when they were not.** Report honestly which checks passed and which could not run.

## Visual Acceptance

Every design implementation must have a corresponding visual reference and a verification procedure.

A task cannot be declared visually complete until:
- The rendered UI has been compared against the Figma reference (via screenshot diff or direct inspection), **or**
- The report explicitly explains why visual verification could not be done (e.g., no Figma link provided, MCP unavailable, application cannot launch).

## Scope

- `/figma-reskin` — page-level visual reskin using Figma as source of truth.
- `/figma-component` — component-level visual update using Figma as source of truth.
- `/visual-check` — compare running UI against Figma reference without code changes.
- `/design-audit` — report inconsistencies between Figma and codebase design system.

## Verification

Run these before declaring completion:
- `npm run build` (tsc + vite build)
- `npm run test` (vitest)
- Visual QA: capture running UI, compare against Figma reference

## Project Stack

| Layer | Technology |
|-------|-----------|
| Framework | React 19.1 + TypeScript 6 |
| Build tool | Vite 8 |
| Styling | Tailwind CSS v4 (`src/App.css` `@theme` block) |
| UI primitives | Base UI React, Radix UI Slider |
| Icons | lucide-react |
| Desktop shell | Tauri v2 (dev server port 1420) |
| Testing | Vitest (node environment) |
| Design tokens | CSS custom properties via `@theme` in `src/App.css` |
| Component conventions | shadcn `base-nova` (`data-slot`, `cva`) |
