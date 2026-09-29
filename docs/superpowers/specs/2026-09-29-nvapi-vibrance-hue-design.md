# NVAPI Vibrance + Hue Presets — Design Spec

**Status:** approved sections 1–5 (2026-09-29 chat); awaiting spec review.
**Approach:** A — dynamic-load `nvapi64.dll`, no SDK, no link-time dependency.

## Goal

Extend ChromaDeck presets with NVIDIA Control Panel parity for desktop
color: digital vibrance and hue per monitor, alongside the existing
ICC + gamma LUT controls. Out of scope: output format/depth/range,
resolution/refresh, video color settings, 3D app profiles.

## Background constraints (verified)

- No OS API exists for vibrance/hue; NVIDIA-only via NVAPI display
  color control. `nvapi64.dll` ships with the driver (present here:
  RTX 3050, driver 610.74), so dynamic loading needs no redist.
- 1D gamma LUTs cannot express saturation or hue rotation (channel
  mixing required) — NVAPI is the only path, no LUT emulation.
- Laptop internal panels may be iGPU-driven and report unsupported;
  the design degrades per display, never globally.

## 1. Store schema + migration

- `Preset` gains `vibrance: f32` (range 0–100, default 50.0 = NVIDIA
  neutral) and `hue_deg: f32` (range −180–180, default 0.0).
- Rust: `#[serde(default = "...")]` on both fields, so existing
  `presets.json` files load with neutral values and need no rewrite
  (written back with fields on next save).
- Backend validation (`create_preset` / `update_preset`) rejects
  out-of-range values with field-named errors, matching the existing
  validation pattern.
- TypeScript `Preset` / `PresetInput` gain required `vibrance: number`
  and `hue_deg: number`. Safe because the backend always serializes
  the fields; the frontend never observes them missing.
- Frontend form validation mirrors the backend ranges.

## 2. `nvapi.rs` module

- Single owner of all NVAPI interaction. Loads `nvapi64.dll` once via
  `std::sync::OnceLock` (`LoadLibraryW`); resolves function pointers
  via `NvAPI_QueryInterface` for exactly: `NvAPI_Initialize`,
  `NvAPI_DISP_GetDisplayIdByDisplayName`,
  `NvAPI_DISP_GetColorControl`, `NvAPI_DISP_SetColorControl`
  (interface IDs taken from `nvapi.h` at implementation; stable across
  driver versions).
- Display mapping: our `device_name` (`\\.\DISPLAYn`) →
  `NvAPI_DISP_GetDisplayIdByDisplayName` → display ID used for
  get/set color control.
- Support probe `nvapi_color_supported(device_name) -> bool`: DLL
  present AND init OK AND display ID resolves AND get-color-control
  succeeds. Exposed as Tauri command
  `vibrance_supported_cmd(edid_id: String) -> bool` (uses the existing
  EDID→device_name resolution).
- Any absence (no DLL, init failure, unsupported display) yields
  `false` / descriptive errors — never a panic, never a hard build
  dependency.
- Hardware isolated behind a `ColorControl` trait (get/set/support)
  with a `TestRecorder`-style mock, mirroring the `ColorApi` pattern.
- NVAPI struct field names and hue units are pinned against the
  headers at implementation; any unit mismatch is normalized in a
  thin adapter inside `nvapi.rs` so the rest of the codebase keeps
  degrees and percent.

## 3. Apply ordering + result type

- New order: ICC → gamma LUT → NVAPI vibrance/hue (vendor overlay
  last).
- Skip rule: `vibrance == 50.0 && hue_deg == 0.0` performs no NVAPI
  call, so pre-existing presets behave byte-identically to today.
- `ApplyResult` gains `vibrance_applied: bool`. `offline()` and
  `success()` constructors updated; error composition extended with
  `"; vibrance: {e}"`. TypeScript `ApplyResult` and card feedback
  updated alongside — no divergent shapes.
- Partial failure follows the established pattern: gamma may succeed
  while vibrance fails (or is unavailable); everything is reported,
  nothing is swallowed.

## 4. Frontend

- `PresetEditor`: Vibrance slider (0–100, default 50) and Hue slider
  (−180–180, default 0) in the color group. Support probe
  (`vibranceSupported(edidId)`, camelCase args) runs on editor open
  and on monitor change; while probing or when unsupported, both
  sliders disable with the note: "Digital vibrance/hue need an
  NVIDIA-driven display."
- `PresetCard`: displays vibrance/hue values; apply feedback includes
  the vibrance outcome.
- `src/lib/tauri.ts` gains `vibranceSupported`; no changes to existing
  wrappers.

## 5. Error handling

- Offline monitor: existing `ApplyResult::offline()` short-circuit,
  no driver calls.
- NVAPI unavailable per display: apply reports
  `"NVAPI unavailable for this display"` for the vibrance step;
  gamma/ICC steps proceed normally.
- Win32/NVAPI failures carry their native error codes in messages
  (established by the gamma-fix commits).

## 6. Testing

- Rust (mocked, no hardware): skip-when-neutral issues zero NVAPI
  calls; offline issues zero driver calls; vibrance failure still
  reports gamma success; support-probe false paths; serde migration
  test loading pre-feature JSON (missing fields → 50.0/0.0);
  validation rejects out-of-range vibrance/hue.
- TypeScript (vitest): form validation for the new ranges.
- Hardware (this machine): external AOC expected supported —
  apply must succeed end-to-end; internal panel exercises either the
  supported or the disable-with-note path depending on its driver
  routing; both outcomes verified live.
- Gate before merge: `cargo test`, `npx tsc --noEmit`,
  `npx vitest run`, `npm run tauri build` all green.

## Accepted risks

- NVAPI header details (struct revisions, hue units) confirmed at
  implementation; adapter normalizes differences.
- Floor/validation quirks of the NVAPI path on real drivers are
  discovered during hardware verification, as with the gamma LUT
  floor (0x8000).
