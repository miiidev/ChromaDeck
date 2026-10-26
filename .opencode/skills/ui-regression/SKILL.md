# Skill: ui-regression

## Purpose

Trigger when changes affect shared components, layouts, routing, or interactions. Verifies that visual redesigns do not break existing ChromaDeck functionality.

## Trigger

Task mentions:
- "ui-regression", "regression", "check for regressions", "functional regression"
- After visual changes to shared components
- Before declaring visual work complete

## Procedure

1. **Identify affected behavior.** List existing functionality that could be impacted:
   - Navigation between screens
   - Preset operations (create, apply, delete, pin)
   - Monitor connections
   - Dialog interactions (open, close, submit)
   - Form inputs and sliders
   - Tauri API calls (`listMonitors`, `listPresets`, `listPins`, `listApplied`)
   - Window show/hide and entrance replay

2. **Run type check and build.** Execute `npm run build` (tsc + vite build). Report pass/fail.

3. **Run available tests.** Execute `npm run test` (vitest). Report which tests ran and which failed.

4. **Check shared components.** For each shared component modified (Button, Badge, Card, Dialog, Input, Slider, StatTile, etc.):
   - Verify all screens using the component still render correctly
   - Verify variant states (default, outline, destructive, pinned, etc.)
   - Verify interactive states (hover, focus, disabled, active)

5. **Check layout overflow and responsiveness.** Verify no clipping, overlap, or broken responsive behavior on the 960x680 default window size.

6. **Report results.** List which checks ran, which passed, which failed, and which remain unverified.

7. **Do not modify business logic** to make tests pass. Escalate implementation issues to the orchestrator.

## Output Format

```
UI REGRESSION REPORT
- changes_scoped: [ component, ... ]
- build: pass/fail
- tests: pass/fail (details)
- screens_checked: [ path, ... ]
- variants_verified: [ variant, ... ]
- regressions_found: [ description, ... ]
- unverified: [ item, ... ]
```

## Do Not

- Do not modify unrelated business logic to make tests pass.
- Do not claim regression-free status without running build and tests.
