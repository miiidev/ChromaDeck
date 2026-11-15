# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

ChromaDeck is a Windows-only Tauri v2 desktop app (React 19 + TypeScript frontend, Rust backend) that manages per-monitor color presets (gamma, brightness, contrast, RGB gains, saturation, vibrance, hue, ICC profiles) across NVIDIA, Intel, and generic displays.

## Commands

All commands run from the repo root.

```powershell
npm install
npm run tauri dev                 # full desktop app with hot reload (Vite on port 1420)
npm run dev                       # frontend only in a browser — uses demo fixtures, no Tauri IPC

npm run build                     # tsc + vite build
npx tsc --noEmit                  # typecheck only
npm test                          # vitest (src/**/*.test.ts, node environment)
npx vitest run src/lib/presetForm.test.ts      # single test file
npx vitest run -t "name of test"               # single test by name

cargo test --manifest-path src-tauri/Cargo.toml              # Rust unit tests
cargo test --manifest-path src-tauri/Cargo.toml store::      # Rust tests filtered by module path

npx playwright test               # browser e2e in tests/ (starts `npm run dev` on :1420 automatically)
npm run tauri build               # release bundles -> src-tauri/target/release/bundle/
npm run bundle:portable           # portable ZIP (run after tauri build)
```

Before declaring work complete, `npm run build`, `npm test`, and `cargo test` should all pass (per README/AGENTS.md).

## Architecture

### Frontend ↔ backend bridge
- `src/lib/tauri.ts` is the only place that calls `invoke()`. Every function wraps one Rust command registered in `src-tauri/src/lib.rs` (`generate_handler![...]`). Adding a command means: implement in Rust → register in `lib.rs` → add a typed wrapper in `tauri.ts` → mirror types in `src/lib/types.ts`.
- **Browser preview mode:** when `isTauri()` is false, read commands return fixtures from `src/lib/demoData.ts` (mirroring the Figma "01-App-Shell-Default" frame). This is what `npm run dev` and Playwright exercise; any Tauri-only effect in `App.tsx` must guard on `isTauri()`.
- `src/main.tsx` routes by window label: windows labelled `identify-N` render `IdentifyOverlay` (transparent numbered overlays spawned by `identify.rs`); everything else renders `App`.
- `src/App.tsx` holds most app state. Pure logic is extracted into `src/lib/*.ts` modules, each with a sibling `.test.ts` — that is where unit tests live (vitest runs in node, no DOM; components are not unit-tested).

### Rust backend (`src-tauri/src/`)
- `backend/mod.rs` defines the vendor-neutral `ColorBackend` trait; implementations are NVIDIA (NVAPI, via `nvapi.rs` + `nvgamma.rs`, which reimplements NVCP transfer math), Intel (`backend/intel.rs`, dynamically loaded IGCL), and a GDI fallback. The backend is chosen **per physical display**, so hybrid laptops route panels independently. AMD falls back to GDI.
- `color.rs` orchestrates an apply: ICC association (Mscms, vendor-neutral) plus the selected backend. `capabilities.rs` reports per-adjustment availability (including HDR → unsupported) to the editor/Apply dialog.
- `monitor.rs` enumerates adapter→monitor trees with EDID identity. Monitors are keyed by `edid_id` everywhere (stable across reconnects); presets themselves are monitor-agnostic.
- `store.rs` persists `presets.json`, `pins.json`, `applied.json`, `monitor_names.json`, `profiles/` under `%LOCALAPPDATA%\ChromaDeck\` (portable build: `data/` next to the exe; `CHROMADECK_DATA_DIR` overrides). Migrations write timestamped `.bak-*` backups first.
- `enforce.rs` runs startup re-apply and a 10s drift-check loop that restores pinned presets. **Intel displays are deliberately skipped** by enforcement (their state lives in a runtime LUT that Reset depends on capturing).
- Auto-apply (program → preset): `auto_apply.rs` is a pure, Win32-free state machine (all policy, fully unit-tested); `auto_apply_runtime.rs` executes it; `foreground.rs` is the minimal Win32 probe for the foreground exe and its display. Auto state is in-memory only, never written to `applied.json`.
- `updater.rs` wraps `tauri-plugin-updater` (Ed25519-signed MSI). Frontend dialog state lives in `src/lib/autoUpdateState.ts` + `src/components/updater/`.
- App is tray-resident and single-instance; closing the window hides to tray.

## Design system and UI conventions

- **Figma is authoritative for appearance** (file key `vfF5g0hTALNbScBM4Nz1Y1`); the existing app is authoritative for behavior. A reskin must not change flows, state, or Tauri integrations — functional changes need explicit user authorization. See `AGENTS.md` for the full rules (inspect before editing, use real Figma values via MCP, report unknown values instead of guessing, visual verification against Figma before claiming visual completion).
- `DESIGN.md` is the current design spec (Flat Dark Minimal, mint `#5eead4` accent, Geist font). The README's "light neobrutalist" description is outdated.
- Design tokens are CSS custom properties in the `@theme` block of `src/App.css` (Tailwind v4, no tailwind.config). Use tokens, not magic values.
- UI primitives in `src/components/ui/` follow shadcn `base-nova` conventions: `data-slot` attributes, `cva` variants, Base UI React primitives (Radix only for Slider). Icons: lucide-react.
- Preset tag color is exposed as a `--tag` CSS variable and reused for state styling (`var(--tag, var(--color-primary))`).

## Docs

`docs/superpowers/specs/` and `docs/superpowers/plans/` hold dated design specs and implementation plans for past and in-progress features — check them for the rationale behind a feature before changing it.
