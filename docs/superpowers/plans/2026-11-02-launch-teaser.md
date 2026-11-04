# ChromaDeck Launch Teaser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and render the approved 30-second offline portrait launch teaser.

**Architecture:** An isolated launch-teaser/ package owns the Remotion entrypoint, reusable components, three scene batches, configuration, scripts, and output. Frame-based deterministic state derives entirely from centralized timing and layout configuration; no application changes.

**Tech Stack:** React, TypeScript, Remotion, @fontsource/geist, @fontsource/jetbrains-mono, FFmpeg.

**Spec:** docs/superpowers/specs/2026-11-02-launch-teaser-design.md and DESIGN.md, plus the user's full scene specification.

## Global Constraints
- 1080x1920, 30 fps, 900 frames, H.264, CRF 16.
- Exact DESIGN.md tokens; no gradients, blur, glow, shadows, glass, external rendering requests, or invented product claims.
- Transform/opacity animation only; specified bezier curves and stagger limits.
- All copy/colors/tag colors/timings/assets/layout in launch-teaser/src/config.ts.
- Effective text >=48px; safe zone 80px sides and 250px top/bottom.
- Geist 400/600/700 and JetBrains Mono 500/700 from local fontsource packages.
- Do not overwrite user changes; no commits without request.

## Review Focus
- Final URL fits the safe width at >=48px; inspect final-frame still.
- Boundary frames do not accidentally blank or overlap scenes; test contiguous 900-frame timeline.
- Drag/drop shows source dim, ghost, valid target, spinner, sweep, filled slot, badge and source exit; inspect state stills.
- Local font assets load without network at render time; inspect glyphs and bundle source.
- Landscape adaptation uses layout config, not portrait positions embedded in scenes; typecheck a second composition if supplied.

---

### Task 1: Isolated scaffold, configuration and primitives
**Files:** launch-teaser/package.json, package-lock.json, tsconfig.json, remotion.config.ts, src/index.ts, src/Root.tsx, src/config.ts, src/fonts.css, src/motion.ts, src/components/*, tests/config.test.*.
**Interfaces:** Config exports dimensions, fps, duration, tokens, copy, scene frame ranges, layout, asset paths, audio cues, preset data. Components consume config and local frame values; root registers ChromaDeckLaunch.
- [ ] Scaffold with create-video noninteractive if supported; remove template content, otherwise document CLI limitation and use an equivalent minimal package.
- [ ] Write and run initial failing tests for 900 contiguous frames, exact token values and final URL.
- [ ] Implement config and offline font loading, Canvas, KineticText/ghost echoes, LogoMark, MintRing, PresetCard, MonitorCard, DragGhost, Cursor, FooterBar, IdentifyOverlay and FlashFill.
- [ ] Run tests and TypeScript check; ensure matching config contracts.

### Task 2: Hooks and reveal
**Files:** launch-teaser/src/scenes/HookReveal.tsx, composition wiring, out/stills/batch-1-*.
**Interfaces:** HookReveal consumes config scene ranges and reusable components for 0–10 seconds.
- [ ] Implement hooks, cycling markers, Meet outlined echo, flat underline, mint ring, staggered C assembly and version chip.
- [ ] Render stills using npx remotion still at frames 30, 90, 135, 195, 270 and inspect them.
- [ ] Fix clipping, unsafe text, incorrect tokens or missing fonts before next batch.

### Task 3: Values, carousel and drag/drop
**Files:** launch-teaser/src/scenes/ValuesProduct.tsx, out/stills/batch-2-*.
**Interfaces:** ValuesProduct renders 10–20.5 seconds, using configured preset identities, cursor targets and state cue frames.
- [ ] Implement monitor outlines/value copy, word staggers, one-card carousel step, flat stepped-opacity edge treatment, and complete drag/drop state sequence.
- [ ] Keep important card details inside safe zone; peripheral neighboring cards may crop.
- [ ] Render and inspect stills at frames 330, 390, 465, 540, 565, 600; fix failures.

### Task 4: Footer, Identify, closing and audio/captions
**Files:** launch-teaser/src/scenes/Finale.tsx, scripts/generate-assets.*, public/audio/*, out/captions.srt, out/stills/batch-3-*.
**Interfaces:** Finale renders 20.5–30 seconds. Caption and audio generation consume config cue timestamps.
- [ ] Implement footer counts, checked Autostart, button press and one health ring, two Identify numbers, closing, mint flash and end card.
- [ ] Generate optional local 30-second pad/riser/impact/ticks with FFmpeg, and SRT from configured copy/timings.
- [ ] Render and inspect frames 645, 705, 735, 780, 825, 861, 894, especially URL width and 48px minimum.

### Task 5: Render and final verification
**Files:** launch-teaser/README.md, out/chromadeck-launch.mp4, out/contact-sheet.jpg, verification report.
- [ ] Run package tests and TypeScript check, existing app npm run build and npm run test; record exact results.
- [ ] Review code for centralized values, forbidden visual styles, deterministic rendering and the requested states.
- [ ] Run npx remotion render ChromaDeckLaunch out/chromadeck-launch.mp4 --codec=h264 --crf=16.
- [ ] Run ffprobe to assert h264, 1080x1920, 30 fps, 30-second duration and 900 frames.
- [ ] Extract 1 frame per 2 seconds with FFmpeg into a contact sheet; view against timeline and fix/re-render mismatches.
- [ ] README documents config edits, studio preview, render command, outputs, offline font/audio behavior, a landscape composition adaptation, and assumptions/TODOs.
- [ ] Final independent adequate-cost review; record any remaining limitations honestly.
