# NVAPI Vibrance + Hue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add NVIDIA digital vibrance (0–100) and hue (0–359°) to ChromaDeck presets via runtime-loaded NVAPI, with per-display graceful degradation.

**Architecture:** New `nvapi.rs` module owns all NVAPI FFI behind a mockable `NvColorApi` trait (same pattern as `ColorApi`); the preset store gains two validated fields with serde-default migration; `apply_preset` runs ICC → gamma LUT → NVAPI as a fourth step that neutral presets skip entirely.

**Tech Stack:** Rust + `windows` crate (`Win32_System_LibraryLoader` added) + raw NVAPI dynamic loading (`nvapi64.dll`, no SDK, no link-time dep); React/Vite/Tailwind frontend as-is.

**Spec:** `docs/superpowers/specs/2026-09-29-nvapi-vibrance-hue-design.md` — the plan argues from the spec; executors read both.

## Global Constraints

- Windows 10/11 only, x64 only (`extern "system"` FFI relies on the single x64 ABI).
- NVAPI is dynamic-load only: no SDK download, no import lib, no new Cargo dependencies. Missing DLL/driver must degrade, never panic.
- Neutral vibrance/hue (`50.0` / `0.0`) performs zero NVAPI calls, so pre-existing presets behave byte-identically to today.
- Hue range is 0–359 degrees (NVCP docs), default 0.0; vibrance 0–100, default 50.0.
- Tauri invoke args use camelCase JS keys (`{ edidId }`), matching the proven `importIcc`/`resetMonitor` convention.
- TDD: failing test first in every task; one variable per fix; commit per task with the message given.
- Verification gates per task; full gate before merge: `cargo test`, `npx tsc --noEmit`, `npx vitest run`, `npm run tauri build`.

---

## File Structure

- Create: `src-tauri/src/nvapi.rs` — DLL loader, `NvColorApi` trait, `RealNvapi`, `MockNvapi` (cfg test), `vibrance_supported_cmd`. Single owner of all NVAPI contact.
- Modify: `src-tauri/src/store.rs` — `vibrance`/`hue_deg` on `Preset` + `PresetInput`, validation in create/update, serde-default migration.
- Modify: `src-tauri/src/color.rs` — `ApplyResult.vibrance_applied`, `apply_preset` signature + 4th step + skip rule, `apply_preset_cmd` wiring, existing 5 tests' call sites + preset literals.
- Modify: `src-tauri/src/lib.rs` — `mod nvapi;`, register `vibrance_supported_cmd`.
- Modify: `src-tauri/Cargo.toml` — add `"Win32_System_LibraryLoader"` to the `windows` features list.
- Modify: `src/lib/types.ts` — `vibrance`, `hue_deg` on `Preset`/`PresetInput`; `vibrance_applied` on `ApplyResult`.
- Modify: `src/lib/validation.ts` — range checks for both fields.
- Modify: `src/lib/tauri.ts` — `vibranceSupported(edidId)` wrapper.
- Modify: `src/components/PresetEditor.tsx` — two sliders + support probe + disable-with-note.
- Modify: `src/components/PresetCard.tsx` — show values + vibrance in apply feedback.
- Tests: Rust `#[cfg(test)]` in `store.rs` + `nvapi.rs` + `color.rs`; Vitest in `src/lib/validation.test.ts`.

---

### Task 1: Store schema, validation, migration

**Files:**
- Modify: `src-tauri/src/store.rs:13-35` (structs), `:109-165` (create), `:169-225` (update), tests `:362-375` (`minimal_input`), `:431-455` (update literal), `:555-565` (persist literal)

