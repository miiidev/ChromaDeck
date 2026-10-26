# /figma-reskin

Purpose: Reskin an existing screen or set of screens using Figma as the visual source of truth.

Workflow:
1. Resolve the target Figma links and screens. If ambiguous, ask the user for clarification instead of selecting a random frame.
2. Inspect the current application state and affected files.
3. Analyze the Figma design via figma-inspect skill to produce a design specification.
4. Produce an implementation checklist from the specification.
5. Implement the design via figma-implement skill, preserving existing behavior and architecture.
6. Run visual QA via visual-qa skill; capture reference and rendered UI, compare, and correct discrepancies iteratively.
7. Run functional regression checks via ui-regression skill; verify build, tests, shared components, variants.
8. Report results: build status, test status, visual fidelity, remaining discrepancies.

## Usage

```
/figma-reskin <figma_url_or_node_description>
```

## Arguments

$ARGUMENTS — Figma URL, file key, node description, or screen name to reskin.

## Required Skills

- figma-inspect
- figma-implement
- visual-qa
- ui-regression

## Acceptance Criteria

- Build passes (`npm run build`)
- Tests pass (`npm run test`)
- Visual fidelity confirmed against Figma reference
- No functional regressions in shared components
