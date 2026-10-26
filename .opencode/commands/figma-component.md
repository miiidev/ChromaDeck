# /figma-component

Purpose: Implement or update a specific component based on its Figma counterpart.

Workflow:
1. Inspect the reference component and its variants via figma-inspect skill.
2. Find the corresponding code component in `src/components/ui/`.
3. Identify shared dependencies and affected screens.
4. Implement the change via figma-implement skill, preserving existing behavior.
5. Verify the component and affected screens via visual-qa skill.
6. Run regression checks via ui-regression skill on affected screens.

## Usage

```
/figma-component <component_name> <figma_url_or_node_description>
```

## Arguments

$ARGUMENTS — Component name (e.g., "Button", "PresetCard", "MonitorList") followed by Figma URL or node description.

## Required Skills

- figma-inspect
- figma-implement
- visual-qa
- ui-regression

## Acceptance Criteria

- Component matches Figma variant specifications
- Build passes (`npm run build`)
- Visual fidelity confirmed on component and all screens using it
- No regressions in variant states (default, outline, destructive, pinned, etc.)
