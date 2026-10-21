# Plan: Universal Display Color Management (ChromaDeck vendor-neutral evolution)

Date: 2026-10-19
Status: IMPLEMENTED (Phases A-G complete; see `docs/superpowers/specs/2026-10-19-intel-igcl-design.md` for hardware evidence)
Baseline verification (recorded before any change):
- `npm test` -> 109 tests passed (6 files)
- `cargo test --quiet` (workdir `src-tauri`) -> 128 tests passed

## 0. Research evidence (Phase A complete)

- Repo audit: clean tree on `master`, latest `efe0ee7 release: v0.8.1`. No AGENTS.md/CLAUDE.md.
- Dev machine is an ideal test rig: Intel UHD (PCI\VEN_8086&DEV_A7A8, driver 32.0.101.7088) + NVIDIA RTX 3050 6GB laptop GPU (hybrid graphics).
- Intel IGCL is PRESENT AND LOADABLE on this machine (verified by direct LoadLibrary/GetProcAddress probe):
  - `C:\Windows\System32\DriverStore\FileRepository\iigd_dch.inf_amd64_*\ControlLib.dll` and `IntelControlLib.dll` both export:
    `ctlInit`, `ctlClose`, `ctlEnumerateDevices`, `ctlEnumerateDisplayOutputs`, `ctlGetDisplayProperties`,
    `ctlPixelTransformationGetConfig`, `ctlPixelTransformationSetConfig`, `ctlPanelDescriptorAccess`, `ctlSetBrightnessSetting`.
    (`ctlDisplayFeatureReset` is NOT exported on this driver — verified in the spike.)
  - Probe DLL load order: `ControlLib.dll`, then `IntelControlLib.dll` fallback.
- Intel official SDK: https://github.com/intel/drivers.gpu.control-library (header `include/igcl_api.h`, ~8333 lines; docs at https://intel.github.io/drivers.gpu.control-library/).
  - License: Intel Software License Agreement 10.07.21 - redistribution permitted with notice, "solely for use on use on Intel platforms", no reverse engineering of binaries; header explicitly redistributable. Transcribe ONLY the minimal structs needed (same pattern as MIT-transcribed `NVAPIWrapper` in `nvapi.rs`), with attribution.
  - Relevant APIs: `ctlPixelTransformationGetConfig`/`SetConfig` (1D LUT + 3D LUT + 3x3 matrix + `CTL_PIXTX_CONFIG_OPERTATION_TYPE_RESTORE_DEFAULT` - note header typo "OPERTATION", preserve verbatim), `ctlDisplayFeatureReset` with `CTL_DISPLAY_FEATURE_RESET_FLAG_COLOR`, `ctlEnumerateDevices`/`ctlEnumerateDisplayOutputs`/`ctlGetDisplayProperties`/`ctlGetDeviceProperties`.
  - NOT usable: video-processing brightness/contrast/hue/saturation APIs (video-content only, not desktop). `ctlSetBrightnessSetting` is "companion display" only.
- Windows-native fallbacks (already partially in app): `SetDeviceGammaRamp` (gamma-family only, SDR, bypassed by exclusive fullscreen), GDI identity purge, ICC association (already GPU-vendor-neutral). HDR 3DLUT explicitly unsupported per IGCL error codes.
- Open question carried into Phase E: correlation between `ctl_os_display_encoder_identifier_t.WindowsDisplayEncoderID` (u32) and Windows `\\.\DISPLAYn` / EDID. No LUID in header. Spike required.

## 1. Requirements restatement

1. ChromaDeck evolves from NVIDIA-oriented tool to universal display color manager, without regressions.
2. ONE shared preset editor for all vendors; no vendor-specific modals; per-adjustment capability indicators.
3. New `ColorBackend` abstraction: discovery, capabilities, apply with per-adjustment results, restore.
4. Display identity (EDID, `monitor.rs`) kept separate from GPU identity (driver handles); hybrid-graphics aware.
5. Capability resolution is per-adjustment, not per-vendor.
6. Preserve NVIDIA behavior + existing preset semantics (`Preserve NVIDIA behavior and existing preset semantics`).
7. Prefer Intel-native interface when available (IGCL confirmed available); AMD explicitly out of scope this iteration (trait leaves a slot).
8. HDR: documented limitation. Games/exclusive fullscreen: best effort.
9. Restore/reset robust: app start, app exit, drift-check reapply loop (`enforce.rs`) all go through the abstraction.
10. All existing tests keep passing (109 vitest + 128 cargo baseline); new logic is test-first.

## 2. Target architecture

