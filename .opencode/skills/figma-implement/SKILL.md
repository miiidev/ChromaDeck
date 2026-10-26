# Skill: figma-implement

## Purpose

Trigger when implementing a Figma design in the ChromaDeck codebase. Maps Figma components to existing code components, applies design tokens, implements faithful layout, and preserves existing behavior.

## Trigger

Task mentions:
- Implementing a Figma design in code
- "Figma-implement", "implement design", "code from Figma"
- Visual changes following a figma-inspect specification

## Prerequisite

A completed figma-inspect design specification must exist before invoking this skill. If no specification exists, run figma-inspect first.

## Procedure

1. **Load design specification.** Read the figma-inspect output (fileKey, nodeId, screenshot, tokens, geometry, typography, motion, variants, assets, unknowns).

2. **Inspect repository.** Read affected files and their Figma references:
   - `src/App.css` — design tokens (`@theme` block)
   - `src/components/ui/` — base UI components (Button, Badge, Card, Dialog, Input, Select, Checkbox, Radio, Slider)
   - `DESIGN.md` — documented design system (compare against Figma values; Figma wins on conflicts)
   - `components.json` — shadcn base-nova conventions

3. **Map Figma components to code components.** Use `figma_get_code_connect_suggestions` if available to find code component matches. Otherwise inspect `src/components/ui/` for:
   - `data-slot` attribute matches
   - Component name matches (Button, Badge, Card, Dialog, etc.)
   - `cva` variant matches

4. **Apply design tokens.** Map Figma values to CSS custom properties in `src/App.css` `@theme` block:
   - Colors → `--color-*`
   - Radii → `--radius-*`
   - Shadows → `--shadow-*`
   - Typography → `--font-*`, type scale classes
   - Motion → `--motion-*`, `--ease-*` tokens

5. **Implement layout faithfully.** Do not simplify into generic cards/buttons/layouts when Figma specifies otherwise:
   - Match exact column counts, gap values, padding
   - Match `data-slot` API conventions (`data-slot="button"`, `data-slot="card"`, etc.)
   - Match `cva` variant patterns
   - Use Base UI React primitives for interactive components
   - Use Radix UI Slider for slider components
   - Use lucide-react for icons

6. **Preserve existing behavior.** Do not change:
   - User flows, navigation, state management
   - Tauri API integrations (`@tauri-apps/api`)
   - React component logic, event handlers
   - Existing component architecture

7. **Handle unknowns.** If a Figma value was marked `unknown` in the specification, do not invent it. Use the nearest existing design token or report as missing.

8. **Verify build.** Run `npm run build` (tsc + vite build) to confirm no TypeScript or CSS syntax errors.

## Output Format

```
FIGMA IMPLEMENTATION
- specification_source: <figma-inspect reference>
- files_modified: [ path, ... ]
- tokens_added: [ token_name: value, ... ]
- components_updated: [ path, ... ]
- unknowns_left: [ item, ... ]
- build_passed: true/false
```

## Do Not

- Do not simplify designs into generic cards/buttons/layouts.
- Do not change business logic, user flows, or Tauri integrations.
- Do not claim visual accuracy without running visual-qa.
