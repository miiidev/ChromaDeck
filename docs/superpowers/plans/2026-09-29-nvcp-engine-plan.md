# NVCP-Native Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the gain-model GDI color engine with NVIDIA's own transfer math, delivery API, and registry persistence on NVAPI-capable displays, keeping GDI as automatic fallback.

**Architecture:** New `nvgamma.rs` owns the NVCP transfer function, 1024-float ramp builder, UI↔internal scale mapping, and registry persist/capture; `nvapi.rs` gains two loader entries (`SetTargetGammaCorrection`, `GetLUIDFromDisplayID`); apply/reset/enforce branch per display on a shared engine selector; presets migrate B/C to neutral with backup.

**Tech Stack:** Rust + `windows` crate + `winreg` (already a dependency) + dynamic NVAPI (no SDK); React frontend as-is.

**Spec:** `docs/superpowers/specs/2026-09-29-nvcp-native-design.md` — the plan argues from the spec; executors read both.

## Global Constraints

- Windows 10/11 only, x64 only.
- UI scale: brightness/contrast 0–100 (50 neutral); gamma stays 1.0–3.0 exponent (1.0 neutral); vibrance 0–100/50, hue 0–359/0, RGB gains unchanged.
- Internal scale: `80 + ui×0.4` for brightness/contrast; gamma internal = exponent×100.
- No NVAPI SDK, no link-time dep; dynamic load only, graceful degradation.
- GPL hygiene: behavioral reimplementation only, attribution comment to nvBrightness, never copy its code.
- Tauri invoke args use camelCase JS keys (proven convention).
- TDD with failing tests first wherever hardware-independent; hardware proofs in Task 7.
- Commit per task with the message given; full gate before merge: `cargo test`, `npx tsc --noEmit`, `npx vitest run`, `npm run tauri build`.

---

## File Structure

- Create: `src-tauri/src/nvgamma.rs` — transfer math, ramp builder, scale maps, registry persist/capture, engine selector.
- Modify: `src-tauri/src/nvapi.rs` — 2 loader entries, gamma struct, LUID helper.
- Modify: `src-tauri/src/store.rs` — validation ranges, `color_model` stamp, backup + migration.
- Modify: `src-tauri/src/color.rs` — engine branch in apply/reset paths.
- Modify: `src-tauri/src/enforce.rs` — engine-aware drift check, registry compare, `StateReader` extension.
- Modify: `src-tauri/src/lib.rs` — register `capture_nvcp_cmd`.
- Modify: `src/lib/types.ts` — comments + `CapturedState`.
- Modify: `src/lib/tauri.ts` — `captureNvcp` wrapper.
- Modify: `src/lib/validation.ts` + test — new brightness/contrast ranges.
- Modify: `src/components/PresetEditor.tsx` — slider ranges/defaults/markers, capture button.

---

### Task 1: Scales, validation, migration

**Files:**
- Modify: `src-tauri/src/store.rs` (Preset struct + `new` + create/update validation + tests)

**Interfaces:**
- Consumes: existing `Store`, `StoreError::InvalidInput/NotFound`, `flush()` pattern.
- Produces:
  - `Preset { ..., color_model: String }` with `#[serde(default = "default_model")]`, `fn default_model() -> String { "nvcp-v1".into() }` — legacy files missing the key deserialize as `"gain-v1"` (see migration rule below, NOT via this default: implement `fn legacy_model() -> String { "gain-v1".into() }` as the serde default; new presets are stamped `"nvcp-v1"` explicitly at creation)
  - Validation: brightness/contrast `0.0..=100.0` ("...must be in 0.0..=100.0"), gamma stays `1.0..=3.0`
  - Migration in `Store::new` (exact): after loading presets, if any preset has `color_model == "gain-v1"`: copy `presets.json` to `presets.json.bak-<yyyymmdd-HHMMSS>` (local date), set those presets' brightness/contrast to `50.0` and model to `"nvcp-v1"`, then `flush()` once. New presets stamped `"nvcp-v1"` in create (ignore any input model).

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn legacy_file_migrates_brightness_contrast_to_neutral_with_backup() {
    let dir = test_store_dir_unique();
    std::fs::write(
        dir.join("presets.json"),
        r#"[{"id":"a","name":"Old","edid_id":"E","icc_hash":"","icc_filename":"","brightness":0.55,"contrast":0.5,"rgb_gains":[1.0,1.0,1.0],"gamma":1.25,"vibrance":100.0,"hue_deg":0.0}]"#,
    )
    .unwrap();
    let store = Store::new(dir.clone()).unwrap();
    let p = &store.list_presets()[0];
    assert_eq!((p.brightness, p.contrast), (50.0, 50.0));
    assert_eq!((p.gamma, p.vibrance), (1.25, 100.0)); // untouched
    assert!(std::fs::read_dir(&dir).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("presets.json.bak-")));
}