- `src-tauri/src/backend/mod.rs` (NEW): `trait ColorBackend { id(); probe(display) -> Vec<AdjustmentCap>; apply(preset, target) -> ApplyResult; restore(target); capture(target); }`
  - `AdjustmentCap = Supported | Unsupported(&str) | Approximate(&str)` per adjustment name (gamma, brightness, contrast, rgb_gains, saturation, vibrance, hue).
- Existing code wrapped, not rewritten:
  - `nvapi.rs` -> `NvapiBackend` (NVAPI display id resolution already exists via `display_id_for_device`).
  - Gamma/GDI path in `color.rs` -> `GdiBackend` (universal fallback: gamma-family only).
  - ICC association stays GPU-level in the orchestrator (`apply_preset`), unchanged semantics.
- `backend/intel.rs` (NEW): dynamic-load IGCL, FFI structs transcribed from `igcl_api.h`.
  - Apply math: gamma/brightness/contrast -> per-channel 1D LUT; rgb_gains -> matrix diagonal; saturation -> luma-lerp 3x3 matrix; hue -> YIQ-style rotation 3x3 matrix; composition order documented in code.
  - Restore: `ctlPixelTransformationSetConfig(RESTORE_DEFAULT)` to reset the matrix, then re-SET the captured pre-apply 1D LUT and verify it by read-back. `ctlDisplayFeatureReset` is NOT exported on this driver, so it is not used.
- Orchestrator (`color.rs::apply_preset`) selects backend per display: NVAPI if display resolved to NVIDIA, else Intel IGCL if resolved to Intel, else GDI. Per-adjustment results returned in `ApplyResult` (already shaped for this).
- Display/GPU correlation: NVIDIA keeps `\\.\DISPLAYn` -> NVAPI id mapping; Intel resolves at runtime from `WindowsDisplayEncoderID` (spike-gated; fallback rule if correlation is ambiguous: apply only when the target is the sole active Intel-attached output, otherwise report Unsupported with reason).
- Schema: add `saturation` (f64, UI 0-100, neutral 50) to `Preset`/`PresetInput`. NVIDIA reports saturation Unsupported (vibrance stays NVIDIA-only); Intel reports vibrance Unsupported, saturation Supported. Missing field in existing `presets.json` migrates to 50 (test-covered).
- Frontend: `PresetEditor.tsx` + `ApplyDialog.tsx` gain capability-driven dimming/tooltips from new `get_display_capabilities` command; hardcoded NVIDIA copy at `PresetEditor.tsx:470` replaced.

## 3. Implementation phases

### Phase B - Backend abstraction (behavior-preserving refactor) [L]
- B1. Create `src-tauri/src/backend/mod.rs` with `ColorBackend`, `AdjustmentCap`, `BackendRegistry` (per-display backend pick). New unit tests for selection logic. Deps: none.
- B2. Adapt `nvapi.rs` -> `NvapiBackend` and `color.rs` gamma/GDI -> `GdiBackend`; `apply_preset` delegates to registry; ICC path untouched. Deps: B1.
- B3. Keep the `enforce.rs` drift check on its `StateReader` seam; it skips Intel (no pixtx read-back) and re-applies through `apply_color`, which selects the backend per display. Only an explicit Reapply heals Intel. Deps: B2.
- B4. Gate: all 128 cargo + 109 vitest green; add registry unit tests. Deps: B3.

### Phase C - Capability resolver + schema [M]
- C1. `src-tauri/src/capabilities.rs`: per-adjustment tables per backend + live probes (wrap existing `vibranceSupported`). Deps: B1.
- C2. Tauri command `get_display_capabilities(edid_id)` registered in `lib.rs`; mirror in `src/lib/tauri.ts` + `types.ts`. Deps: C1.
- C3. `store.rs`: add `saturation` field + default/neutral helpers + migration test for legacy `presets.json` (no field -> 50). Deps: none (parallel with B).
- C4. Gate: `npm test` (new store/capability tests) + `cargo test` green; `npm run build` (tsc) clean.

### Phase D - Unified editor UI [M]
- D1. `src/components/ApplyDialog.tsx`: target display selector shows detected backend label (NVIDIA / Intel / Basic) from capabilities command. Deps: C2.
- D2. `src/components/PresetEditor.tsx`: per-slider support state (dim + tooltip reason) replaces hardcoded NVIDIA note (line ~470); add saturation slider; keep single shared editor (no vendor modals). Deps: D1, C3.
- D3. Vitest component tests for capability dimming + saturation field round-trip. Deps: D2.

