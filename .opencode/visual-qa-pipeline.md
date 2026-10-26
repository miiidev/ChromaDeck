# ChromaDeck Visual Verification Pipeline

This document describes the end-to-end visual verification pipeline for comparing ChromaDeck's rendered UI against Figma references.

## Architecture

```
Figma MCP → figma_get_design_context → Reference screenshot
                    ↓
Vite dev server (port 1420) or Tauri dev → Running application
                    ↓
Playwright MCP (chrome) → Application screenshot at matching viewport
                    ↓
Visual QA comparison → Discrepancy report
                    ↓
Iterative correction → Re-capture → Re-compare
```

## Pipeline Steps

### Step 1: Figma Reference Capture

**Tool:** `figma_get_design_context` + `figma_get_screenshot`

**Inputs:**
- `fileKey` — extracted from Figma URL
- `nodeId` — extracted from Figma URL `node-id` parameter (convert `-` to `:`)
- `clientLanguages`: `"typescript,html,css"`
- `clientFrameworks`: `"react,tailwindcss"`

**Output:** Reference code, screenshot URL, contextual metadata, design tokens.

**Failure mode:** If MCP is unavailable or fileKey/nodeId is invalid, document as blocked and proceed with code review only.

### Step 2: Application Startup

**Browser-testable frontend (no Tauri APIs):**
```bash
npm run dev
# Serves on http://localhost:1420
```

**Tauri-required features:**
```bash
npm run tauri dev
# Opens Tauri window; DevTools accessible via F12
```

**Note:** Playwright cannot connect to Tauri WebView2 via `http://localhost:1420` (connection refused). For browser-based screenshot capture, use the Vite dev server directly (`npm run dev`) when the screen does not require Tauri APIs. For screens requiring Tauri APIs, capture via Chrome DevTools (F12) manually in the running Tauri window.

### Step 3: Application Screenshot Capture

**Tool:** Playwright MCP (chrome browser)

**Viewport consistency:** Match Figma reference dimensions exactly. ChromaDeck default window: 960x680 (from tauri.conf.json).

**State control:**
- Scroll position: top (0,0) unless testing scroll behavior
- Animation state: reduced-motion off, entrance animations complete
- Dialog/modal state: closed unless testing modal
- Dynamic content: use consistent test data

### Step 4: Screenshot Comparison

**Comparison priorities:**
1. Page structure and geometry
2. Container sizes and alignment
3. Typography and line wrapping
4. Spacing and component sizing
5. Colors, borders, shadows, radii
6. Icons, imagery, fine details

**Acceptable differences (not errors):**
- Font antialiasing differences
- OS-level rendering variations
- Animation frame timing
- Dynamic content differences

**Comparison tools:**
- Playwright screenshot diff (when available)
- Manual inspection of screenshots side by side
- Overlay/difference images when helpful

### Step 5: Discrepancy Report

**Format:**
```
VISUAL QA REPORT
- figma_reference: <url or nodeId>
- app_url: <url>
- viewport: WxH
- discrepancies: [ { severity, element, expected, actual, correction } ]
- matched: [ element, ... ]
- visual_fidelity: pass/fail
```

### Step 6: Iterative Correction

1. Implementation engineer applies corrections based on discrepancy report.
2. Re-capture application screenshot.
3. Re-compare against Figma reference.
4. Repeat until visual_fidelity = pass or remaining discrepancies are documented as known limitations.

### Step 7: Regression Checks

Run `ui-regression` skill:
- `npm run build` passes
- `npm run test` passes
- Shared components verified across all screens
- Variant states verified

## Artifact Storage

Screenshots and comparison artifacts are stored in:
```
.opencode/qa-artifacts/
  figma-reference-<nodeId>.png
  app-screenshot-<screen>-<timestamp>.png
  diff-<screen>-<timestamp>.png
```

Do not commit large temporary files to git.

## Known Limitations

| Limitation | Impact | Mitigation |
|------------|--------|------------|
| Playwright cannot connect to Tauri WebView2 (`ERR_CONNECTION_REFUSED`) | Cannot capture screenshots from Tauri window via MCP | Use Vite dev server directly for non-Tauri screens; use Chrome DevTools manually for Tauri screens |
| Vitest runs in node environment (no jsdom) | Cannot run component rendering tests in CI | Visual QA must be manual via browser |
| Figma MCP may be rate-limited or unavailable | Cannot retrieve design context | Document as blocked; proceed with code review |
| No image-diff tool installed | Cannot generate automated difference images | Manual visual comparison via screenshots |