#[test]
fn create_rejects_brightness_above_100() {
    let mut input = minimal_input();
    input.brightness = 101.0;
    let err = test_store().create_preset(input).unwrap_err();
    assert!(err.to_string().contains("brightness"));
}

#[test]
fn create_stamps_nvcp_model() {
    let mut store = test_store();
    let preset = store.create_preset(minimal_input()).unwrap();
    assert_eq!(preset.color_model, "nvcp-v1");
}
```

(`test_store_dir_unique()` / `test_store()` helpers exist per the enforcer plan's Task 1; reuse them. `minimal_input()` must set brightness/contrast to in-range UI values — update it to `brightness: 55.0, contrast: 60.0` as part of this task, plus update every other test literal using gain-scale values: `create_preset_rejects_bad_brightness` (1.5 → 101.0), `update_preset_changes_fields` (0.9/0.3 → 55.0/60.0), persist test (0.3/0.6 → 30.0/60.0). Gamma literals (2.2/2.0/2.5/0.5-reject) stay valid — do not touch.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test migration_ 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (no `color_model` field — red state).

- [ ] **Step 3: Implement** (struct fields + defaults + validation ranges + `new` migration + create stamp, exactly as specified above)

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass (63 + new/migrated tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/store.rs
git commit -m "feat: NVCP-scale validation with legacy preset migration"
```

---

### Task 2: NVCP math, ramp, loader entries

**Files:**
- Create: `src-tauri/src/nvgamma.rs` (pure math + maps; NO Win32 here)
- Modify: `src-tauri/src/nvapi.rs` (loader + struct + LUID helper)

**Interfaces:**
- Consumes: existing loader pattern (`load_nvapi`, `NvapiFns`, `OnceLock`, named ID consts), `resolve_device_name` (exists, `pub(crate)`).
- Produces:
  - `pub fn ui_to_internal(ui: f64) -> f64 { 80.0 + ui * 0.4 }`, `pub fn internal_to_ui(v: f64) -> f64 { (v - 80.0) / 0.4 }`
  - `pub fn nvcp_ramp_value(index: u16 /*0..1023*/, brightness: f64 /*internal*/, contrast: f64 /*internal*/, gamma_exp: f64) -> f32` implementing exactly: `c=(contrast-100)/100; shaped = if c<=0 {(c+1)(x-0.5)} else {(x-0.5)/(1-c)}; v=(brightness-100)/100+shaped+0.5 clamp; out=pow(v,1/gamma_exp) clamp` with `x=index/1023`
  - `pub fn nvcp_ramp(brightness, contrast, gamma_exp: f64) -> [f32; 3072]` channel-interleaved `[i*3+c]`
  - `pub(crate) struct NvGammaRampEx { version: u32, ramp: [f32; 3072], unknown: u32 }` with version computed `(size|1<<16)` (proves 0x13008 in test)
  - Loader: `SetTargetGammaCorrection` (`0x7082A053`, sig `(u32, *const NvGammaRampEx) -> i32`), `GetLUIDFromDisplayID` (`0xD4A859F2`, sig `(u32, u32 /*1*/, *mut windows::core::GUID) -> i32`), `pub(crate) fn display_luid(fns, display_id) -> Result<u32>` computing `u32::from_le_bytes(guid bytes 4..8) ^ 0xF0000000`
  - `pub(crate) fn set_target_gamma(fns, display_id, ramp: &[f32; 3072]) -> Result<(), String>` (builds struct, calls, maps status via existing `status_to_string`)

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn neutral_inputs_yield_identity_float_ramp() {
    let ramp = nvcp_ramp(100.0, 100.0, 1.0);
    for i in 0..1024 {
        let expected = i as f32 / 1023.0;
        for c in 0..3 {
            assert!((ramp[i * 3 + c] - expected).abs() < 1e-6, "index {i}");
        }
    }
}

