# ChromaDeck v1.0 Completeness Implementation Plan

> **For agentic workers:** Execute this plan **one phase at a time** (and task-by-task within the active phase). Steps use checkbox (`- [ ]`) syntax so progress persists across model switches and sessions. Pause after completing each phase's verification gate unless instructed to continue into the next phase.

**Goal:** Close all remaining UI/backend gaps and add zero-friction switching, preset portability, AMD native support, DDC/CI hardware brightness, Windows 11 ACM/HDR resilience, and WCAG AA keyboard accessibility to ship ChromaDeck `v1.0.0`.

**Tech Stack:** Rust (Tauri v2, `windows` crate 0.62, dynamic FFI for NVAPI / IGCL / ADLX / Dxva2), React 19.1 + TypeScript 6, Tailwind CSS v4 (`src/App.css` `@theme`), Base UI React + Radix Slider (`base-nova` conventions), Vitest + Playwright.

---

## Global Mandatory Rules (Read Before Every Phase)

1. **Respect `AGENTS.md`, `PRODUCT.md`, and `DESIGN.md`:**
   - Use CSS custom properties from `src/App.css` (`@theme` tokens) — never hardcode hex colors or magic spacing/radius numbers (`rounded-[8px]`, 1px `border-border` hairlines, `surface` / `surface-raised` / `surface-hover`, `Geist` + `JetBrains Mono`).
   - Follow `base-nova` component conventions (`data-slot`, `cva` variants, Base UI primitives).
   - **Honest capability & outcome reporting (`PRODUCT.md` Principle 6):** Never report false success. Unsupported adjustments on a target display must carry a clear human-readable reason; unverified writes must set `verified: false`; read-back failures on reset must surface errors.
2. **Display-Write Serialization:**
   - Any new runtime path that writes to hardware (live preview, CLI apply, hotkeys, tray apply, schedule) **must** acquire `auto_apply_runtime::DISPLAY_OPERATION` (respecting lock order: `DISPLAY_OPERATION` → `AppStore` → runtime state) and call `auto_apply_runtime::relinquish_manual` when acting on explicit user intent.
3. **Backwards-Compatible Persistence:**
   - Every new field on `Preset`, `PresetInput`, or `Settings` in `src-tauri/src/store.rs` must use `#[serde(default)]` (or a default fn) with a unit test proving legacy `presets.json` / `settings.json` files load unchanged, and must respect `Store::ensure_writable()` (`LoadFaults` read-only protection).
4. **Mandatory Verification Gate Per Phase:**
   - `cargo test --manifest-path src-tauri/Cargo.toml`
   - `npm run test` (Vitest)
   - `npm run build` (`tsc && vite build`)
   - Visual QA / Playwright check for any UI modification (per `AGENTS.md` Visual Acceptance).

---

## Phase Overview

| Phase | Name | Scope Summary | Complexity |
|---|---|---|---|
| **Phase 1** | Editor Completeness & Live Hardware Preview | RGB gains UI + validation fix, slider neutral-reset & numeric input, `captureNvcp` UI, detach ICC + optional ICC reset, live panel preview in editor | M |
| **Phase 2** | Smarter Auto-Apply & Running-App Picker | Revert to `applied.json` preset instead of raw identity, multi-monitor concurrent auto-apply, running-processes combobox | M |
| **Phase 3** | Zero-Friction Switching (Tray, CLI, Hotkeys) | Per-monitor preset submenus in tray, single-instance CLI flags (`--apply`, `--reset`, `--reapply`), global hotkeys | M |
| **Phase 4** | Portability, Scheduling & Diagnostics Export | `.chromadeck` bundle import/export (JSON + ICC), day/night schedule rules, one-click diagnostics `.zip`/report export | M |
| **Phase 5** | AMD Native Backend (`ADLX`) | Dynamic-loaded `amdadlx64.dll` FFI spike + `AmdBackend` (gamma/LUT, saturation, hue, gains) + drift enforcement | L |
| **Phase 6** | DDC/CI Hardware Controls & HDR/ACM Events | Opt-in DDC/CI VCP luminance/contrast (`Dxva2`), Windows 11 ACM detection, `WM_DISPLAYCHANGE` HDR toggle listener | M–L |
| **Phase 7** | Accessibility Pass & `v1.0.0` Release Gate | WCAG AA keyboard grid roving focus, `aria-live` announcements, Playwright E2E suite, docs & `1.0.0` bump | S–M |

