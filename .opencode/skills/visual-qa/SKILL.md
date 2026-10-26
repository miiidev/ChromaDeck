# Skill: visual-qa

## Purpose

Trigger after implementing or modifying a visual component. Captures the Figma reference and running application screenshots, compares them, and produces a prioritized discrepancy report with actionable corrections.

## Trigger

Task mentions:
- "visual-qa", "visual QA", "compare Figma", "check visual", "verify visual"
- After figma-implement completes
- Before declaring visual work complete

## Procedure

1. **Capture Figma reference.** Use `figma_get_screenshot` or the screenshot from figma-inspect context for the target node. Record dimensions and viewport.

2. **Start the application.**
   - For browser-testable frontend: start Vite dev server (`npm run dev`) on port 1420.
   - For Tauri-required features: start `npm run tauri dev` and open Chrome DevTools (F12) in the running window.
   - Record the URL and viewport used.

3. **Capture application screenshot.** Use Playwright MCP (chrome browser) to navigate to the target screen and capture a screenshot at the same viewport dimensions as the Figma reference.
   - Set consistent device scale factor, scroll position, and animation state.
   - Establish repeatable state (mock data, closed dialogs, etc.).

4. **Compare reference and implementation.**
   - Compare layout geometry, alignment, typography, spacing, colors, component dimensions, clipping, asset fidelity.
   - Use image-difference tooling when available (Playwright screenshot diff).
   - Prioritize: page structure → container sizes → typography → spacing → colors/borders/shadows → icons/assets.

5. **Generate discrepancy report.** List highest-impact mismatches first with specific correction targets.

6. **Iterate.** After corrections, re-capture screenshots and re-compare. Verify the corrected result — do not assume changes worked.

7. **Account for legitimate differences:** font antialiasing, dynamic content, OS rendering, animation state. Do not treat these as design errors.

## Output Format

```
VISUAL QA REPORT
- figma_reference: <url or nodeId>
- app_url: <url>
- viewport: WxH
- discrepancies: [ { severity, element, expected, actual, correction }, ... ]
- matched: [ element, ... ]
- remaining_unknowns: [ item, ... ]
- visual_fidelity: pass/fail
```

## Do Not

- Do not treat successful build or passing tests as visual fidelity evidence.
- Do not compare unrelated states or differently sized screenshots.
- Do not claim visual accuracy without actual comparison.
