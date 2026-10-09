# ChromaDeck

Per-monitor display color profile manager for Windows. Save color presets — brightness, contrast, gamma, RGB gains, digital vibrance, hue, and ICC profiles — and apply, pin, or enforce them per monitor from a single deck-style UI.

![Preset library](docs/screenshots/library.png)

## Features

- **Global presets, per-monitor pins** — presets are monitor-agnostic and apply to any display (identified by EDID, stable across docking, reconnects, and reboots); pin one preset per monitor for enforcement. Monitors are user-renamable.
- **Empty by default** — fresh installs start with an idle library: no seeded preset, nothing pinned or applied. (Upgrades run a one-shot purge of pristine seeded Standard installs; user-tweaked presets are never touched.)
- **NVCP-native color engine** — brightness/contrast/gamma use NVIDIA's own transfer math, driver API, and registry persistence, so results match the NVIDIA Control Panel exactly. Automatic GDI fallback on non-NVIDIA displays.
- **Digital vibrance & hue** — driven through the NVIDIA driver (same control as NVCP), with per-display support detection.
- **ICC profiles** — bundle `.icc`/`.icm` files into presets and associate them per monitor.
- **Pin & enforce** — pin a preset per monitor and a background loop restores it if anything (games, HDR toggles, driver updates) stomps it. Tray-resident with autostart and single-instance.
- **Reset, duplicate, delete** — full-default reset (gamma + vibrance + hue), one-click duplicate, delete with confirmation.
- **Import NVCP state** — capture the driver's live color state straight into a new preset.
- **Preset deck UI** — dark-only shadcn theme (canonical), Bauhaus shape-language influence (geometric shape-coded status, honest flat materials), tap-to-apply deck, in-use highlighting on preset cards, monitor sidebar, support for offline monitors.
- **Tray-native window** — closing hides to the tray; every re-open (tray Show, tray click, second launch) replays the staged load animation; the window spawns centered on launch.

## Installation (Windows 10/11 x64)

Download from the [releases page](https://github.com/miiidev/ChromaDeck/releases) or build it yourself:

| File | What it is |
|---|---|
| `ChromaDeck_*_x64_en-US.msi` | Standard Windows installer (recommended) |
| `ChromaDeck_*_x64-setup.exe` | NSIS setup wizard |
| `chromadeck.exe` | Standalone binary, no install needed |

Requirements: Windows 10/11 x64 and the WebView2 runtime (preinstalled on Windows 10 1809+ and Windows 11). An NVIDIA GPU unlocks digital vibrance/hue and the NVCP-native engine; other GPUs fall back to OS-level color controls where supported.

## Usage

1. Launch ChromaDeck — the sidebar lists your monitors (connected, plus last-known offline ones); the library deck below holds your presets. Fresh installs start empty: click **+ Create** (or **Create preset** in the empty state) to build your first preset — name it, tweak sliders, optionally attach an ICC profile, capture the live NVCP state, or set a color tag.
2. Click a preset pad (or **Apply…** on a monitor's pinned preset) to open the Apply dialog: pick the target monitor (your last target per preset is remembered), optionally toggle **Pin**, then **Apply**.
3. A pinned preset stays enforced — the background loop restores it if games, HDR toggles, or driver updates stomp it. Cards show **IN USE** while pinned or applied on a connected monitor.
4. Manage each monitor from the sidebar: **✎** renames it, **Reset** returns it to defaults (and unpins it), **Unpin** releases a pin. **Identify** flashes every monitor's number on screen.
5. Manage the library from the deck toolbar: pencil edits, copy duplicates, trash deletes (with confirmation); **Select** enables multi-delete. The footer shows preset/monitor/pin counts plus **Reapply** and the **Start with Windows** toggle.
6. Closing the window hides it to the tray — right-click the tray icon for Show / Reapply now / Quit, or left-click to reopen.

All values show their neutral points in the editor (e.g. brightness/contrast 50, gamma 1.0): a fresh preset changes nothing until you move a slider.

## How it works

- **Frontend:** React + Vite + Tailwind CSS v4, dark-only shadcn theme with Bauhaus shape-language influence (geometric shape-coded status, honest flat materials; see `DESIGN.md`).
- **Backend:** Rust via Tauri v2. `monitor.rs` enumerates displays as an adapter→monitor tree with EDID identity; `store.rs` persists presets/pins/aliases as JSON; `color.rs` applies ICC + gamma ramps; `nvgamma.rs` + `nvapi.rs` implement the NVCP transfer math (reimplemented from observed driver behavior), 1024-entry float ramps, and driver-registry persistence; `enforce.rs` runs the 10-second drift-check loop; `identify.rs` spawns numbered overlay windows for monitor identification.
- **Data lives in** `%LOCALAPPDATA%\ChromaDeck\` (`presets.json`, `pins.json`, `applied.json`, `monitor_names.json`, `profiles\`, plus timestamped `.bak-*` backups before migrations).

## Development

Prerequisites: Node.js 20+, Rust stable toolchain.

```powershell
npm install          # frontend deps
npm run tauri dev    # dev app with hot-reload
```

Verification (all must pass):

```powershell
npx tsc --noEmit                                # typecheck
npm test                                        # frontend tests (vitest)
cargo test --manifest-path src-tauri/Cargo.toml # backend tests
npm run tauri build                             # release bundles -> src-tauri/target/release/bundle/
```
(All commands run from the repo root.)

Project layout:

```
src/                    # React frontend (App, components, lib)
src-tauri/src/          # Rust backend (monitor, store, color, nvapi, nvgamma, enforce)
docs/superpowers/       # design specs + implementation plans
docs/screenshots/       # README images
```

## Acknowledgements

- [nvBrightness](https://github.com/pbatard/nvBrightness) by Pete Batard — behavioral reference for NVIDIA's brightness transfer math (reimplemented, no code reused).
- [NvAPIWrapper](https://github.com/falahati/NvAPIWrapper) — reference for NVAPI interface IDs.
- Built with [Tauri](https://tauri.app/), React, and Tailwind CSS.

## License

MIT — see [LICENSE](LICENSE). (Default choice; say the word if you'd rather ship another license.)
