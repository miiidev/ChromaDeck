---
target: ChromaDeck color palette
total_score: 27
p0_count: 0
p1_count: 2
timestamp: 2026-10-06T14-24-28Z
slug: chromadeck-color-palette
---
# Critique: ChromaDeck color palette (slop check)

Scope: palette and color identity of the ChromaDeck desktop tool UI (tokens in `src/App.css`, usage in `src/components/`). Register: product.

## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 3 | IN USE badge + shape-coded dots solid; session-applied state lost on restart |
| 2 | Match System / Real World | 3 | NVCP vocabulary fluent for pros; opaque to outsiders (acceptable: audience is pros) |
| 3 | User Control and Freedom | 3 | Cancel/delete-confirm/reset present; no undo of an apply |
| 4 | Consistency and Standards | 3 | Fully tokenized except two hardcoded status dots |
| 5 | Error Prevention | 3 | Inline validation, delete confirm, stated neutral points |
| 6 | Recognition Rather Than Recall | 2 | Icon-only card buttons (tooltip/aria only, no visible labels) |
| 7 | Flexibility and Efficiency | 2 | No keyboard shortcuts or bulk actions in a pro tool |
| 8 | Aesthetic and Minimalist Design | 2 | Stock shadcn tokens; identical card grid; uppercase-everywhere label grammar |
| 9 | Error Recovery | 3 | Inline errors near fields; reset path always visible |
| 10 | Help and Documentation | 1 | No in-app help; README only |
| **Total** | | **27/40** | **Acceptable** |

## Anti-Patterns Verdict: does this look AI-generated?

**LLM assessment**: Half-guilty. The token set is stock shadcn `base-nova` dark (`#08090F`, cyan `#22D3EE`, violet `#7C3AED`, pink `#EC4899`), which is one of the most common AI-output clusters of 2026, and the card grid + pill badges + uppercase tracked labels match the familiar scaffold. What saves it: zero gradient/glass/blob decoration, a genuinely committed single accent (cyan owns primary actions, not a purple wash), and shape-coded status (circle/square/triangle), which is tool-like thinking, not slop. Verdict: reads as competent default, not distinctive. Not dead, but forgettable.

**Deterministic scan**: 2 warnings, both `overused-font` (Inter in `src/App.css` lines 41, 53). No color, gradient, glass, or eyebrow hits. The font hit is a partial false positive: the product register permits familiar sans, and a calibration tool has no business in a display face. No browser overlay (no automation in this harness); contrast ratios measured manually as fallback signal.

## Overall Impression

A well-built instrument wearing off-the-rack clothes. The structure and state language are thoughtful; the palette is where the AI smell lives, plus two genuine contrast failures the stock theme ships with.

## What's Working

1. **Single committed accent.** Cyan means action/selection/status everywhere and nothing else. Per the product register, accent-only usage is exactly right, and it is already honored.
2. **Shape-first status.** Connected (circle), pinned (square), warning (triangle) plus text labels mean state survives color blindness and dim rooms. This is the most distinctive, non-slop idea in the UI. Lean into it.
3. **Full tokenization.** One grep finds every color in the app (two hardcoded dots excepted). Any palette fix is a token edit, not archaeology.

## Priority Issues

- **[P1] Primary button text fails contrast (1.81:1).** White `#FFFFFF` on cyan `#22D3EE` (APPLY buttons). Nearly unreadable by the metric; small bold caps survive only by stroke weight. Why it matters: the single most-clicked action in a calibration tool is low-legibility. Fix: `--color-primary-foreground` to a deep cyan-black (e.g. `#062B33`, ratio ~10:1). Suggested command: direct token edit, then `/uizze polish`.
- **[P1] PINNED badge text fails contrast (3.25:1).** `text-accent` violet `#7C3AED` on card `#10131D` at 12px. Why it matters: the enforced-state indicator, the feature that justifies the product, is washed out. Fix: introduce `--color-accent-soft` (lightened violet, e.g. `#C4B5FD`, ratio ~7:1) for accent text on dark; keep `#7C3AED` for fills. Suggested command: direct token edit, then `/uizze polish`.
- **[P2] Palette is indistinguishable from default shadcn.** Same cyan/violet/pink cluster as ten thousand generated dashboards. Why it matters: for a color product, an undifferentiated palette undermines the "trust our eye" principle. Fix: keep dark-only shadcn bones (a light theme would be actively harmful next to fullscreen games and color-critical work), keep cyan primary, and differentiate through the existing shape-coding language plus one committed Bauhaus nod (see direction note). Do NOT chase a new hue for novelty. Suggested command: `/uizze colorize`.
- **[P2] Two hardcoded status colors off-token.** Amber `#f5a623` and green `#4ade80` in `src/App.css` status dots. Why it matters: the only colors that can drift silently. Fix: `--color-warning` and `--color-success` tokens. Suggested command: direct token edit.
- **[P2] README vs DESIGN.md direction drift.** README claims Bauhaus light/dark; DESIGN.md says shadcn dark-only. Why it matters: every palette decision depends on which is true. Fix: declare dark-only shadcn canonical (recommendation below), correct the README. Suggested command: direct doc edit.

## Persona Red Flags

**Alex (Impatient Power User)**: No keyboard shortcuts for apply/pin/identify; every action is pointer-only. No bulk apply across monitors. Alex calibrates three displays and feels every click.
**Sam (Keyboard/Screen-reader User)**: Icon-only card buttons expose only tooltips and aria-labels; focus ring exists (`ring-primary`) but has never been audited at 200% zoom; status dots carry title attributes, good, but live-region announcement of apply/enforce events is absent.

## Minor Observations

- Secondary white-on-blue is 3.67:1, acceptable for bold labels, watch on smaller sizes.
- Uppercase-everywhere grammar (LIBRARY, MONITORS, footer status, ALL-CAPS buttons) matches the AI eyebrow scaffold family; product tools earn some of it, but sentence-case SEQ buttons would read more confident.
- Destructive pink text on card passes (5.25:1); white on destructive (3.53:1) only appears on bold button labels, acceptable.
- Selection tint uses `color-mix` primary at 25 percent, tasteful, keep.

## Direction note (README vs DESIGN.md, folded in per request)

Recommendation: **dark-only shadcn stays canonical.** Rationale: the UI sits beside fullscreen games and color-critical work, so a light theme is a working-conditions failure, not a feature; PRODUCT.md principle 5 already commits to this. The Bauhaus claim should survive as influence, not theme: geometric shape-coding and honest materials, expressed inside the dark system. Concretely: fix the README line, keep `DESIGN.md` tokens, differentiate via shape language and the P1/P2 token fixes above.

## Questions to Consider

- If the app icon and logo sat next to the UI, would anyone believe the same hand made both?
- What would a confident version of the preset card look like with no badges at all?
- Does cyan own enough meaning that a second hue anywhere would feel like noise?
