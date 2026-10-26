# /visual-check

Purpose: Compare a running screen against its Figma reference without unnecessarily changing application code.

Workflow:
1. Capture the Figma reference via figma-inspect skill (screenshot + design context).
2. Start the application (Vite dev server on port 1420, or Tauri dev if native APIs required).
3. Capture the rendered UI screenshot via Playwright MCP at matching viewport dimensions.
4. Compare reference and implementation via visual-qa skill.
5. Generate a prioritized discrepancy report.
6. Optionally implement corrections only when the user requests them.

## Usage

```
/visual-check <screen_name> <figma_url_or_node_description>
```

## Arguments

$ARGUMENTS — Screen name to check, followed by Figma URL or node description for comparison.

## Required Skills

- figma-inspect
- visual-qa

## Output

A prioritized discrepancy report with:
- Layout geometry mismatches
- Typography and spacing differences
- Color, border, shadow, radius differences
- Asset fidelity issues
- Remaining unknowns

Corrections are NOT applied automatically. The user must request implementation of any fixes.