#[test]
fn ui_internal_mapping_roundtrips() {
    assert_eq!(ui_to_internal(50.0), 100.0);
    assert_eq!(ui_to_internal(0.0), 80.0);
    assert_eq!(ui_to_internal(100.0), 120.0);
    assert_eq!(internal_to_ui(104.0), 60.0);
    assert_eq!(internal_to_ui(ui_to_internal(60.0)), 60.0);
}

#[test]
fn below_normal_crushes_never_lifts() {
    // brightness 90 (< 100): peak must stay below 1.0 AND index 0 must be 0
    let ramp = nvcp_ramp(90.0, 100.0, 1.0);
    assert!(ramp[1023 * 3] < 1.0);
    assert_eq!(ramp[0], 0.0, "black stays black (no fog lift)");
}

#[test]
fn gamma_struct_version_is_computed() {
    assert_eq!(
        (std::mem::size_of::<NvGammaRampEx>() as u32) | (1 << 16),
        0x13008
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test nvgamma 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (no `nvgamma` module — red state).

- [ ] **Step 3: Implement** (math + maps + struct + loader entries + LUID helper, exactly as specified; attribution comment naming nvBrightness as the behavioral source with NO copied code; wire `mod nvgamma;` into lib.rs — one line, nothing else there)

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/nvgamma.rs src-tauri/src/nvapi.rs src-tauri/src/lib.rs src-tauri/Cargo.lock
git commit -m "feat: NVCP transfer math with gamma correction API surface"
```

---

### Task 3: Registry persist + NVCP capture

**Files:**
- Modify: `src-tauri/src/nvgamma.rs` (persist + read fns)
- Modify: `src-tauri/src/lib.rs` (register `capture_nvcp_cmd` only)

**Interfaces:**
- Consumes: `display_luid` (Task 2), `winreg` crate (already a dependency; mirror `monitor.rs` read style), DVC/hue getters (existing `read_levels` in nvapi.rs).
- Produces:
  - `pub(crate) fn persist_nvcp(display_id: u32, b: [f64; 3], c: [f64; 3], g: [f64; 3] /*internal*/) -> Result<(), String>` writing DWORDs `3538946+i` + `NvCplGammaSet=1` under `HKCU\Software\NVIDIA Corporation\Global\NVTweak\Devices\<luid>-0\Color` (create keys as needed; registry value base `3538946` per live-driver observation)
  - `pub(crate) fn read_nvcp(luid: u32) -> (f64, f64, f64) /*internal b/c/g, channel means; missing keys = 100.0*/`
  - `pub struct CapturedState { pub brightness: f64, pub contrast: f64, pub gamma: f64, pub vibrance: f64, pub hue_deg: f64 }` (UI scales: B/C 0–100, gamma exponent, vibrance 0–100, hue 0–359; derive Debug, Clone, Serialize) + `pub fn capture_nvcp(edid_id: &str) -> Result<CapturedState, String>` (registry means→UI via `internal_to_ui`, gamma internal/100; rgb gains intentionally left at 1.0 — documented; DVC/hue via existing `read_levels`)
  - `#[tauri::command] pub fn capture_nvcp_cmd(edid_id: String) -> Result<CapturedState, String>`

- [ ] **Step 1: Write failing tests** (registry tests use a scratch subkey, never the real NVTweak path: parameterize with `fn nvtweak_color_key(luid: u32) -> String` + `#[cfg(test)] fn test_key() -> String` pointing at `HKCU\Software\ChromaDeckTest\<luid>-0\Color`; delete the scratch tree at test end)

```rust
#[test]
fn persist_then_read_roundtrips() {
    let luid = 0xC0FFEE;
    persist_nvcp_test(&[90.0, 100.0, 110.0], &[100.0; 3], &[100.0; 3], luid).unwrap();
    let (b, c, g) = read_nvcp_test(luid);
    assert_eq!((b, c, g), (100.0, 100.0, 100.0)); // channel means
    delete_test_tree();
}

#[test]
fn missing_keys_read_as_neutral() {
    let (b, c, g) = read_nvcp_test(0xDEAD);
    assert_eq!((b, c, g), (100.0, 100.0, 100.0));
}
```

(Implement `persist_nvcp`/`read_nvcp` over an internal `*_at(base_key: &str, ...)` so tests inject the scratch base; production passes the real NVTweak base. Exact seam, no test-only branching in production code.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test persist_then_read 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (no such fns — red state).

- [ ] **Step 3: Implement** (exact fns + command + lib.rs registration of `capture_nvcp_cmd` only)

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/nvgamma.rs src-tauri/src/lib.rs
git commit -m "feat: NVCP registry persistence and state capture"
```

---

### Task 4: Apply path rewrite (engine branch)

**Files:**
- Modify: `src-tauri/src/color.rs` (engine selection in apply/reset paths; GDI fallback mapping)

**Interfaces:**
- Consumes: `nvgamma::{ui_to_internal, nvcp_ramp, persist_nvcp}`, `nvapi::{display_luid-ish resolution, set_target_gamma}`, `build_gamma_ramp` (GDI fallback, unchanged).
- Produces: identical public signatures (`apply_preset`, `apply_color`, `reset_monitor`, commands); behavior change only in computed values on NVAPI displays.

- [ ] **Step 1: Red state via behavior test (no signature changes, so compile-fail red is impossible — use a failing BEHAVIOR test instead)**

```rust
#[test]
fn nvcp_engine_selection_prefers_nvapi_path() {
    // Pure helper under test:
    assert_eq!(select_gamma_engine_for_test(true), GammaEngine::Nvapi);
    assert_eq!(select_gamma_engine_for_test(false), GammaEngine::Gdi);
}
```

with `#[derive(Debug, PartialEq)] pub enum GammaEngine { Nvapi, Gdi }` and `fn select_gamma_engine_for_test(nvapi_available: bool) -> GammaEngine` as the placeholder seam — implement the real `pub(crate) fn gamma_engine(edid_id: &str) -> GammaEngine` (NVAPI gamma resolves AND displayId resolves → Nvapi, else Gdi) and rewrite the test against it with injected bools via a `#[cfg(test)]` constructor. Concretely: implement `fn engine_for_flags(nvapi_ok: bool) -> GammaEngine` (pure, tested above) called by `gamma_engine()` after probing; test the pure fn. (Exact seam satisfying TDD without hardware.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test engine_for_flags 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (no such items — red state).

- [ ] **Step 3: Implement** (exact)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GammaEngine { Nvapi, Gdi }

fn engine_for_flags(nvapi_ok: bool) -> GammaEngine {
    if nvapi_ok { GammaEngine::Nvapi } else { GammaEngine::Gdi }
}

pub(crate) fn gamma_engine(edid_id: &str) -> GammaEngine {
    // NVAPI gamma path available? = loader resolves SetTargetGammaCorrection
    // AND a displayId resolves for this edid (reuse nvapi resolve helpers).
    // Any failure → Gdi. No driver writes in this probe.
    ...exact...
}
```

GDI fallback mapping (exact, documented best-effort): `b_gain = 0.8 + 0.004 * ui_b`, `c_gain = 0.8 + 0.004 * ui_c`, gamma exponent as-is → existing `build_gamma_ramp(b_gain, c_gain, rgb_gains, gamma)`. (Derivation: NVCP internal/100 at neutral=1.0.)

NVAPI apply path (exact, in `apply_color`/`apply_preset` gamma step position and in `reset_monitor`): convert UI→internal, build 1024-float ramp (per-channel: same B/C/G all channels — RGB gains applied as post-multiplier then clamp, preserving existing gain behavior), `set_target_gamma`, then `persist_nvcp` (failures: gamma error aborts with win32/NVAPI message; persist failure appends `"; persist: {e}"` but does NOT fail the apply). Reset writes internal 100s + registry (full default the driver agrees with).

Keep `set_gamma_ramp` (GDI) intact for fallback + `blank_channel_error` guard on the GDI path only.

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass (existing GDI-path tests unaffected — fallback math lives beside, not inside, `build_gamma_ramp`).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/color.rs
git commit -m "feat: NVCP-native gamma path with GDI fallback"
```

---

### Task 5: Enforcer engine-awareness

**Files:**
- Modify: `src-tauri/src/enforce.rs` (`StateReader` extension, engine branch in `check_once`)

**Interfaces:**
- Consumes: `gamma_engine`, NVCP ramp + downsample, registry read, existing mocks (+ extended).
- Produces: unchanged `check_once` signature; updated drift semantics.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn nvapi_display_verifies_via_registry_not_lut() {
    // MockStateReader gains `nvcp: Option<(f64, f64, f64)>` (internal B/C/G)
    // and `lut: Option<[u16; 768]>`; a registry MATCH (100s for a neutral
    // preset) with a garbage LUT must NOT count as drift on an NVAPI display.
    // (Engine mocked as Nvapi via a `force_engine: Option<GammaEngine>` field
    // on the mock reader — exact seam, no hardware.)
}
```

(Implement `MockStateReader { lut, color, fail }` + `nvcp_registry: Option<(f64,f64,f64)>` + `engine: GammaEngine`; `StateReader` gains `fn read_nvcp_registry(&self, edid_id: &str) -> Option<(f64, f64, f64)>` and `fn engine(&self, edid_id: &str) -> GammaEngine`; real impls delegate to `gamma_engine` + registry read. GDI branch keeps today's exact memcmp logic.)

Concretely add: `nvapi_registry_match_is_not_drift`, `nvapi_registry_mismatch_reapplies`, `gdi_branch_unchanged` (existing 7 tests cover the last — keep them green untouched).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test nvapi_registry 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (no such methods — red state).

- [ ] **Step 3: Implement** (exact branch)

```rust
// GDI branch (unchanged): lut memcmp vs build_gamma_ramp(mapped gains).
// NVAPI branch: registry (b,c,g internal) must equal preset's converted
//   values (ui_to_internal each) AND live GDI LUT must be identity
//   (NVAPI path owns color; a foreign LUT writer is drift → restore
//   identity via the GDI path to keep stages pure) AND vibrance/hue exact.
// Mismatch anywhere → reapply via apply_color (which re-selects engine).
```

Downsample helper for possible future use is OUT OF SCOPE — registry compare replaces tolerance compare (simpler, exact). (Supersedes the tolerance idea from planning notes; rationale recorded here.)

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/enforce.rs
git commit -m "feat: engine-aware enforcer drift detection"
```

---

### Task 6: Frontend (scales, capture, markers)

**Files:**
- Modify: `src/lib/validation.ts` (+ test), `src/lib/types.ts` (comments + `CapturedState`), `src/lib/tauri.ts` (`captureNvcp`), `src/components/PresetEditor.tsx` (ranges, defaults stay, markers, capture button)

**Interfaces:**
- Consumes: `capture_nvcp_cmd(edid_id)` → `CapturedState`.
- Produces: NVCP-scale UI. No Rust changes.

- [ ] **Step 1: Write failing tests** (append to `src/lib/validation.test.ts`)

```ts
test("rejects brightness/contrast outside 0-100", () => {
  expect(validatePreset({ brightness: 101 }).brightness).toContain("0 and 100");
  expect(validatePreset({ contrast: -1 }).contrast).toBeDefined();
  expect(validatePreset({ brightness: 50 }).brightness).toBeUndefined();
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `npx vitest run 2>&1 | Select-String "Tests |FAIL"` (from repo root)
Expected: FAIL (current messages say "between 0 and 1" — red state).

- [ ] **Step 3: Implement** (exact)

`validation.ts`: brightness message → `"Brightness must be between 0 and 100"`, range `0..100`; contrast likewise; gamma/rgb/vibrance/hue untouched. `validatePresetForm` unchanged (already passes fields through).

`types.ts`: update `Preset`/`PresetInput` comments (`brightness: number; // 0–100 UI, 50 neutral` etc.) + append:

```ts
export interface CapturedState {
  brightness: number; // 0–100 UI
  contrast: number;   // 0–100 UI
  gamma: number;      // exponent
  vibrance: number;
  hue_deg: number;
}
```

`tauri.ts` append:

```ts
/** Read NVCP/driver live state into editor-fillable values. */
export async function captureNvcp(edidId: string): Promise<CapturedState> {
  return invoke<CapturedState>("capture_nvcp_cmd", { edidId });
}
```

`PresetEditor.tsx`:
- Brightness slider: `min={0} max={100} step={1}`, value display `toFixed(0)`; footer `<span>0</span><span>50 (neutral)</span><span>100</span>`.
- Contrast slider: same range/display/footer change.
- Gamma/RGB/vibrance/hue sliders + `DEFAULT_INPUT` unchanged (already neutral: verify — DEFAULT_INPUT is now brightness 1.0/contrast 1.0/gamma 1.0 per the neutral-defaults fix; UPDATE it to brightness 50, contrast 50, gamma 1.0).
- Markers: change existing `(neutral)` tags — brightness/contrast max-end tags move to middle (`50 (neutral)`); gamma `1.0 (neutral)` stays; RGB/vibrance/hue untouched.
- Capture button next to the monitor select (exact): a `Browse…`-style button labeled `Import NVCP state`, disabled when no monitor selected or while importing; on click calls `captureNvcp(form.edid_id)`, fills brightness/contrast/gamma/vibrance/hue_deg (rgb_gains untouched), surfaces errors in `errors.icc_path`-style field `errors.nvcp` (extend `ValidationErrors` with `nvcp?: string`).

- [ ] **Step 4: Run checks**

Run: `npx tsc --noEmit; echo TSC; npx vitest run 2>&1 | Select-String "Tests |Test Files "` (from repo root)
Expected: tsc clean, 16 + new tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib/validation.ts src/lib/validation.test.ts src/lib/types.ts src/lib/tauri.ts src/components/PresetEditor.tsx
git commit -m "feat: NVCP-scale sliders with state capture"
```

---

### Task 7: Hardware verification + release build

**Files:** none (verification only).

- [ ] **Step 1: Unit gates re-run**

Run: `cargo test` (src-tauri), `npx tsc --noEmit`, `npx vitest run` (root).
Expected: all green (report exact counts).

- [ ] **Step 2: Mapping predictions (the money tests)**

With NVCP closed: set NVCP brightness to 60% manually, read
`HKCU\...\NVTweak\Devices\<luid>-0\Color` → expect the brightness
DWORDs to read **104**. Set back to 50% → expect **100**. (Validates
`internal = 80 + ui×0.4` against the driver of record.)

- [ ] **Step 3: Round-trip through the app**

Via dev app: create preset (brightness 60, rest neutral), Apply →
NVCP UI must show 60% → registry reads 104 → GDI LUT read shows the
NVCP-shaped ramp (NOT the old gain shape). Then Reset → NVCP UI back
to 50%, registry 100s.

- [ ] **Step 4: Clobber-restore + capture**

Externally clobber (script-set neutral LUT + vibrance 50 on a pinned
non-neutral preset) → auto-restore within 15s. Then run Import NVCP
state → form fills with live values (spot-check against NVCP UI).

- [ ] **Step 5: Regression sweep**

Old bugs stay fixed: brightness-0 preset → actionable error (not
Win32 failure); neutral preset → zero NVAPI writes; GDI fallback path
untouched per existing suite; delete/pin/reset flows unchanged.

- [ ] **Step 6: Release build**

Run: `npm run tauri build` (repo root). Expected: MSI + NSIS clean.

- [ ] **Step 7: Report (no commit — no code changes)**

Report per-step outcomes. On ANY failure: STOP, return evidence, do
not stack fixes.