**Interfaces:**
- Consumes: nothing new.
- Produces (exact names/types later tasks rely on):
  - `Preset { ..., vibrance: f64, hue_deg: f64 }` with `#[serde(default = "default_vibrance")]` / `#[serde(default = "default_hue")]`
  - `PresetInput { ..., vibrance: f64, hue_deg: f64 }` (required; frontend always sends)
  - `fn default_vibrance() -> f64 { 50.0 }`, `fn default_hue() -> f64 { 0.0 }`
  - Validation errors containing `"vibrance"` / `"hue_deg"`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn old_json_without_nvapi_fields_gets_neutral_defaults() {
    let json = r#"{"id":"x","name":"Old","edid_id":"E","icc_hash":"","icc_filename":"","brightness":0.5,"contrast":0.5,"rgb_gains":[1.0,1.0,1.0],"gamma":2.2}"#;
    let preset: Preset = serde_json::from_str(json).unwrap();
    assert_eq!(preset.vibrance, 50.0);
    assert_eq!(preset.hue_deg, 0.0);
}

#[test]
fn create_preset_rejects_bad_vibrance() {
    let mut input = minimal_input();
    input.vibrance = 101.0;
    let err = test_store().create_preset(input).unwrap_err();
    assert!(err.to_string().contains("vibrance"));
}

#[test]
fn create_preset_rejects_bad_hue() {
    let mut input = minimal_input();
    input.hue_deg = 360.0;
    let err = test_store().create_preset(input).unwrap_err();
    assert!(err.to_string().contains("hue_deg"));
}