---

## Phase 1: Editor Completeness & Live Hardware Preview

**Status:** COMPLETE (verified: 285 cargo tests, 215 vitest tests, `npm run build`, 5 Playwright E2E tests + Visual QA screenshots).

**Goal:** Expose every adjustment and helper already implemented in the backend (`rgb_gains`, `resetSliderValues`, `capture_nvcp_cmd`), allow removing attached ICC profiles, and let users preview slider tweaks live on a real monitor.

### Task 1.1: Store Validation Parity & Clear-ICC Contract
**Files:**
- Modify: `src-tauri/src/store.rs` (`PresetInput`, `update_preset`, orphan ICC cleanup helper)
- Modify: `src/lib/types.ts`, `src/lib/presetForm.ts`
- Modify: `src/lib/presetForm.test.ts`

- [x] **Step 1:** Write failing Rust unit tests in `store.rs`:
  - `update_preset` rejects out-of-range `rgb_gains` (outside `0.0..=10.0`), matching `create_preset`.
  - `update_preset` with `clear_icc: true` clears `icc_hash` and `icc_filename` to `""`, and deletes `profiles/{hash}.icc` if and only if no other preset references `hash`.
- [x] **Step 2:** Add `#[serde(default)] pub clear_icc: bool` to `PresetInput` in `store.rs` and `clear_icc?: boolean` in `src/lib/types.ts`; add `rgb_gains` validation to `Store::update_preset`.
- [x] **Step 3:** Add `withClearedIcc(form: PresetInput): PresetInput` in `src/lib/presetForm.ts` + Vitest unit tests.

### Task 1.2: RGB Gains Sliders & Slider Precision Ergonomics
**Files:**
- Modify: `src/components/PresetEditor.tsx`
- Modify: `src/lib/validation.ts`, `src/lib/validation.test.ts`

- [x] **Step 1:** Upgrade `SliderField` in `PresetEditor.tsx`:
  - Support `neutral: number` prop: double-clicking the slider or clicking a subtle non-neutral indicator resets that single parameter to `neutral`.
  - Support inline click-to-edit numeric readout (`JetBrains Mono` `12px` `tabular-nums`): clicking the value turns it into a compact numeric input (`Enter`/blur commits clamped/validated number, `Escape` reverts).
- [x] **Step 2:** Add Red, Green, and Blue gain `SliderField` controls (`0.50` to `1.50` slider range, `0.01` step, `1.00` neutral; numeric input accepts full `0.00..=10.00` range) under a clean `RGB GAINS` sub-group in `PresetEditor.tsx`, plus a header ghost action `Reset sliders` wired to `resetSliderValues(form)`.
- [x] **Step 3:** Add a `Clear` (`X`) button on the ICC profile row in `PresetEditor.tsx` whenever `iccStatus` is present, calling `withClearedIcc` and clearing `iccStatus`.

### Task 1.3: Wire "Import from Monitor" (`captureNvcp`)
**Files:**
- Modify: `src/components/PresetEditor.tsx`, `src/App.tsx` (pass `monitors` prop to `PresetEditor`)
- Modify: `src-tauri/src/nvgamma.rs` (optional: add `capture_display_state_cmd` wrapper or use `captureNvcp` + `getDisplayCapabilities`)

