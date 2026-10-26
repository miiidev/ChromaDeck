# /design-audit

Purpose: Audit a screen or collection of screens for inconsistencies with Figma and the shared design system.

Workflow:
1. Inspect the target screen(s) via figma-inspect skill.
2. Compare against current codebase implementation (App.css tokens, component variants, DESIGN.md).
3. Report layout discrepancies, token inconsistencies, typography mismatches, missing/incorrect assets, component inconsistencies, responsive issues, and accessibility concerns.
4. Prioritize issues by visual impact.
5. Do not implement corrections unless requested.

## Usage

```
/design-audit <screen_name_or_scope> <figma_url_or_node_description>
```

## Arguments

$ARGUMENTS — Screen name or scope (e.g., "PresetCard", "MonitorDashboard", "all screens"), followed by Figma URL or node description.

## Required Skills

- figma-inspect
- visual-qa

## Output

A design audit report with:
- Token mismatches (code vs Figma values)
- Typography deviations
- Component variant inconsistencies
- Missing or incorrect assets
- Layout/responsive issues
- Accessibility concerns (contrast, focus indicators)
- Recommended corrections

Corrections are NOT applied automatically.