#[test]
fn create_preset_carries_vibrance_and_hue() {
    let mut store = test_store();
    let mut input = minimal_input();
    input.vibrance = 75.0;
    input.hue_deg = 120.0;
    let preset = store.create_preset(input).unwrap();
    assert_eq!(preset.vibrance, 75.0);
    assert_eq!(preset.hue_deg, 120.0);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test nvapi_fields 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (`no field vibrance`) — that failure IS the red state; do not fix anything else yet.

- [ ] **Step 3: Implement schema + validation**

```rust
fn default_vibrance() -> f64 { 50.0 }
fn default_hue() -> f64 { 0.0 }

pub struct Preset {
    // ... existing fields ...
    pub gamma: f64,
    #[serde(default = "default_vibrance")]
    pub vibrance: f64, // 0–100, 50 = neutral
    #[serde(default = "default_hue")]
    pub hue_deg: f64, // 0–359 degrees
}

pub struct PresetInput {
    // ... existing fields ...
    pub gamma: f64,
    pub vibrance: f64,
    pub hue_deg: f64,
}
```

In `create_preset`, after the gamma check (~line 127):

```rust
        if !(0.0..=100.0).contains(&input.vibrance) {
            return Err(StoreError::InvalidInput(
                "vibrance must be in 0.0..=100.0".into(),
            ));
        }
        if !(0.0..=359.0).contains(&input.hue_deg) {
            return Err(StoreError::InvalidInput(
                "hue_deg must be in 0.0..=359.0".into(),
            ));
        }
```

Same two blocks in `update_preset` after its gamma check (~line 193). Add `vibrance: input.vibrance, hue_deg: input.hue_deg,` to both struct literals (create ~line 157, update ~line 217). Update the three test literals (`minimal_input`, update-changes input, persist input) with `vibrance: 0.5→` no — use realistic values: `minimal_input` gets `vibrance: 50.0, hue_deg: 0.0`.

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"`
Expected: all pass (36 existing + 4 new = 40).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/store.rs
git commit -m "feat: vibrance and hue fields in preset store with migration"
```

---

### Task 2: `nvapi.rs` loader, trait, mock

**Files:**
- Create: `src-tauri/src/nvapi.rs`
- Modify: `src-tauri/Cargo.toml:30-35` (add `"Win32_System_LibraryLoader"` to `windows` features)

**Interfaces:**
- Consumes: `monitor::list_monitors` + `Monitor.edid_id/device_name` (existing) for a small EDID→device resolver owned by this task (`resolve_device_name` in `nvapi.rs`).
- Produces:
  - `pub trait NvColorApi { fn supported(&self, edid_id: &str) -> bool; fn set(&self, edid_id: &str, vibrance: f64, hue_deg: f64) -> Result<(), String>; }` (EDID-keyed, matching the `ColorApi` convention so `color.rs` passes `preset.edid_id` straight through)
  - `pub struct RealNvapi;`, `fn load_nvapi(library_name: &str) -> Option<NvapiFns>` (named param enables the hermetic missing-DLL test)
  - `#[tauri::command] pub fn vibrance_supported_cmd(edid_id: String) -> bool` (defined here in `nvapi.rs`; registered in `lib.rs` in Task 3)
  - `#[cfg(test)] pub struct MockNvapi { calls: Mutex<Vec<(String, f64, f64)>>, supported: bool, fail_set: bool }` with `MockNvapi::new(supported: bool)`

Authoritative references (public, MIT-licensed sample; docs):
- Flow + init + display-id enumeration: `https://github.com/NVIDIA/nvapi` → `Sample_Code/DisplayColorControl/NVHelper.cpp` (`NvAPI_Initialize()`, `NvAPI_DISP_GetDisplayIdByDisplayName(const char*, NvU32*)`, `NvAPI_GetErrorMessage`).
- Private DVC + HUE function names, `NvAPI_QueryInterface` IDs, and `PrivateDisplayDVCInfoEx` / `PrivateDisplayHUEInfo` C layouts: `https://github.com/falahati/NvAPIWrapper` → `NvAPIWrapper/Native/DisplayApi.cs` (`GetDVCInfoEx`, `SetDVCLevelEx`, `GetHUEInfo` + setter counterpart) and its Delegates companion. Transcribe each ID as a named `const` (e.g. `const NVAPI_D3D_SET...` — no, use the real names from source); no magic numbers at call sites. Known-good example to verify against source: `NvAPI_Initialize = 0x0150E828`.
- Hue units: degrees 0–359 (NVCP help). DVC level: percent 0–100. If the transcribed source shows any scaling, convert in a documented adapter fn, not at call sites.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn mock_records_set_with_exact_values() {
    let mock = MockNvapi::new(true);
    mock.set("\\\\.\\DISPLAY5", 75.0, 120.0).unwrap();
    assert_eq!(
        *mock.calls.lock().unwrap(),
        vec![("set".to_string(), 75.0, 120.0)]
    );
}

#[test]
fn mock_respects_supported_flag_and_fail_set() {
    let mock = MockNvapi::new(false);
    assert!(!mock.supported("\\\\.\\DISPLAY5"));
    mock.fail_set = true; // NOTE: field must be non-`mut`-locked; construct via struct literal instead:
}
```

Write the second test correctly (field assignment needs mut):

```rust
#[test]
fn mock_fail_set_returns_error() {
    let mut mock = MockNvapi::new(true);
    mock.fail_set = true;
    let err = mock.set("\\\\.\\DISPLAY5", 75.0, 0.0).unwrap_err();
    assert!(err.contains("mock vibrance failure"));
}

#[test]
fn missing_dll_degrades_to_none() {
    assert!(load_nvapi("chromadeck_nonexistent_xyz.dll").is_none());
}

#[test]
fn neutral_constants_match_spec() {
    assert_eq!(VIBRANCE_NEUTRAL, 50.0);
    assert_eq!(HUE_NEUTRAL, 0.0);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test nvapi 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (`can't find crate nvapi` / unresolved module — red state).

- [ ] **Step 3: Implement `nvapi.rs`**

```rust
// ── NVAPI vibrance/hue (dynamic load, no SDK) ─────────────────────────────
// Windows-only, x64-only. Loads nvapi64.dll at runtime; every absence
// (no DLL, init failure, unsupported display) degrades to false/Err.

use std::ffi::{c_char, c_void, CString};
use std::sync::Mutex;
use std::sync::OnceLock;
use windows::Win32::System::LibraryLoader::{FreeLibrary, GetProcAddress, LoadLibraryW, HMODULE};

pub const VIBRANCE_NEUTRAL: f64 = 50.0;
pub const HUE_NEUTRAL: f64 = 0.0;

pub trait NvColorApi {
    fn supported(&self, device_name: &str) -> bool;
    fn set(&self, device_name: &str, vibrance: f64, hue_deg: f64) -> Result<(), String>;
}

type QueryInterfaceFn = unsafe extern "system" fn(u32) -> *mut c_void;
pub type NvAPI_ShortString = [c_char; 64];

struct NvapiFns {
    _lib: HMODULE, // kept alive for process lifetime; never freed
    initialize: unsafe extern "system" fn() -> i32,
    get_error_message: unsafe extern "system" fn(i32, *mut NvAPI_ShortString),
    get_display_id: unsafe extern "system" fn(*const c_char, *mut u32) -> i32,
    get_dvc_ex: unsafe extern "system" fn(/* transcribed params */) -> i32,
    set_dvc_ex: unsafe extern "system" fn(/* transcribed params */) -> i32,
    get_hue: unsafe extern "system" fn(/* transcribed params */) -> i32,
    set_hue: unsafe extern "system" fn(/* transcribed params */) -> i32,
}

// SAFETY: resolved once at startup; function pointers stay valid for the
// process lifetime and are only ever called (never mutated).
unsafe impl Send for NvapiFns {}
unsafe impl Sync for NvapiFns {}

static NVAPI: OnceLock<Option<NvapiFns>> = OnceLock::new();

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Load and initialize NVAPI from the named library. `None` on ANY failure
/// (missing DLL, missing export, init failure) — callers degrade gracefully.
pub fn load_nvapi(library_name: &str) -> Option<NvapiFns> {
    // ... LoadLibraryW → GetProcAddress("nvapi_QueryInterface") →
    // resolve each ID const → NvAPI_Initialize() == 0, else None.
    // Transcribe the `#[repr(C)]` info structs + QueryInterface IDs from
    // the NvAPIWrapper sources named above; encode IDs as named consts.
}

fn fns() -> Option<&'static NvapiFns> {
    NVAPI.get_or_init(|| load_nvapi("nvapi64.dll")).as_ref()
}

