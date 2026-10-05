# Bauhaus Redesign — Approach A (CSS-vars + data-theme) Design

Date: 2026-10-05
Status: approved for implementation planning
Scope: full UI reskin, dual light/dark Bauhaus theme with toggle, full playful geometry, geometric sans

## 1. Context

ChromaDeck is Tauri 2 + React 19 + Vite + Tailwind v4 (`@import "tailwindcss"`, CSS-first, no tailwind.config).
Current visual language (to replace): neo-brutalist dark — `bg-neutral-950`, 2px `neutral-200` borders,
hard shadows (`4px_4px_0px_#e5e7eb`), square corners, lime-400 `#a3e635` accent, uppercase tracking-widest
labels, mono numerals. Files: `src/App.tsx`, `src/App.css` (range sliders + `trackFill()` helper in
`src/components/PresetEditor.tsx:4-7`), `src/components/MonitorList.tsx`, `MonitorSidebar.tsx`,
`PresetCard.tsx`, `PresetEditor.tsx` (434 lines), `ApplyDialog.tsx`. No `DESIGN.md` exists.

## 2. Goal

Replace lime-brutalism with authentic but restrained Bauhaus: off-white paper + ink grid + red/blue/yellow
primaries + circle/square/triangle motifs, functional asymmetry. Deliver as dual theme (light authentic,
dark adapted) with header/footer toggle persisted in localStorage. Add geometric sans for headings.

Non-goals: no backend/Rust changes, no preset logic changes, no new routes, no charting.

## 3. Tokens (new DESIGN.md source of truth)

Create `DESIGN.md` at repo root with exact values; `src/App.css` `@theme` maps to CSS vars:

- Paper light `#F4F1EA`, paper dark `#1E1E1E`; bg light paper, bg dark `#141414`
- Ink `#111111` (both themes; borders/rules always ink in light, paper `#F4F1EA` in dark)
- Red `#E30613` (destructive / APPLY primary), Blue `#0066B3` (info / EDIT), Yellow `#FFCC00` (warning / brightness; ink text on yellow only — yellow/white text forbidden)
- Grid: 8px base, 12-col content, section gap 20px (`gap-5` kept), container `p-6`
- Radius: 0 for blocks/cards/buttons; `rounded-full` only for circle motifs/status dots
- Shadow: `4px_4px_0 ink` light, `4px_4px_0 #000` dark; `2px_2px_0` for small buttons
- Type: Headings `Archivo` 600/700, uppercase, tracking 0.12em; body `Inter` system fallback; numerals `ui-monospace` kept. Load via Google Fonts link in `index.html` with `display=swap`, local fallback stacks.
- Color-role mapping (binding, prevents Mondrian noise):
  - APPLY = red bg + white text; EDIT = blue bg + white text; DUP = paper/ink outline; DEL = ink bg + red text/border; UNPIN amber replaced by yellow bg + ink text; PINNED badge = blue outline + blue text; ICC badge = ink outline; RETRY/REAPPLY = ink outline buttons.
  - Sliders: gamma=blue square thumb, brightness=yellow circle thumb (ink border), contrast=red triangle thumb (`clip-path`), RGB gains = respective channel ink-bordered squares, vibrance/hue = ink.

## 4. Architecture

- `src/lib/theme.tsx`: `ThemeProvider`, `useTheme()`, `theme: 'bauhaus-light' | 'bauhaus-dark'`, init from localStorage (`chromadeck-theme`, default light), effect sets `document.documentElement.dataset.theme`. Export `toggle()`.
- `src/App.css`: `:root, [data-theme='bauhaus-light']` vars + `[data-theme='bauhaus-dark']` overrides; `@theme` tokens referencing vars; range-input restyle (8px track, 2px ink border, shape-coded thumbs incl. `-moz-` variants, `:focus-visible` 2px blue outline offset 3px, `:disabled` gray); `.brutalist-btn` → `.bauhaus-btn` alias kept for compat (translate-y active, `prefers-reduced-motion` disables).
- `src/main.tsx`: wrap `<App/>` in `ThemeProvider`.
- `src/App.tsx`: header becomes asymmetric grid — left cluster (logo mark: 28px square split red/blue/yellow + circle + triangle SVG, title `ChromaDeck`, version badge), right cluster (theme toggle button `●/◐` + label LIGHT/DARK); footer status bar keeps uppercase mono summary + REAPPLY + autostart; error banner becomes ink-bordered block with red header bar.
- Components: `PresetCard` gets 6px left rail (red if pinned else blue) + shape badge row; `MonitorSidebar`/`MonitorList` status = ● connected (blue), ■ pinned (red), ▲ warning (yellow); `ApplyDialog`/`PresetEditor` modal = paper bg, 2px ink border, 6px shadow, header bar with primary block + title, no gradient.
- No data-flow changes: theme state is UI-local only; preset/monitor Tauri IPC (`listMonitors`, `listPresets`, `listPins`, `reapplyNow`) untouched.

## 5. Error handling / edge cases

- Font load failure → fallback stacks, no layout shift (fixed heading sizes).
- localStorage unavailable (privacy mode) → default light, toggle still works in-memory.
- Yellow contrast: enforce ink text on yellow via lint comment + manual check; never white-on-yellow or yellow-on-white body text.
- Existing hardcoded Tailwind neutrals (`neutral-950/900/800/200`) must all be replaced with vars — grep-verify zero remaining `neutral-` bg/border in touched files except intentional gray disabled states.
- Reduced motion: keep existing `motion-reduce:` guards.

## 6. Testing / verification

- `npm run build` (tsc + vite) passes; `vitest run` passes (add `src/lib/theme.test.ts` for init/toggle/persistence).
- Manual: toggle light/dark persists across reload; screenshots at 1280px + 375px; keyboard tab reaches toggle, sliders, dialog buttons with visible focus; delete-confirm + apply dialogs render correctly in both themes.
- Spot-check one interaction state per new component (hover/active/focus/disabled) and no text overflow at 375px.
