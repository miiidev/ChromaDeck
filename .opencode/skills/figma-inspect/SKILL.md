# Skill: figma-inspect

## Purpose

Trigger when a task references Figma or requires implementation from a design. Retrieves actual Figma design context, screenshots, component details, and assets through the available Figma MCP tools, then produces a structured design specification for implementation.

## Trigger

Task mentions:
- A Figma URL or file key
- "Figma", "design", "reskin", "match Figma", "from Figma", "design reference"
- Visual appearance changes to UI components or screens

## Procedure

1. **Resolve Figma target.** Extract `fileKey` and `nodeId` from any Figma URL provided. If no nodeId is present, ask the user for a node-specific URL before proceeding. Do not guess node IDs.
   - URL format: `https://figma.com/design/:fileKey/:fileName?node-id=:int1-:int2`
   - Extract `fileKey` from the path, `nodeId` from `node-id` parameter (convert `-` to `:`)

2. **Load design-to-code guidance.** Before calling `get_design_context`, load the `/figma-design-to-code` skill if available; otherwise read `skill://figma/figma-design-to-code/SKILL.md`.

3. **Retrieve design context.** Call `figma_get_design_context` with `nodeId`, `fileKey`, `clientLanguages: "typescript,html,css"`, `clientFrameworks: "react,tailwindcss"`. Include screenshot unless explicitly excluded.

4. **Retrieve related assets.** If the context references images, icons, or fonts, call `figma_download_assets` to fetch them with correct file extensions.

5. **Inspect component variants.** If the target is a component set or has variants, call `figma_get_context_for_code_connect` to enumerate variant options and property definitions.

6. **Get variable definitions.** Call `figma_get_variable_defs` for the node to extract applied color, spacing, typography, and motion variables.

7. **Search design system.** Call `figma_search_design_system` to find matching components, variables, and styles in connected libraries (buttons, cards, tokens, etc.).

8. **Handle large/truncated frames.** If context is truncated, inspect smaller sections or nested components individually rather than approximating.

9. **Produce design specification.** Output:
   - Node ID, file key, screenshot URL
   - Color tokens (hex values, roles)
   - Geometry (dimensions, radii, borders, shadows)
   - Typography (font family, sizes, weights, tracking)
   - Motion/easing values
   - Component variants and their property mappings
   - Asset URLs (images, icons, SVGs)
   - List of unknowns (values not retrievable from Figma)

10. **Report unknowns explicitly.** Never invent missing Figma values. Mark them as `unknown` in the specification.

## Output Format

```
FIGMA SPECIFICATION
- fileKey: <value>
- nodeId: <value>
- screenshot: <url>
- colors: { role: #hex, ... }
- geometry: { width, height, radius, border, shadow, ... }
- typography: { family, size, weight, tracking, ... }
- motion: { duration, easing, ... }
- variants: { variant_name: { properties }, ... }
- assets: [ { type, url, format }, ... ]
- unknowns: [ item, ... ]
- corresponding_code_components: [ path, ... ]
```

## Do Not

- Do not approximate design values from screenshots alone when MCP tools can retrieve them.
- Do not guess node IDs or file keys.
- Do not proceed to implementation without a completed specification.