pub struct RealNvapi;

impl NvColorApi for RealNvapi {
    fn supported(&self, edid_id: &str) -> bool {
        // resolve_device_name → display_id_for_device → get_dvc_ex OK
        // && get_hue OK. Any failure → false. No state changes.
    }
    fn set(&self, edid_id: &str, vibrance: f64, hue_deg: f64) -> Result<(), String> {
        // Same resolution; set DVC level then hue; map non-zero status via
        // get_error_message into: "NVAPI call failed (nvapi 0x{code:X}): {text}".
        // Resolution failure → Err("NVAPI unavailable for this display").
    }
}
```

Display-ID resolution helpers (exact) — `resolve_device_name` is owned by
this task so `nvapi.rs` never depends on `color.rs` internals:

```rust
pub(crate) fn resolve_device_name(edid_id: &str) -> Option<String> {
    crate::monitor::list_monitors()
        .into_iter()
        .find(|m| m.edid_id == edid_id && !m.device_name.is_empty())
        .map(|m| m.device_name)
}

fn display_id_for_device(fns: &NvapiFns, device_name: &str) -> Result<u32, String> {
    let name = CString::new(device_name)
        .map_err(|_| "display name contains nul byte".to_string())?;
    let mut id: u32 = 0;
    // SAFETY: GetDisplayIdByDisplayName writes one u32 on success.
    let status = unsafe { (fns.get_display_id)(name.as_ptr(), &mut id) };
    if status != 0 {
        return Err("NVAPI unavailable for this display".into());
    }
    Ok(id)
}
```

Mock (exact):

```rust
#[cfg(test)]
pub struct MockNvapi {
    pub calls: Mutex<Vec<(String, f64, f64)>>,
    pub supported: bool,
    pub fail_set: bool,
}