- [x] **Step 1:** Pass `monitors: Monitor[]` into `PresetEditor`.
- [x] **Step 2:** Add an *"Import from monitor"* control in `PresetEditor` that lists connected displays, checks `getDisplayCapabilities(edid_id)`, and for `nvapi` displays calls `captureNvcp(edid_id)` to populate `brightness`, `contrast`, `gamma`, `vibrance`, and `hue_deg` (resetting `rgb_gains` to `[1, 1, 1]` with an honest note that per-channel gains aren't stored separately by NVCP). Non-NVAPI displays show disabled with reason (`"Live driver capture requires an NVIDIA display"`).

### Task 1.4: Live Hardware Preview Toggle in PresetEditor
**Files:**
- Modify: `src-tauri/src/color.rs` (add `preview_preset_cmd(input: PresetInput, target_edid: String) -> ApplyOutcome` and `restore_preview_cmd(state, target_edid: String) -> ApplyResult`)
- Modify: `src-tauri/src/lib.rs`, `src/lib/tauri.ts`, `src/components/PresetEditor.tsx`

- [x] **Step 1:** Implement `preview_preset_cmd` in `color.rs`: acquires `DISPLAY_OPERATION`, constructs an ephemeral `Preset` (no store write, skips ICC association so OS profiles aren't churned during slider drags), and runs `apply_color`.
- [x] **Step 2:** In `PresetEditor.tsx`, add an opt-in *"Live preview"* bar (toggle + connected monitor selector). When enabled, throttle slider changes (~60 ms) to `previewPreset`.
- [x] **Step 3:** Guarantee cleanup: if the user clicks `Cancel`, closes the dialog (`Escape`/backdrop), switches target monitor, or toggles Live Preview off, restore the monitor to its pre-editor state (its pinned/applied preset if one existed, otherwise `resetMonitor`).

### Task 1.5: Optional ICC Dissociation on Monitor Reset
**Files:**
- Modify: `src-tauri/src/color.rs` (`ColorApi::dissociate_icc` using `DisassociateColorProfileFromDeviceW`), `src/lib/tauri.ts`, `src/components/MonitorSidebar.tsx`

- [x] **Step 1:** Add `clear_icc: bool` (default `false`) option to `reset_monitor_cmd` and `reset_all_monitors_cmd` so default behavior remains non-destructive while giving users an explicit checkbox in the Reset-all confirmation dialog (and Shift+click on single-monitor Reset) to remove ChromaDeck-installed ICC associations.
- [x] **Step 2:** Run Phase 1 verification gates (`cargo test`, `npm test`, `npm run build`, Playwright/Visual QA) and commit.

---

## Phase 2: Smarter Auto-Apply & Running-App Picker

**Status:** COMPLETE (verified: 291 cargo tests, 215 vitest tests, `npm run build`, 6 Playwright E2E tests + Visual QA screenshot).

**Goal:** Make foreground auto-apply multi-monitor aware, preserve manually applied desktop presets on revert, and let users pick from running apps without hunting `.exe` paths.

### Task 2.1: Revert to Previous Manual Preset (`applied.json`) Instead of Raw Identity
**Files:**
- Modify: `src-tauri/src/auto_apply.rs`, `src-tauri/src/auto_apply_runtime.rs`
- Modify: `src/lib/autoApplyState.ts`, `src/lib/autoApplyState.test.ts`

- [x] **Step 1:** Write failing unit tests in `auto_apply.rs` and `auto_apply_runtime.rs`:
  - Monitor `D1` has manual preset `Desktop Warm` in `applied.json` (unpinned). Mapped game `cs2.exe` opens on `D1` → applies `FPS Vibrance`. `cs2.exe` closes → `D1` reverts to `Desktop Warm` (`apply_color`), **not** raw identity (`reset_monitor`).
  - Monitor `D2` has nothing in `applied.json` → `cs2.exe` closes → `D2` reverts via `reset_monitor` as before.
- [x] **Step 2:** Implement fallback lookup against `store.list_applied()` inside the `AutoAction::Revert` and switch-off revert handlers in `auto_apply_runtime.rs` (falling back to `reset_monitor` if `applied.json` has no entry or the referenced preset was deleted).

### Task 2.2: Multi-Monitor Concurrent Auto-Apply Tracking
**Files:**
- Modify: `src-tauri/src/auto_apply.rs`, `src-tauri/src/auto_apply_runtime.rs`, `src/lib/tauri.ts`, `src/App.tsx`

- [x] **Step 1:** Upgrade `ActiveAuto` state from a single `Option<ActiveAuto>` to per-monitor map `HashMap<String, ActiveAuto>` (`edid_id -> ActiveAuto`) and evaluate visible/foreground windows cleanly so moving a mapped window from Monitor A to Monitor B reverts A to its prior state and applies on B atomically, with `list_auto_applied_cmd` returning `Vec<AutoAppliedInfo>` (keeping frontend `AutoMap` in sync).
- [x] **Step 2:** Add unit tests covering dual-monitor window drag, monitor unplug mid-session, concurrent two-monitor presets, and manual apply relinquishing only the targeted monitor.

### Task 2.3: "Pick from Running Programs" Combobox in PresetEditor
**Files:**
- Modify: `src-tauri/src/foreground.rs` (`list_visible_windows`, `filter_running_programs`, `list_running_programs_cmd`)
- Modify: `src-tauri/src/lib.rs`, `src/lib/tauri.ts`, `src/components/PresetEditor.tsx`, `src/components/ui/combobox.tsx`

- [x] **Step 1:** Implement `list_running_programs_cmd()` in `foreground.rs` using `EnumWindows` (`IsWindowVisible && !IsIconic && GetWindowTextLengthW > 0` → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW`), normalizing to deduplicated sorted lowercase `.exe` names and excluding `self_program()` and core shell noise (`explorer.exe`, `searchhost.exe`, `shellexperiencehost.exe`, `applicationframehost.exe`, etc.).
- [x] **Step 2:** Replace the plain text `Input` in `PresetEditor.tsx` Programs section with Base UI `Combobox` pre-populated by `listRunningPrograms()` on open (plus a refresh button), while still allowing arbitrary typed `.exe` names and `Browse…`.
- [x] **Step 3:** Run Phase 2 verification gates (`cargo test`, `npm test`, `npm run build`, Playwright E2E + Visual QA).

---

## Phase 3: Zero-Friction Mid-Game Switching (Tray, CLI, Global Hotkeys)

**Status:** COMPLETE (verified: 301 cargo tests, 218 vitest tests, `npm run build`, 7 Playwright E2E tests + Visual QA screenshot).

**Goal:** Allow users to switch presets, reset monitors, and trigger reapply mid-game or via Stream Deck / scripts without opening the main window.

### Task 3.1: Dynamic Per-Monitor Preset Submenus in System Tray
**Files:**
- Create: `src-tauri/src/tray.rs` (extracted from `src-tauri/src/lib.rs`)
- Modify: `src-tauri/src/store.rs`, `src-tauri/src/color.rs` (trigger `tray::refresh_tray` on preset/pin/apply/reset/alias mutations)

- [x] **Step 1:** Create `src-tauri/src/tray.rs` with `build_tray` and `refresh_tray(app: &AppHandle)`:
  - Top items: `Show ChromaDeck` | per-connected-monitor `Submenu` (`{alias_or_model}` ▸ `CheckMenuItem` per `Preset` showing shortcut hint + separator + `Reset to default`) | separator | `Reapply now` | `Auto-apply` check item | separator | `Quit`.
- [x] **Step 2:** Handle menu event IDs (`tray-apply::{edid_id}::{preset_id}`, `tray-reset::{edid_id}`), acquiring `DISPLAY_OPERATION`, updating `applied.json`, relinquishing auto-apply on that monitor, and emitting `store://changed` so the main window stays live if open.
- [x] **Step 3:** Rebuild the tray menu whenever presets, pins, applied records, or monitor names change.

### Task 3.2: CLI Flags via Single-Instance Hook (`Stream Deck / Scripting`)
**Files:**
- Create: `src-tauri/src/cli.rs`
- Modify: `src-tauri/src/lib.rs`

- [x] **Step 1:** Implement pure argument parser `parse_cli_args(args: &[String]) -> ParsedCli` in `cli.rs` with full unit tests (pure `std` slice parser):
  - `--apply <preset-name-or-id> [--monitor|-m <1-based-index | alias | edid_id | all>] [--pin]`
  - `--reset [--monitor|-m <1-based-index | alias | edid_id | all>] [--clear-icc]` / `--reset-all`
  - `--reapply`
  - `--auto-apply <on|off|toggle>`
  - `--headless` / `--quiet` (start hidden in tray on cold boot without showing main window).
- [x] **Step 2:** Wire `cli::execute_cli` into `tauri_plugin_single_instance::init(|app, argv, _cwd| ...)` so running `chromadeck.exe --apply "Warm" --monitor 1` while ChromaDeck is in the tray executes silently **without** popping the main window forward over a fullscreen game.
- [x] **Step 3:** Support cold-start CLI execution & `--headless` in `.setup(...)`.

### Task 3.3: Global Hotkeys (Native Win32 `RegisterHotKey`)
**Files:**
- Modify: `src-tauri/Cargo.toml` (enable `Win32_UI_Input_KeyboardAndMouse` on existing `windows` crate — zero new crates)
- Create: `src-tauri/src/hotkeys.rs`, `src/lib/shortcut.ts`, `src/lib/shortcut.test.ts`
- Modify: `src-tauri/src/store.rs` (`Preset.shortcut`, `Settings.reapply_shortcut`, `Settings.cycle_shortcut`), `src/lib/types.ts`, `src/lib/presetForm.ts`, `src/lib/tauri.ts`, `src/components/PresetEditor.tsx`, `src/components/PresetCard.tsx`, `src/App.tsx`

- [x] **Step 1:** Implement dedicated native Win32 `RegisterHotKey` / `WM_HOTKEY` worker thread in `hotkeys.rs` so shortcuts fire reliably even while hidden in tray or behind fullscreen games.
- [x] **Step 2:** Add `#[serde(default)] pub shortcut: Option<String>` to `Preset` / `PresetInput` with canonical normalization (`Ctrl+Alt+F1`) and uniqueness validation across presets and global shortcuts (`reapply_shortcut`, `cycle_shortcut`) in `Store`. Target resolution applies to the monitor holding the current foreground window (falling back to primary connected monitor).
- [x] **Step 3:** Add a key-chord recorder input in `PresetEditor.tsx` (`Click to record…` + `Clear`), suspending global shortcuts via `set_hotkeys_suspended_cmd` while recording, and show shortcut badges on `PresetCard.tsx`.
- [x] **Step 4:** Run Phase 3 verification gates (`cargo test`, `npm test`, `npm run build`, Playwright E2E + Visual QA).

---

## Phase 4: Preset Portability, Scheduling & Diagnostics Export — COMPLETE

**Goal:** Let users back up/share presets (with ICC files intact), schedule automatic day/night display looks, and export one-click hardware diagnostics.

### Task 4.1: Preset & Library Import/Export Bundles (`.chromadeck.json`)
**Files:**
- Create: `src-tauri/src/bundle.rs`
- Modify: `src-tauri/src/store.rs`, `src-tauri/src/lib.rs`, `src/lib/tauri.ts`
- Modify: `src/components/MonitorList.tsx` (library toolbar Import/Export), `src/components/PresetCard.tsx` (single-preset export action)

- [x] **Step 1:** Design self-contained `.chromadeck.json` schema (`schema_version: 1`, `exported_at`, `presets: Vec<ExportedPreset>`, `icc_blobs: HashMap<String, String>` hex/base64-encoded `.icc` bytes keyed by `sha256` hash). Zero extra zip C-dependencies required, human-inspectable, and verifies SHA-256 of every embedded ICC blob before writing to `profiles/{hash}.icc`!
- [x] **Step 2:** Implement `export_presets_cmd(ids: Option<Vec<String>>, dest_path: String)` and `import_presets_cmd(src_path: String) -> ImportSummary` (`imported_count`, `renamed_collisions`, `stripped_conflicting_programs` reported honestly when a program is already mapped locally).
- [x] **Step 3:** Wire `Export` and `Import` buttons via `@tauri-apps/plugin-dialog` `save` / `open` in the library toolbar (and single-card export), with Vitest + Rust round-trip tests (including corrupted ICC hash rejection).

### Task 4.2: Time-of-Day Per-Monitor Schedule Rules
**Files:**
- Create: `src-tauri/src/schedule.rs`
- Modify: `src-tauri/src/store.rs` (`Settings.schedules: Vec<ScheduleRule>`), `src-tauri/src/lib.rs`, `src/lib/tauri.ts`
- Create: `src/components/ScheduleDialog.tsx`, Modify: `src/App.tsx` (footer toggle/button)

- [x] **Step 1:** Implement pure `schedule.rs` evaluation engine (unit-tested across midnight wrap-around, e.g., `21:00` → `07:00`):
  - Precedence law: **Pinned monitor > Active Foreground Auto-Apply > Active Schedule Rule > Manual Default**.
  - Evaluates once per minute and on wake/startup; only applies on schedule window transitions so manual overrides within a window are respected until the next transition.
- [x] **Step 2:** Build a compact `ScheduleDialog.tsx` following `DESIGN.md` (`surface-raised`, `rounded-[8px]`, `1px` borders, `JetBrains Mono` `HH:MM` inputs) accessible from the footer bar.

### Task 4.3: One-Click Diagnostics Report Export
**Files:**
- Modify: `src-tauri/src/diagnostics.rs` (`export_diagnostics_cmd(dest_path: String) -> Result<(), String>`)
- Modify: `src-tauri/src/lib.rs`, `src/lib/tauri.ts`, `src/App.tsx`

- [x] **Step 1:** Gather ChromaDeck version, OS build, adapter/monitor enumeration tree, per-monitor `probe_monitor` + `get_display_capabilities` JSON, tail of `diagnostics.log`, and tail of `auto-apply.log` (redacting user home path usernames to `<USER>`) into a single structured `.txt`/`.log` report saved via file dialog.
- [x] **Step 2:** Run Phase 4 verification gates and commit.

---

## Phase 5: AMD Native Display Backend (`ADLX`)

**Goal:** Deliver first-class AMD Radeon & Ryzen iGPU support alongside NVIDIA and Intel, completing three-vendor desktop & hybrid-laptop coverage.

### Task 5.1: ADLX Dynamic-Load Spike & Header Transcription
**Files:**
- Create: `spikes/amd-adlx/` (standalone probe crate, mirroring `spikes/intel-igcl/`)
- Modify: `NOTICE` (add MIT attribution for AMD ADLX SDK C header struct/vtable definitions)

- [ ] **Step 1:** Transcribe minimal `#[repr(C)]` C-interface vtables from AMD's public MIT-licensed ADLX SDK (`ADLXQueryFullVersion`, `ADLXInitializeWithCallerAdl` / `ADLXInitialize`, `IADLXSystem`, `IADLXDisplayServices`, `IADLXDisplay`, `IADLXDisplayCustomColor`, `IADLXDisplayGamma`) loaded dynamically via `LoadLibraryExW("amdadlx64.dll")` — zero build-time SDK binary dependency; machines without AMD drivers cleanly skip in <1 ms.
- [ ] **Step 2:** Verify display correlation (`IADLXDisplay::EDID` / manufacturer+model+serial / connector mapping to ChromaDeck `edid_id`) and document findings in `docs/superpowers/specs/amd-adlx-design.md`. If no AMD hardware is attached on the dev machine, gate hardware-live paths cleanly behind mockable `AmdApi` FFI trait (matching `NvColorApi` and `IntelBackend` mock seams).

### Task 5.2: `AmdBackend` Implementation & Registry Integration
**Files:**
- Create: `src-tauri/src/backend/amd.rs`
- Modify: `src-tauri/src/backend/mod.rs` (`BackendRegistry` selection order: `nvapi` → `amd` → `intel` → `gdi` per physical display)
- Modify: `src-tauri/src/capabilities.rs`, `src-tauri/src/enforce.rs`, `src-tauri/src/color.rs`

- [ ] **Step 1:** Implement `ColorBackend` for `AmdBackend`:
  - `id()` → `"amd"`, label → `"AMD (ADLX)"`.
  - `probe(edid_id)` queries `IADLXDisplayCustomColor` (`IsHueSupported`, `IsSaturationSupported`, `IsBrightnessSupported`, `IsContrastSupported`, `IsTemperatureSupported`) and HDR state per display, reporting exact `Supported` / `Unsupported(reason)` verdicts.
  - `apply` maps ChromaDeck UI ranges (`brightness` 0–100, `contrast` 0–100, `gamma` 0.3–2.8, `saturation` 0–100, `hue_deg` 0–359, `rgb_gains`) to ADLX custom color & re-gamma LUT, purging GDI ramp to identity first for stage purity.
  - `restore` resets ADLX custom color & gamma to driver defaults and verifies by read-back.
- [ ] **Step 2:** Wire `AmdBackend` into `enforce.rs` drift detection (reading back ADLX custom color / gamma state) and `probe_monitor_cmd`.
- [ ] **Step 3:** Add comprehensive unit tests with mock ADLX vtables + run Phase 5 verification gates and commit.

---

## Phase 6: DDC/CI Hardware Backlight & Windows 11 ACM/HDR Resilience

**Goal:** Add optional physical monitor backlight control via DDC/CI and make ChromaDeck reactive to Windows 11 HDR / Auto Color Management (ACM) state changes.

### Task 6.1: Opt-In DDC/CI Hardware Brightness (`Dxva2`)
**Files:**
- Modify: `src-tauri/Cargo.toml` (add `"Win32_Devices_Display"` to `windows` crate features)
- Create: `src-tauri/src/ddcci.rs`
- Modify: `src-tauri/src/store.rs` (`Preset.hardware_brightness: Option<u8>`), `src-tauri/src/capabilities.rs`, `src-tauri/src/color.rs`, `src/lib/types.ts`, `src/components/PresetEditor.tsx`

- [ ] **Step 1:** Implement `DdcciApi` trait + `RealDdcci` using `EnumDisplayMonitors` → `GetPhysicalMonitorsFromHMONITOR` → `GetVCPFeatureAndVCPFeatureReply` / `SetVCPFeature` (VCP `0x10` Luminance, `0..=max` normalized to `0..=100`), always calling `DestroyPhysicalMonitors` on drop. Cache negative DDC/CI support per `edid_id` (since DDC/CI I2C timeouts on unsupported/internal laptop panels take ~40–100 ms; laptop eDP panels can optionally check WMI/display brightness or report `"Internal/non-DDC panel"`).
- [ ] **Step 2:** Keep `hardware_brightness: Option<u8>` **opt-in per preset** (toggle + 0–100 slider in `PresetEditor.tsx`): when `None`, presets never touch physical monitor OSD brightness. Capture pre-apply VCP `0x10` value in session state so `Reset` / Auto-apply `Revert` restores prior backlight brightness cleanly.

### Task 6.2: Windows 11 ACM Detection & `WM_DISPLAYCHANGE` Listener
**Files:**
- Modify: `src-tauri/src/monitor.rs` or create `src-tauri/src/display_events.rs`
- Modify: `src-tauri/src/capabilities.rs`, `src-tauri/src/enforce.rs`

- [ ] **Step 1:** Query `DisplayConfigGetDeviceInfo` with `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO` per target to detect both **HDR active** and **Windows 11 Auto Color Management (ACM) active** on SDR panels across *all* vendors (not only Intel IGCL). Report HDR/ACM status in `Monitor` diagnostics and `get_display_capabilities` (`Unsupported("HDR is active on this display — switch to SDR to apply presets")` where driver LUTs are bypassed).
- [ ] **Step 2:** Spawn a lightweight hidden message-only Win32 window listening for `WM_DISPLAYCHANGE` and `WM_SETTINGCHANGE`: on display hot-plug or `Win+Alt+B` HDR toggle, invalidate capability caches, emit `monitors://changed` to the frontend (instant UI update instead of waiting up to 5s poll), and trigger a non-forced `enforce::check_once` pass when a display returns from HDR to SDR.
- [ ] **Step 3:** Run Phase 6 verification gates and commit.

---

## Phase 7: Accessibility Pass & `v1.0.0` Release Readiness

**Goal:** Complete the deferred accessibility audit in `PRODUCT.md`, expand Playwright E2E coverage across all new workflows, update docs, and prepare `v1.0.0`.

### Task 7.1: WCAG AA Contrast, Focus Rings & Keyboard Deck Navigation
**Files:**
- Modify: `src/components/MonitorList.tsx`, `src/components/MonitorSidebar.tsx`, `src/components/PresetCard.tsx`, `src/components/PresetEditor.tsx`, `src/App.tsx`, `src/App.css`

- [ ] **Step 1:** Implement arrow-key roving focus across the Preset grid/carousel (`ArrowLeft`/`ArrowRight`/`ArrowUp`/`ArrowDown`, `Enter` to open ApplyDialog, `E` to edit, `Delete` to confirm delete) and Monitor cards.
- [ ] **Step 2:** Add a visually hidden `aria-live="polite"` status region announcing apply outcomes, auto-apply transitions, pin changes, and reset results; verify 3px `--color-ring` focus visibility and ≥4.5:1 AA contrast on every new control and badge.
- [ ] **Step 3:** Verify `prefers-reduced-motion: reduce` disables any new transitions/animations cleanly.

### Task 7.2: Playwright E2E Suite, Docs & `v1.0.0` Bump
**Files:**
- Create/Modify: `tests/editor-completeness.spec.ts`, `tests/portability-and-schedule.spec.ts`, `tests/keyboard-a11y.spec.ts`
- Modify: `README.md`, `PRODUCT.md`, `DESIGN.md`, `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`

- [ ] **Step 1:** Add Playwright E2E tests covering RGB gains editing, numeric slider input, ICC attach/clear, bundle export/import UI, schedule dialog, and keyboard deck navigation in browser-preview mode.
- [ ] **Step 2:** Update `README.md` (Display support matrix with AMD + DDC/CI, CLI flags table, hotkeys, bundle sharing, schedules), `PRODUCT.md` (mark Accessibility completed), and `DESIGN.md` (document new component slots).
- [ ] **Step 3:** Run full release verification suite:
  - `npx tsc --noEmit`
  - `npm test`
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `npm run build`
  - `npx playwright test`
- [ ] **Step 4:** Bump version to `1.0.0` across `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` (when authorized by user for release).