### Phase E - Intel IGCL backend [L]
- E1. Spike (throwaway probe crate or gated test): load `ControlLib.dll`/`IntelControlLib.dll`, enumerate devices/outputs, dump `WindowsDisplayEncoderID` + `ctl_display_properties_t` next to `EnumDisplayDevices`/`QueryDisplayConfig` on this machine; record correlation findings in `docs/superpowers/specs/2026-10-19-intel-igcl-design.md`. Deps: B1. Risk gate for E2-E4.
- E2. `src-tauri/src/backend/intel.rs`: FFI layer (LoadLibrary/GetProcAddress pattern copied from `nvapi.rs`), minimal `#[repr(C)]` structs transcribed from `igcl_api.h`, version/size fields set per header rules. Deps: E1.
- E3. `IntelBackend` impl: probe (pixtx get), apply (LUT+matrix builder), restore (restore-default + feature reset). Deps: E2, B1.
- E4. Math unit tests: hue rotation matrix, saturation luma-lerp, brightness/contrast/gamma LUT generation, composition order; restore-default path test with mock FFI. Deps: E3.
- E5. Hardware verification checklist (manual, this machine): internal panel + external output, apply/reset round-trip, drift reapply loop, HDR on/off behavior documented. Deps: E4, D2.

### Phase F - Lifecycle/restore hardening [M]
- F1. Startup: on launch, current preset reapplied via registry; driver restore used when preset removed/reset (parity with existing `reset_monitor`). Deps: B3, E3.
- F2. Exit/tray-quit: restore semantics defined per backend (document + implement). Deps: F1.
- F3. Surface per-adjustment `ApplyResult` in UI (skipped adjustments reported, not silent). Deps: D2, E3.

### Phase G - Docs + verification [S]
- G1. `PRODUCT.md` + docs: vendor-neutral positioning, capability matrix, HDR limitation, exclusive-fullscreen best-effort, Intel license attribution (`NOTICE` file for transcribed header structs). Deps: all.
- G2. Full gate: `npm test`, `npm run build`, `cargo test`, manual checklist E5. Deps: G1.

## 4. Dependencies (summary)

- B1 blocks B2/B3, C1, E1.
- C2 blocks D1; C3 parallel with B; D2 blocks F3.
- E1 gates all Intel work (correlation risk); E3 blocks F1/F2.
- G last. Critical path: B1 -> E1 -> E2 -> E3 -> E5 -> G2.

## 5. Risks and mitigations

- HIGH: Intel display-to-Windows correlation (`WindowsDisplayEncoderID`) unproven. Mitigation: E1 spike first; fallback = apply only when correlation unambiguous, else report Unsupported with reason (never guess).
- HIGH: Refactor of `color.rs` (1197 lines) regressing NVIDIA behavior. Mitigation: behavior-preserving wrapper design, baseline 128 tests recorded, B4 gate before any Intel work.
- MEDIUM: pixtx units/scaling (matrix vs UI 0-100, LUT entry count/format) not documented. Mitigation: derive from `Color_Samples` source + on-hardware probing in E1; golden-value tests in E4.
- MEDIUM: Intel SLA compliance for header transcription. Mitigation: transcribe minimal structs only, attribution + `NOTICE`, no binary reverse engineering, keep verbatim names incl. "OPERTATION" typo.
- MEDIUM: HDR / exclusive-fullscreen coverage unknown. Mitigation: document as limitation; capability probe returns Unsupported in HDR mode where detectable.
- MEDIUM: `presets.json` migration (new `saturation`). Mitigation: serde default + C3 migration test; legacy presets load unchanged.
- LOW: panel driven by NVIDIA vs Intel depending on MUX. Mitigation: runtime detection already handled by backend selection order; GDI fallback always available.
- LOW: two candidate Intel DLLs. Mitigation: probe chain `ControlLib.dll` -> `IntelControlLib.dll` (both verified exporting required symbols).

## 6. Estimated complexity

- Phase B: L (~1-2 days) - refactor with high regression risk, mitigated by baseline tests.
- Phase C: M (~0.5-1 day).
- Phase D: M (~0.5-1 day).
- Phase E: L (~2-3 days, spike-dependent) - largest unknowns.
- Phase F: M (~0.5 day).
- Phase G: S (~0.5 day).
- Overall: L (~5-8 days) - MVP scope excludes AMD, HDR LUTs, OSD/brightness-backlight control.

## 7. Testing strategy

- Unit (cargo): backend selection, capability tables, store migration, Intel math (LUT/matrix), restore sequencing (mock FFI).
- Unit (vitest): capabilities command wiring, saturation field, dimming states.
- Integration: apply/reset round-trip per backend on hardware; enforce drift loop.
- E2E manual: E5 checklist on the hybrid test machine (internal Intel panel + external output).
- Regression gate every phase: `npm test` + `cargo test` + `npm run build`.