#[cfg(test)]
impl MockNvapi {
    pub fn new(supported: bool) -> Self {
        MockNvapi {
            calls: Mutex::new(Vec::new()),
            supported,
            fail_set: false,
        }
    }
}

#[cfg(test)]
impl NvColorApi for MockNvapi {
    fn supported(&self, _edid_id: &str) -> bool {
        self.supported
    }
    fn set(&self, _edid_id: &str, vibrance: f64, hue_deg: f64) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(("set".to_string(), vibrance, hue_deg));
        if self.fail_set {
            Err("mock vibrance failure".into())
        } else {
            Ok(())
        }
    }
}
```

#[test]
fn support_command_returns_false_for_unknown_edid() {
    // Unknown EDID → device resolution fails → false, no panic, no driver
    // contact beyond the (real, present) DLL load.
    assert!(!vibrance_supported_cmd("NO_SUCH_EDID".into()));
}
```

Tauri command (exact, lives in `nvapi.rs`; registered in `lib.rs` in Task 3):

```rust
#[tauri::command]
pub fn vibrance_supported_cmd(edid_id: String) -> bool {
    RealNvapi.supported(&edid_id)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"`
Expected: all pass (40 + new nvapi tests; real loader untouched by tests except the missing-DLL case).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/nvapi.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: NVAPI loader with mockable vibrance/hue trait"
```

---

### Task 3: Apply integration (result type, 4th step, commands)

**Files:**
- Modify: `src-tauri/src/color.rs` (ApplyResult struct + `offline()` + `success()` + `apply_preset` signature/body + `apply_preset_cmd` + existing 5 tests' call sites + `fake_preset`/`preset_no_icc` literals)
- Modify: `src-tauri/src/lib.rs:1-10,28-37` (`mod nvapi;`, `use`, handler entry)
- Modify: `src/lib/tauri.ts` (append `vibranceSupported` wrapper)

**Interfaces:**
- Consumes: `NvColorApi::supported/set(edid_id,...)` + `VIBRANCE_NEUTRAL/HUE_NEUTRAL` from Task 2; `Preset.vibrance/hue_deg` from Task 1.
- Produces:
  - `ApplyResult { icc_applied: bool, gamma_applied: bool, vibrance_applied: bool, error: Option<String> }`
  - `apply_preset(api: &dyn ColorApi, nv: &dyn NvColorApi, preset: &Preset, store_profiles_dir: &str) -> ApplyResult`
  - `vibrance_supported_cmd(edid_id: String) -> bool`
  - TS: `vibranceSupported(edidId: string): Promise<boolean>` invoking `"vibrance_supported_cmd"` with `{ edidId }`

- [ ] **Step 1: Write failing tests** (in `color.rs` tests mod; import `crate::nvapi::MockNvapi`)

```rust
#[test]
fn neutral_preset_issues_zero_nvapi_calls() {
    let color = TestRecorder::new(true);
    let nv = MockNvapi::new(true);
    let preset = fake_preset(); // vibrance 50.0, hue_deg 0.0 after literal update
    let result = apply_preset(&color, &nv, &preset, "profiles");
    assert!(nv.calls.lock().unwrap().is_empty());
    assert!(!result.vibrance_applied);
    assert!(result.error.is_none());
}

#[test]
fn nonneutral_preset_calls_nvapi_with_exact_values() {
    let color = TestRecorder::new(true);
    let nv = MockNvapi::new(true);
    let mut preset = fake_preset();
    preset.vibrance = 75.0;
    preset.hue_deg = 120.0;
    let result = apply_preset(&color, &nv, &preset, "profiles");
    assert_eq!(
        *nv.calls.lock().unwrap(),
        vec![("set".to_string(), 75.0, 120.0)]
    );
    assert!(result.gamma_applied && result.vibrance_applied);
    assert!(result.error.is_none());
}

