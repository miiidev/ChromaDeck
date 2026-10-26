# ChromaDeck Figma-to-Code Agent System

## Overview

This system enables reusable, reliable Figma-to-code workflows for ChromaDeck using Figma as the visual source of truth while preserving the existing codebase as the source of truth for application behavior.

## Agent Architecture

Five specialized agents coordinate through the Design Orchestrator:

| Agent | Role | File |
|-------|------|------|
| Design Orchestrator | Coordinates workflow, delegates tasks, enforces visual QA gate | `.opencode/agents/design-orchestrator.md` |
| Figma Design Analyst | Retrieves Figma design context, assets, tokens via MCP | `.opencode/agents/figma-design-analyst.md` |
| Frontend Implementation Engineer | Implements designs in React/Tailwind codebase | `.opencode/agents/frontend-implementation-engineer.md` |
| Visual QA Engineer | Captures and compares Figma vs rendered UI screenshots | `.opencode/agents/visual-qa-engineer.md` |
| Functional Regression Tester | Runs build/tests, checks for regressions | `.opencode/agents/functional-regression-tester.md` |

## Skills

| Skill | Trigger | File |
|-------|---------|------|
| figma-inspect | Task references Figma or needs design implementation | `.opencode/skills/figma-inspect/SKILL.md` |
| figma-implement | Implementing a Figma design in code | `.opencode/skills/figma-implement/SKILL.md` |
| visual-qa | After implementing/modifying a visual component | `.opencode/skills/visual-qa/SKILL.md` |
| ui-regression | Changes affect shared components/layouts/interactions | `.opencode/skills/ui-regression/SKILL.md` |

## Commands

| Command | Purpose | File |
|---------|---------|------|
| `/figma-reskin` | Page-level visual reskin from Figma | `.opencode/commands/figma-reskin.md` |
| `/figma-component` | Component-level visual update from Figma | `.opencode/commands/figma-component.md` |
| `/visual-check` | Compare running UI against Figma reference | `.opencode/commands/visual-check.md` |
| `/design-audit` | Audit screens for Figma/design system inconsistencies | `.opencode/commands/design-audit.md` |

## Configuration

### OpenCode Registration

Project skills path added to `~/.config/opencode/opencode.json`:
```json
"skills": {
  "paths": [
    "../skills",
    "D:/Syahmi/MyCode/ChromaDeck/.opencode/skills"
  ]
}
```

Project commands and agents are defined as files in `.opencode/commands/` and `.opencode/agents/`. To make them invocable as OpenCode commands/agents, add them to `~/.config/opencode/opencode.json` following the existing inline definition pattern.

### Project Instructions

`AGENTS.md` at project root establishes design authority, implementation discipline, visual acceptance, scope, and verification rules.

## Visual Verification Pipeline

Documented in `.opencode/visual-qa-pipeline.md`. Pipeline:

1. Figma MCP retrieves reference frame and screenshot
2. Frontend started (Vite dev on port 1420, or Tauri dev for native APIs)
3. Playwright MCP captures application screenshot at matching viewport
4. Reference vs implementation compared
5. Discrepancy report generated with prioritized corrections
6. Iterative correction until visual fidelity confirmed
7. Regression checks run (build, tests, shared components)

### Known Limitations

| Limitation | Impact | Mitigation |
|------------|--------|------------|
| Playwright cannot connect to Tauri WebView2 | Cannot capture Tauri window screenshots via MCP | Use Vite dev server directly for non-Tauri screens; Chrome DevTools manually for Tauri screens |
| Vitest node environment (no jsdom) | No component rendering tests in CI | Visual QA must be manual via browser |
| Figma MCP requires selected layer | Cannot inspect nodes without nodeId | Ask user for specific Figma frame link |
| No image-diff tool installed | No automated difference images | Manual visual comparison via screenshots |

## Design Token / Component Mapping

Documented in `.opencode/token-mapping.md`. Maps Figma color/geometry/motion/typography tokens to `src/App.css` `@theme` block, and Figma components to code components in `src/components/ui/`.

## Required MCP Connections

- **Figma** — for design context, screenshots, assets (connected, authenticated as miiidev)
- **Playwright** — for browser automation and screenshot capture (configured in opencode.json, chrome browser)
- **Supabase** — configured but not used by this system

## Verification Commands

```bash
npm run build      # tsc + vite build
npm run test       # vitest
npm run dev        # Vite dev server on port 1420
npm run tauri dev  # Tauri desktop app
```

## Pilot Test

The pilot test requires a specific Figma frame link (URL with node-id parameter). The Figma file key is `vfF5g0hTALNbScBM4Nz1Y1`. Provide a node-specific URL to run the pilot on a specific screen or component.

## Known Issues

- Working tree has uncommitted changes (App.tsx, MonitorList.tsx, MonitorSidebar.tsx, PresetCard.tsx, main.tsx, ErrorBoundary.tsx, temp_orig.css)
- DESIGN.md documents light theme; Figma is dark theme — convergence needed
