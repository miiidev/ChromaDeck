# NVCP-Native Color Engine — Design Spec

**Status:** approach A + sections 1–6 approved in chat (2026-09-29);
awaiting spec review.
**Approach:** A — NVCP transfer math (reimplemented) + `SetTargetGammaCorrection`
+ driver registry persistence. Rejected: GDI curve-fitting (B), external
tooling (C).

## Goal

Make ChromaDeck brightness/contrast/gamma behave exactly like NVIDIA
Control Panel by using NVIDIA's own transfer function, delivery API,
and persistence — replacing our gain-model GDI engine on NVAPI-capable
displays. Vibrance/hue (already exact via DVC) are untouched.

## Background (spike findings, verified)

- NVCP B/C/G have no settable "value" API: NVCP itself computes ramp
  tables and pushes them driver-side. Parity = same math + same API.
- Transfer function (behavioral reimplementation of nvBrightness's
  `CalculateGamma`; that codebase is GPL-3.0 — NO code copied, math
  reimplemented with attribution comment):
  `contrast_c = (c−100)/100; shaped = c≤0 ? (1+c)(x−0.5) : (x−0.5)/(1−c);`
  `v = (b−100)/100 + shaped + 0.5, clamped; out = pow(v, 1/(g/100)), clamped`,
  with `x = i/1023`, per channel, output float 0..1.
- Scale: internal 80–120, **100 = normal**. NVCP UI 0–100% maps as
  `internal = 80 + ui×0.4` (50% ⟺ 100; bounds match exactly).
  Brightness below normal crushes toward black (offset model) — it can
  never lift blacks, so the floor-lift fog class disappears by
  construction.
- Delivery: undocumented `NvAPI_DISP_SetTargetGammaCorrection(displayId,
  NV_GAMMA_CORRECTION_EX)` (QueryInterface `0x7082A053`).
  Struct: `{ version: u32, gammaRampEx: [f32; 3072], unknown: u32 }`,
  `version` = computed `sizeof | (1<<16)`, layout channel-interleaved
  `[i*3+c]`, `unknown = 1`.
- Display targeting: existing NVAPI displayId resolution; LUID via
  `NvAPI_SYS_GetLUIDFromDisplayID` (`0xD4A859F2`), LUID =
  `((u32*)&guid)[1] ^ 0xF0000000`.
- Persistence: `HKCU\Software\NVIDIA Corporation\Global\NVTweak\`
  `Devices\<luid>-0\Color\<3538946+i>`, i in 0..9 (attrs
  brightness/contrast/gamma × colors R/G/B), DWORDs, plus
  `NvCplGammaSet = 1`. Verified present on this machine (all 100 =
  defaults). The driver re-enforces these itself across reboot/mode-set.
- GDI `SetDeviceGammaRamp` path stays as the automatic fallback for
  non-NVAPI displays (e.g. iGPU-driven panels).

## 1. Scales + math

- UI sliders: brightness/contrast 0–100% (50 neutral) → internal
  `80 + ui×0.4`; gamma stays 1.0–3.0 exponent → internal `×100`.
- RGB gains kept as post-multiplier on the computed float ramp
  (backward compatible, clamped 0..1).
- Vibrance (0–100/50), hue (0–359/0), ICC: unchanged.
- New `nvgamma.rs`: pure `calculate Bramwell` + 1024-entry ramp builder.
  Unit-pinned vectors: all-100 → identity ramp; UI↔internal mapping
  (60% ⟺ 88).

## 2. NVAPI surface

- `nvgamma.rs` owns math, ramp builder, registry persist/capture.
- `nvapi.rs` loader gains exactly: `SetTargetGammaCorrection`
  (`0x7082A053`), `GetLUIDFromDisplayID` (`0xD4A859F2`).
- Engine selection per display at apply time: NVAPI gamma available →
  NVCP path; else GDI fallback (existing tests stay green untouched).
- Attribution comment in `nvgamma.rs` naming the behavioral source
  (nvBrightness by Pete Batard) without reproducing its code.

## 3. Registry persist + capture

- Every NVCP-path gamma apply writes the 9 DWORDs + `NvCplGammaSet=1`
  under the display's current `<luid>-0\Color` (LUID resolved per
  display, per apply — LUIDs can change across configurations).
- New "Import NVCP state": reads those keys (missing/absent =
  neutral) + live DVC vibrance/hue → builds a preset. Exact for all
  fields — the feasible form of "capture current".

## 4. Migration

- On first load, existing presets' brightness/contrast reset to 50
  (no faithful gain→NVCP mapping exists; conversion would lie).
  Gamma/RGB/vibrance/hue/ICC untouched (gamma scale unchanged).
- `presets.json` backed up to `presets.json.bak-<date>` before the
  rewrite. One-way, logged.

## 5. Apply / reset / enforcer deltas

- Capable displays → NVCP path; others → GDI fallback (auto-selected).
- Reset writes internal 100s + vibrance 50 + hue 0 + registry (driver
  agrees; full default).
- Enforcer: gamma drift-check becomes tolerance-based (1024-float
  driver ramp vs 256-entry read cannot memcmp); vibrance/hue stay
  exact. Driver self-enforcement is a backstop, not a replacement —
  verification stays ours.

## 6. Testing

- Unit: known vectors (all-100 → identity; UI↔internal mapping both
  directions); mocks for new paths; migration (old file → neutral
  B/C + backup written).
- Hardware (this machine): prediction NVCP-60% ⟺ registry 88;
  app-set-60 ⟺ NVCP UI shows 60%; clobber-restore through the new
  path; GDI fallback still green on existing tests.
- Gate before merge: `cargo test`, `npx tsc --noEmit`,
  `npx vitest run`, `npm run tauri build` all green.

## Accepted risks

- Undocumented API (stability across drivers — mitigated: support
  probe + GDI fallback; same posture as existing DVC integration).
- NVCP UI↔internal mapping inferred from bounds + author statements;
  verified live during implementation (acceptance test above).
- GPL hygiene: behavioral reimplementation only, with attribution.