#[test]
fn vibrance_failure_still_reports_gamma_success() {
    let color = TestRecorder::new(true);
    let mut nv = MockNvapi::new(true);
    nv.fail_set = true;
    let mut preset = fake_preset();
    preset.vibrance = 75.0;
    let result = apply_preset(&color, &nv, &preset, "profiles");
    assert!(result.gamma_applied);
    assert!(!result.vibrance_applied);
    assert!(result.error.as_ref().unwrap().contains("vibrance"));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test vibrance 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (unknown `MockNvapi` / missing `vibrance_applied` / arity mismatch — red state).

- [ ] **Step 3: Implement**

Compile-fix checklist (do these first — the crate will not build without
them):
- `fake_preset()` literal gains `vibrance: 50.0, hue_deg: 0.0`; same two
  fields on `preset_no_icc()` (via `fake_preset`, so automatic).
- All 5 existing `apply_preset(&recorder, &preset, "profiles")` calls
  become `apply_preset(&color, &nv, &preset, "profiles")` with
  `let color = TestRecorder::new(true);` (keep each test's existing
  connectivity) and `let nv = MockNvapi::new(true);` added per test.
- Every other literal `ApplyResult { ... }` in `color.rs` gains a
  `vibrance_applied` field: `false` in both `apply_preset_cmd` error
  paths and in `reset_monitor`'s offline/error paths; the reset success
  path gets `vibrance_applied: false` (reset never touches NVAPI).

`ApplyResult` + constructors:

```rust
pub struct ApplyResult {
    pub icc_applied: bool,
    pub gamma_applied: bool,
    pub vibrance_applied: bool,
    pub error: Option<String>,
}
```

`offline()`: all three false + offline error. `success()`: all three true, no error.

Step 4 in `apply_preset`, after the gamma block. The trait is EDID-keyed
(Task 2), so `preset.edid_id` passes straight through — no resolution
code here:

```rust
    // Step 4 — NVAPI vibrance/hue overlay (last; skipped when neutral so
    // pre-existing presets behave exactly as before). Declare next to the
    // existing `let mut gamma_applied` line:
    let mut vibrance_applied = false;
    if preset.vibrance != crate::nvapi::VIBRANCE_NEUTRAL
        || preset.hue_deg != crate::nvapi::HUE_NEUTRAL
    {
        match nv.set(&preset.edid_id, preset.vibrance, preset.hue_deg) {
            Ok(()) => vibrance_applied = true,
            Err(e) => {
                let nv_err = format!("vibrance: {e}");
                error = Some(match error {
                    Some(ref prev) => format!("{prev}; {nv_err}"),
                    None => nv_err,
                });
            }
        }
    }
```

`apply_preset_cmd`: add `let nv = crate::nvapi::RealNvapi;` and pass `&nv` (needs `use crate::nvapi::NvColorApi;` in color.rs so the trait methods resolve). `vibrance_supported_cmd` is already defined in `nvapi.rs` (Task 2) — this task only registers it (see `lib.rs` below).

`lib.rs`: add `mod nvapi;`, extend the `use color::{...}` line to `use color::{apply_preset_cmd, reset_monitor_cmd};` (unchanged) plus `use nvapi::vibrance_supported_cmd;`, add `vibrance_supported_cmd,` to the handler list after `apply_preset_cmd,`.

`tauri.ts` append:

```ts
/**
 * Probe whether a monitor supports NVIDIA vibrance/hue control.
 * Resolves by EDID; arg name must be camelCase (Tauri v2 convention).
 */
export async function vibranceSupported(edidId: string): Promise<boolean> {
  return invoke<boolean>("vibrance_supported_cmd", { edidId });
}
```

Update existing call sites: all 5 existing `apply_preset(&recorder, &preset, "profiles")` become `apply_preset(&color_recorder, &nv_mock, &preset, "profiles")` — rename per test as already written; `fake_preset()` + `preset_no_icc()` literals gain `vibrance: 50.0, hue_deg: 0.0`.

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass (40 + 3 new = 43).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/color.rs src-tauri/src/lib.rs src/lib/tauri.ts
git commit -m "feat: vibrance/hue apply step with neutral skip"
```

---

### Task 4: Frontend (types, validation, editor, card)

**Files:**
- Modify: `src/lib/types.ts:18-44`, `src/lib/validation.ts:4-75`, `src/lib/validation.test.ts` (append), `src/components/PresetEditor.tsx` (DEFAULT_INPUT, populate effect, sliders, probe), `src/components/PresetCard.tsx` (values row, feedback)

**Interfaces:**
- Consumes: `vibranceSupported` (Task 3), `Preset.vibrance/hue_deg`, `ApplyResult.vibrance_applied`.
- Produces: working editor + card; no new exports except none (all local).

- [ ] **Step 1: Write failing tests** (append to `src/lib/validation.test.ts`)

```ts
test("rejects vibrance out of range", () => {
  expect(validatePreset({ vibrance: 101 }).vibrance).toContain("0 and 100");
  expect(validatePreset({ vibrance: -1 }).vibrance).toBeDefined();
  expect(validatePreset({ vibrance: 50 }).vibrance).toBeUndefined();
});

test("rejects hue out of range", () => {
  expect(validatePreset({ hue_deg: 360 }).hue_deg).toContain("0 and 359");
  expect(validatePreset({ hue_deg: 0 }).hue_deg).toBeUndefined();
});
```

Check the existing test file's import line first and reuse it (`validatePreset` already imported per the gamma test).

- [ ] **Step 2: Run to verify they fail**

Run: `npx vitest run 2>&1 | Select-String "Tests|FAIL|FAIL_"` (from repo root)
Expected: FAIL (`vibrance`/`hue_deg` unknown → `undefined` vs expected — red state).

- [ ] **Step 3: Implement**

`types.ts`:

```ts
export interface Preset {
  // ... existing ...
  gamma: number;        // 1.0 – 3.0
  vibrance: number;     // 0 – 100, 50 neutral
  hue_deg: number;      // 0 – 359 degrees
}

export interface PresetInput {
  // ... existing ...
  gamma: number;
  vibrance: number;
  hue_deg: number;
}

export interface ApplyResult {
  icc_applied: boolean;
  gamma_applied: boolean;
  vibrance_applied: boolean;
  error?: string;
}
```

`validation.ts`: add `vibrance?: string; hue_deg?: string;` to `ValidationErrors`; add blocks (exact):

```ts
  if (input.vibrance !== undefined) {
    if (typeof input.vibrance !== "number" || input.vibrance < 0 || input.vibrance > 100) {
      errors.vibrance = "Vibrance must be between 0 and 100";
    }
  }

  if (input.hue_deg !== undefined) {
    if (typeof input.hue_deg !== "number" || input.hue_deg < 0 || input.hue_deg > 359) {
      errors.hue_deg = "Hue must be between 0 and 359";
    }
  }
```

and pass `vibrance: input.vibrance, hue_deg: input.hue_deg` in `validatePresetForm`'s `validatePreset({...})` call.

`PresetEditor.tsx`:
- import: `import { vibranceSupported } from "../lib/tauri";`
- `DEFAULT_INPUT` gains `vibrance: 50, hue_deg: 0,`.
- edit populate effect gains both fields from `editPreset`.
- probe state + effect (exact):

```tsx
const [nvSupported, setNvSupported] = useState<boolean | null>(null);

useEffect(() => {
  let cancelled = false;
  setNvSupported(null);
  if (!form.edid_id) {
    setNvSupported(false);
    return;
  }
  vibranceSupported(form.edid_id)
    .then((ok) => { if (!cancelled) setNvSupported(ok); })
    .catch(() => { if (!cancelled) setNvSupported(false); });
  return () => { cancelled = true; };
}, [form.edid_id]);
```

- sliders after the RGB gains block, mirroring the gamma block's classes, with `disabled={nvSupported === false}` and `disabled:opacity-40` added to className:

```tsx
          {/* Vibrance slider */}
          <div>
            <div className="flex items-center justify-between mb-1.5">
              <label className="text-xs font-medium text-neutral-400">Digital Vibrance</label>
              <span className="text-xs text-neutral-500 font-mono">{form.vibrance.toFixed(0)}</span>
            </div>
            <input
              type="range" min={0} max={100} step={1} value={form.vibrance}
              disabled={nvSupported === false}
              onChange={(e) => updateField("vibrance", parseFloat(e.target.value))}
              className="w-full h-1.5 rounded-lg appearance-none cursor-pointer bg-neutral-700 accent-indigo-500 disabled:opacity-40"
            />
            <div className="flex justify-between text-xs text-neutral-600 mt-0.5">
              <span>0</span><span>100</span>
            </div>
            {errors.vibrance && <p className="mt-1 text-xs text-red-400">{errors.vibrance}</p>}
          </div>
```

Hue block identical with min 0 max 359, `form.hue_deg`, `errors.hue_deg`, label "Hue", value `{form.hue_deg.toFixed(0)}°`. Plus note (exact copy):

```tsx
            {nvSupported === false && (
              <p className="mt-1 text-xs text-amber-400">Digital vibrance/hue need an NVIDIA-driven display.</p>
            )}
```

`PresetCard.tsx`: values row gains `<span>V {preset.vibrance.toFixed(0)}</span>` and `<span>H {preset.hue_deg.toFixed(0)}°</span>`; feedback builder gains `if (lastResult.vibrance_applied) parts.push("vibrance applied");`.

- [ ] **Step 4: Run tests**

Run: `npx tsc --noEmit && npx vitest run 2>&1 | Select-String "Tests|Test Files|FAIL"` (from repo root)
Expected: tsc clean, 14 + 2 = 16 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib/types.ts src/lib/validation.ts src/lib/validation.test.ts src/components/PresetEditor.tsx src/components/PresetCard.tsx
git commit -m "feat: vibrance/hue editor sliders with support probe"
```

---

### Task 5: Hardware verification + release build

**Files:** none (verification only).

**Interfaces:**
- Consumes: full stack from Tasks 1–4.

- [ ] **Step 1: Dev-mode probe on both displays**

Run: `npm run tauri dev` (from repo root). For EACH display (external AOC, internal panel): open Create Preset, select the monitor, record whether vibrance/hue sliders stay enabled or disable with the note.
Expected: external enabled (NVIDIA-driven); internal records actual outcome either way — both are valid test results, write them down.

- [ ] **Step 2: End-to-end apply with NVCP cross-check**

On the enabled display: set Vibrance 75, Hue 0, save, Apply.
Expected: feedback includes "vibrance applied"; NVIDIA Control Panel shows Digital Vibrance at 75%. Then set back to 50 via a second apply and confirm NVCP returns to 50%.

- [ ] **Step 3: Regression sweep (old bugs stay fixed)**

Apply a neutral preset (vibrance 50/hue 0): expect NO vibrance step (feedback shows only ICC/gamma parts as before). Apply brightness-0 preset: expect the actionable brightness error, not a Win32 failure. Reset button: identity restore works.
Expected: all three behave exactly as on master.

- [ ] **Step 4: Release build**

Run: `npm run tauri build` (from repo root).
Expected: MSI + NSIS produced with no errors.

- [ ] **Step 5: Report**

Append results (supported Y/N per display, NVCP cross-check values, regressions) to the task report. No commit (no code changes); if any step fails, STOP and return to Phase 1 with the evidence — do not stack fixes.
