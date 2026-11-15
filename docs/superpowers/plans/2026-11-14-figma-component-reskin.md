# Figma Component Reskin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve an editable current-app baseline in Figma and produce a separate restrained reskin.

**Architecture:** Read current React components and browser-rendered geometry; reuse existing Figma primitives where they match. Keep baseline and reskin independent so new styling cannot modify the preserved baseline or older screens.

**Tech Stack:** React 19, Tailwind 4, Vite 8, Playwright, Figma MCP.

**Spec:** `docs/superpowers/specs/2026-11-14-figma-component-reskin-design.md`

## Global Constraints
- Figma-only visual changes; do not edit application source or existing working-tree changes.
- Target file: `vfF5g0hTALNbScBM4Nz1Y1`.
- Preserve layout, positions, component footprints, spacing, content, controls, and behavior.
- Single outer card border; remove decorative nested border, not meaningful input/drop-target boundaries.
- Target 12–13px frequently read text where measured bounds permit; report fit conflicts.
- Preserve dark/mint identity, Geist/JetBrains Mono, assets, and all semantic states.

## Review Focus
- Long preset/monitor names must retain current truncation without overlap.
- Footer text enlargement must not displace or hide controls.
- Pinned/applied states must remain distinguishable after removing nested borders.
- Dialog labels and values must not collide after text enlargement.
- Baseline and old screens must not change through shared reskin component mutations.

---

### Task 1: Discover current rendering and Figma assets
**Files:** Read `src/App.tsx`, `src/App.css`, `src/components/`, `src/lib/tauri.ts`, existing Playwright fixtures, `DESIGN.md`; no source edits.
**Interfaces:** Produces viewport, visible state fixtures, source component inventory, measured geometry, and Figma asset map.
- [ ] Read relevant components, defaults, desktop browser fallback, and existing fixtures.
- [ ] Load Figma use/generate-design guidance; create/reuse task session; search design system first and inspect available libraries, current screens, variables, and styles.
- [ ] Render app using existing tooling without altering source. Use existing browser fixtures only when necessary and identify mocked states.
- [ ] Capture source screenshot and geometry at the actual desktop viewport; inventory main view, editor, apply and confirmation dialogs, and updater.

### Task 2: Transfer faithful baseline
**Files:** Figma new baseline page/frames only.
**Interfaces:** Consumes Task 1 inventory; produces baseline node IDs and verification screenshot.
- [ ] Build editable current UI using applicable component instances and bound tokens, with source SVG assets. Use required live capture as visual reference alongside semantic construction.
- [ ] Verify exact viewport, major section/card/control coordinates, visible labels, font families, and long-name behavior against source.
- [ ] Inspect layer types and image fills; reject complete-screen raster deliverables. Remove temporary capture after comparison and asset transfer.

### Task 3: Create restrained reskin and verify
**Files:** Figma separate reskin page and independent reusable components only.
**Interfaces:** Consumes baseline node IDs; produces reskin links and before/after verification.
- [ ] Duplicate baseline without allowing shared main-component changes to affect it.
- [ ] Remove inner decorative card border; preserve outer border and meaningful slot/input outlines. Keep tag-coloured applied/pinned states.
- [ ] Enlarge small reading text toward 12–13px while measuring fit; retain large titles. Apply restrained visual refinements to remaining controls without geometry changes.
- [ ] Compare baseline/reskin bounds and control inventory; inspect footer fit, long names, card actions, badges, and dialog label/value collisions. Report any text that cannot be enlarged under frozen geometry.
- [ ] Validate editability, font families, and state treatments; inspect full-view screenshots and correct concrete defects.
- [ ] Deliver Figma links and exact scope/limitations. No application build/test claims: application source is unchanged; visual verification is the relevant gate.
