# Preset Enforcer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep explicitly-pinned presets applied via a resident 10s drift-check loop with tray presence, surviving fullscreen games, HDR toggles, and dock changes.

**Architecture:** Pin map (`edid_id → preset_id`) persisted in a new `pins.json`; a new `enforce.rs` module runs pure, mock-tested drift detection (`check_once`) over store snapshots; `apply_preset` is refactored so its gamma+vibrance half (`apply_color`) is shared with the enforcer; tray/single-instance/autostart come from official Tauri v2 plugins.

**Tech Stack:** Rust (`windows` crate, existing), `tauri-plugin-single-instance 2.0.0`, `tauri-plugin-autostart 2.0.0` (+ npm `@tauri-apps/plugin-autostart`), React frontend as-is.

**Spec:** `docs/superpowers/specs/2026-09-29-enforcer-design.md` — the plan argues from the spec; executors read both.

## Global Constraints

- Windows 10/11 only, x64 only.
- 10s tick fixed; no focus hooking in v1.
- Neutral vibrance/hue (50.0/0.0) is unmanaged by the enforcer (mirrors the apply skip rule); gamma LUT is always managed for pinned presets.
- ICC is manual-Apply-only; the enforcer never touches ICC association.
- Reset unpins that monitor (frontend sequences unpin-then-reset).
- Autostart default OFF; residency is explicit opt-in.
- Tauri invoke args use camelCase JS keys (proven convention).
- TDD with failing tests first wherever hardware-independent; hardware-dependent reads verified in Task 6.
- Commit per task with the message given; full gate before merge: `cargo test`, `npx tsc --noEmit`, `npx vitest run`, `npm run tauri build`.

---

## File Structure

- Modify: `src-tauri/src/store.rs` — pin map load/flush, pin/unpin/list API, delete-cascade, 3 new commands.
- Modify: `src-tauri/src/color.rs` — extract `open_display_dc` + `read_gamma_ramp` (`pub(crate)`), extract `apply_color`, keep `apply_preset` behavior identical.
- Modify: `src-tauri/src/nvapi.rs` — add `pub(crate) read_levels` (DVC current + hue angle).
- Create: `src-tauri/src/enforce.rs` — `StateReader` trait + `RealStateReader` + `MockStateReader` (cfg test), `EnforceEvent`, `check_once`, `enforce_loop`, `reapply_now_cmd`.
- Modify: `src-tauri/src/lib.rs` — `mod enforce;`, 4 command registrations, tray builder, close-to-tray, 3 plugin registrations, setup spawn.
- Modify: `src-tauri/Cargo.toml` — `tauri-plugin-single-instance = "2.0.0"`, `tauri-plugin-autostart = "2.0.0"`.
- Modify: `package.json` — `@tauri-apps/plugin-autostart` (^2).
- Modify: `src-tauri/capabilities/default.json` — `autostart:allow-enable`, `autostart:allow-disable`, `autostart:allow-is-enabled`.
- Modify: `src/lib/tauri.ts` — `pinPreset`, `unpinMonitor`, `listPins`, `reapplyNow` wrappers.
- Modify: `src/lib/types.ts` — `EnforceEvent` interface.
- Modify: `src/App.tsx` — pins state, footer enforcing count, autostart + reapply settings row.
- Modify: `src/components/MonitorList.tsx` — pins prop, pass-through to cards.
- Modify: `src/components/PresetCard.tsx` — Pin/Unpin button + badge.
- Modify: `src/components/MonitorList.tsx` (`MonitorResetButton`) — unpin-before-reset.

---

### Task 1: Pin state in the store

**Files:**
- Modify: `src-tauri/src/store.rs` (struct ~line 82, `new` ~line 91, `delete_preset` ~line 262, commands ~line 326+)

**Interfaces:**
- Consumes: `Store`, `StoreError::{NotFound,InvalidInput}`, `flush()` atomic pattern, `AppStore`.
- Produces:
  - `Store { ..., pins_path: PathBuf, pinned: HashMap<String,String> }` (edid_id → preset_id)
  - `pin_preset(&mut self, edid_id: &str, preset_id: &str) -> Result<(), StoreError>`
  - `unpin_monitor(&mut self, edid_id: &str)` (no error when absent)
  - `list_pins(&self) -> HashMap<String, String>`
  - `pin_preset_cmd(state, edid_id: String, preset_id: String) -> Result<(), String>`
  - `unpin_monitor_cmd(state, edid_id: String)` (returns `()`)
  - `list_pins_cmd(state) -> HashMap<String, String>`

- [ ] **Step 1: Write failing tests** (in `store.rs` tests mod; `test_store()` helper exists)

```rust
#[test]
fn pin_and_unpin_roundtrip() {
    let mut store = test_store();
    let preset = store.create_preset(minimal_input()).unwrap();
    store.pin_preset("EDID-001", &preset.id).unwrap();
    assert_eq!(store.list_pins().get("EDID-001"), Some(&preset.id));
    store.unpin_monitor("EDID-001");
    assert!(store.list_pins().is_empty());
}

#[test]
fn pin_rejects_unknown_preset() {
    let mut store = test_store();
    let err = store.pin_preset("EDID-001", "nope").unwrap_err();
    assert!(err.to_string().contains("preset not found"));
}

#[test]
fn pin_rejects_wrong_monitor_preset() {
    let mut store = test_store();
    let preset = store.create_preset(minimal_input()).unwrap(); // edid EDID-001
    let err = store.pin_preset("EDID-999", &preset.id).unwrap_err();
    assert!(err.to_string().contains("does not belong"));
}

#[test]
fn delete_cascades_pin() {
    let mut store = test_store();
    let preset = store.create_preset(minimal_input()).unwrap();
    store.pin_preset("EDID-001", &preset.id).unwrap();
    store.delete_preset(&preset.id).unwrap();
    assert!(store.list_pins().is_empty());
}

#[test]
fn pins_persist_across_reopen() {
    let dir = test_store_dir(); // NOTE: test_store() uses a unique dir per call;
    // this test needs a SHARED dir: build it with the same recipe as
    // test_store() but a fixed name, create+pin, drop, reopen, assert.
    let mut store = Store::new(dir.clone()).unwrap();
    let preset = store.create_preset(minimal_input()).unwrap();
    store.pin_preset("EDID-001", &preset.id).unwrap();
    drop(store);
    let reopened = Store::new(dir).unwrap();
    assert_eq!(reopened.list_pins().get("EDID-001"), Some(&preset.id));
}
```

(If `test_store()` cannot share a dir, add a `test_store_at(dir: PathBuf)` helper and reimplement `test_store()` on top of it — exact refactor, no behavior change.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test pin_ 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (no `pinned` field/methods — red state).

- [ ] **Step 3: Implement**

```rust
use std::collections::HashMap;

// Store struct gains:
    pins_path: PathBuf,
    pinned: HashMap<String, String>, // edid_id -> preset_id

// new(): after presets load —
        let pins_path = data_dir.join("pins.json");
        let pinned: HashMap<String, String> = if pins_path.exists() {
            let content = std::fs::read_to_string(&pins_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };
// ... include pins_path + pinned in the Ok(Store { ... }) literal.

    pub fn pin_preset(&mut self, edid_id: &str, preset_id: &str) -> Result<(), StoreError> {
        let preset = self
            .presets
            .iter()
            .find(|p| p.id == preset_id)
            .ok_or_else(|| StoreError::NotFound(preset_id.into()))?;
        if preset.edid_id != edid_id {
            return Err(StoreError::InvalidInput(
                "preset does not belong to this monitor".into(),
            ));
        }
        self.pinned.insert(edid_id.into(), preset_id.into());
        self.flush_pins()
    }

    pub fn unpin_monitor(&mut self, edid_id: &str) {
        if self.pinned.remove(edid_id).is_some() {
            let _ = self.flush_pins();
        }
    }

    pub fn list_pins(&self) -> HashMap<String, String> {
        self.pinned.clone()
    }

    fn flush_pins(&self) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(&self.pinned)?;
        let tmp = self.data_dir.join("pins.json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.pins_path)?;
        Ok(())
    }
```

In `delete_preset`, after `self.presets.remove(idx);` add:

```rust
        let pinned_gone = self.pinned.values().any(|v| v == id);
        if pinned_gone {
            self.pinned.retain(|_, v| v != id);
        }
```

and after `self.flush()?;` add `if pinned_gone { self.flush_pins()?; }` (keep the existing `Ok(removed)` tail).

Commands (exact, after `delete_preset_cmd`):

```rust
#[tauri::command]
pub fn pin_preset_cmd(
    state: tauri::State<'_, AppStore>,
    edid_id: String,
    preset_id: String,
) -> Result<(), String> {
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    store.pin_preset(&edid_id, &preset_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn unpin_monitor_cmd(state: tauri::State<'_, AppStore>, edid_id: String) {
    if let Ok(mut store) = state.0.lock() {
        store.unpin_monitor(&edid_id);
    }
}

#[tauri::command]
pub fn list_pins_cmd(
    state: tauri::State<'_, AppStore>,
) -> Result<HashMap<String, String>, String> {
    let store = state.0.lock().map_err(|e| e.to_string())?;
    Ok(store.list_pins())
}
```

(Registration in `generate_handler!` happens in Task 4 with the other new commands — state it: do NOT register yet.)

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass (51 + 5 new = 56).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/store.rs
git commit -m "feat: pinned-preset state with cascade and persistence"
```

---

### Task 2: Read paths + shared apply core

**Files:**
- Modify: `src-tauri/src/color.rs` (extract `open_display_dc` + `read_gamma_ramp`, extract `apply_color`, keep `apply_preset` behavior identical)
- Modify: `src-tauri/src/nvapi.rs` (add `pub(crate) read_levels`)

**Interfaces:**
- Consumes: existing `CreateDCW`/`DeleteDC`/`SetDeviceGammaRamp` blocks, `build_gamma_ramp`, `NvColorApi`, DVC/HUE get path.
- Produces:
  - `pub(crate) fn read_gamma_ramp(device_name: &str) -> Result<[u16; 256*3], String>` (color.rs)
  - `pub(crate) fn read_levels(edid_id: &str) -> Result<(f64, f64), String>` (nvapi.rs; vibrance, hue)
  - `pub fn apply_color(api: &dyn ColorApi, nv: &dyn NvColorApi, preset: &Preset) -> ApplyResult` with `icc_applied: false` always

- [ ] **Step 1: Note the red state (refactor task)**

There is no failing test to write for new behavior — the deliverable is a behavior-preserving split. The red state is: `read_gamma_ramp` / `read_levels` / `apply_color` do not exist. Write one compiler-checked usage first in a scratch test? No — instead: the existing `apply_preset` tests (5 suites) are the regression net and must stay green untouched. Proceed to implementation; any behavior change surfaces as a test failure.

- [ ] **Step 2: Implement**

In `color.rs`, extract the DC block shared by set/read (exact shape — replace the inline block in `set_gamma_ramp` with a call):

```rust
/// Open a device context for a display device. Shared by set/read paths.
fn open_display_dc(device_name: &str) -> Result<HDC, String> {
    let wide_device: Vec<u16> = OsString::from(device_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: CreateDCW for a display device returns the monitor's DC.
    let hdc: HDC = unsafe {
        CreateDCW(
            PCWSTR::from_raw(windows::core::w!("DISPLAY").as_ptr()),
            PCWSTR::from_raw(wide_device.as_ptr()),
            None,
            None,
        )
    };
    if hdc.is_invalid() {
        return Err(format!("failed to create DC for {device_name}"));
    }
    Ok(hdc)
}
```

(`HDC`, `CreateDCW`, `OsString`/`encode_wide`, `PCWSTR` imports already exist in color.rs — reuse, add none. Error string identical to the current one so the existing "failed to create DC" behavior/tests are untouched.)

```rust
/// Read the live gamma ramp for a display device. Used by the enforcer's
/// drift check. Hardware-only; verified live in Task 6.
pub(crate) fn read_gamma_ramp(device_name: &str) -> Result<[u16; 256 * 3], String> {
    use windows::Win32::Graphics::Gdi::GetDeviceGammaRamp;
    let hdc = open_display_dc(device_name)?;
    let mut ramp = [0u16; 256 * 3];
    // SAFETY: GetDeviceGammaRamp fills 768 WORDs on success.
    let ok: bool = unsafe { GetDeviceGammaRamp(hdc, &mut ramp as *mut _ as _).as_bool() };
    // SAFETY: DeleteDC is safe after a successful CreateDCW.
    unsafe { let _ = DeleteDC(hdc); };
    if !ok {
        return Err(format!("GetDeviceGammaRamp failed for {device_name}"));
    }
    Ok(ramp)
}
```

Extract `apply_color` (move the gamma + vibrance blocks verbatim out of `apply_preset`; `apply_preset` keeps connectivity check + ICC step, then merges):

```rust
/// Apply driver-level color state (gamma LUT + NVAPI vibrance/hue),
/// without ICC association. Shared by manual Apply and the enforcer.
pub fn apply_color(
    api: &dyn ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    preset: &crate::store::Preset,
) -> ApplyResult {
    if !api.is_connected(&preset.edid_id) {
        return ApplyResult::offline();
    }
    // ... gamma block verbatim (sets gamma_applied / gamma error) ...
    // ... vibrance block verbatim incl. neutral skip (sets vibrance_applied) ...
    ApplyResult {
        icc_applied: false,
        gamma_applied,
        vibrance_applied,
        error,
    }
}
```

`apply_preset` becomes connectivity check + ICC step, then:

```rust
    let mut result = apply_color(api, nv, preset);
    result.icc_applied = icc_applied;
    result.error = match (icc_error, result.error) {
        (Some(a), Some(b)) => Some(format!("{a}; {b}")),
        (Some(a), None) => Some(a),
        (None, b) => b,
    };
    result
```

where the ICC step sets local `icc_applied: bool` and `icc_error: Option<String>` exactly as today (`Some(format!("ICC: {e}"))`). Composition order (ICC; gamma; vibrance) is preserved byte-for-byte.

In `nvapi.rs`, add (uses the existing `fns()`, `resolve_device_name`, `display_id_for_device`, get-call pattern — no new FFI):

```rust
/// Read current (vibrance, hue) for an EDID. Used by the enforcer's drift
/// check. Hardware-only; verified live in Task 6.
pub(crate) fn read_levels(edid_id: &str) -> Result<(f64, f64), String> {
    let fns = fns().ok_or_else(|| "NVAPI unavailable for this display".to_string())?;
    let device = resolve_device_name(edid_id)
        .ok_or_else(|| "NVAPI unavailable for this display".to_string())?;
    let id = display_id_for_device(fns, &device)?;
    // ... get_dvc_ex → current_level as f64, get_hue → current_angle as f64 ...
    // Any non-zero status → status_to_string error. Return Ok((vibrance, hue)).
}
```

(Build the two info structs with `version: nvapi_version::<DvcInfoEx>()` / `::<HueInfo>()` — the proven encoding.)

- [ ] **Step 3: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all 56 pass UNCHANGED (refactor proof: zero behavior delta).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/color.rs src-tauri/src/nvapi.rs
git commit -m "refactor: shared apply_color plus gamma/NVAPI read paths"
```

---

### Task 3: Enforcer logic + commands

**Files:**
- Create: `src-tauri/src/enforce.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod enforce;` + 4 command registrations — pin/unpin/list commands from Task 1 plus `reapply_now_cmd` from this task)

**Interfaces:**
- Consumes: `apply_color`, `read_gamma_ramp`, `build_gamma_ramp` (make `pub(crate)` in Task 2), `read_levels`, `Store::list_pins/list_presets`, `Preset`.
- Produces:
  - `pub struct EnforceEvent { pub edid_id: String, pub preset_id: String, pub applied: bool, pub error: Option<String> }` (derive Debug, Clone, Serialize)
  - `pub fn check_once(api, nv, reader, pins: &HashMap<String,String>, presets: &[Preset], force: bool) -> Vec<EnforceEvent>`
  - `pub trait StateReader { fn read_lut(&self, edid_id: &str) -> Result<[u16;768], String>; fn read_color(&self, edid_id: &str) -> Result<(f64,f64), String>; }` (+ `RealStateReader`, `MockStateReader` cfg test)
  - `reapply_now_cmd(state) -> Vec<EnforceEvent>` (force = true via real impls)
  - `enforce_loop(app: AppHandle)` async task (10s tick calling a private `check_and_enforce(&app)`; spawned in Task 4's setup — define here, wire there)

- [ ] **Step 1: Write failing tests** (in `enforce.rs` tests mod; import `crate::color::TestRecorder`, `crate::nvapi::MockNvapi`; build presets via a local `fn p(id, edid, brightness, vibrance, hue)` helper returning `crate::store::Preset` with sane remaining fields)

```rust
#[test]
fn no_pins_no_calls() {
    let color = TestRecorder::new(true);
    let nv = MockNvapi::new(true);
    let reader = MockStateReader::clean();
    let events = check_once(&color, &nv, &reader, &HashMap::new(), &[p("a", "E", 1.0, 50.0, 0.0)], false);
    assert!(events.is_empty());
    assert!(color gamma never called);
}

#[test]
fn in_sync_preset_issues_no_writes() {
    // reader returns exactly build_gamma_ramp(preset) + (50.0, 0.0);
    // expect: events.len() == 1, applied == false, no color/nv calls.
}

#[test]
fn drifted_lut_reapplies_gamma_only_when_neutral() {
    // reader LUT = identity, preset brightness 0.5 (neutral vibrance/hue);
    // expect applied == true, gamma called once, nv mock calls empty.
}

#[test]
fn drifted_vibrance_reapplies() {
    // reader color (50.0, 0.0), preset vibrance 75.0;
    // expect applied == true, nv calls == [("set", 75.0, 0.0)].
}

#[test]
fn offline_monitor_skipped() {
    // TestRecorder::new(false) (+ nv unsupported): expect event with
    // applied == false and no driver calls at all.
}

#[test]
fn stale_pin_and_missing_preset_skipped() {
    // pins references unknown preset id and unknown edid: expect zero events.
}

#[test]
fn force_reapplies_despite_sync() {
    // in-sync state + force=true → applied == true with driver calls.
}
```

(MockStateReader shape, exact: `pub struct MockStateReader { pub lut: [u16; 768], pub color: (f64, f64), pub fail: bool }` + `MockStateReader::clean()` returning identity ramp + `(50.0, 0.0)` + fail false. For identity ramp in tests: `build_gamma_ramp(1.0, 1.0, [1.0,1.0,1.0], 1.0)` — must be `pub(crate)` (Task 2). For "gamma never called" assertions: TestRecorder has no call log! Check: TestRecorder in color.rs — does it record calls? The nvapi MockNvapi records `calls`; color TestRecorder... earlier tests asserted `calls == ["icc","gamma"]` on the NV recorder?? No — precedence test used color TestRecorder and asserted calls vec. So color::TestRecorder DOES have a calls log. Reuse it in assertions (Mock shape verified in source: `calls: Mutex<Vec<String>>` — assert on it directly.))

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test enforce 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (no `enforce` module — red state).

- [ ] **Step 3: Implement** (`enforce.rs`, exact logic)

```rust
// ── Preset enforcer: resident drift-check + reapply ────────────────────────

use std::collections::HashMap;
use serde::Serialize;

/// Outcome of one enforced (or skipped) pin.
#[derive(Debug, Clone, Serialize)]
pub struct EnforceEvent {
    pub edid_id: String,
    pub preset_id: String,
    pub applied: bool,
    pub error: Option<String>,
}

/// Hardware state reads, isolated for testability.
pub trait StateReader {
    fn read_lut(&self, edid_id: &str) -> Result<[u16; 768], String>;
    fn read_color(&self, edid_id: &str) -> Result<(f64, f64), String>;
}

pub struct RealStateReader;

impl StateReader for RealStateReader {
    fn read_lut(&self, edid_id: &str) -> Result<[u16; 768], String> {
        let device = crate::nvapi::resolve_device_name(edid_id)
            .ok_or_else(|| "unknown monitor".to_string())?;
        crate::color::read_gamma_ramp(&device)
    }
    fn read_color(&self, edid_id: &str) -> Result<(f64, f64), String> {
        crate::nvapi::read_levels(edid_id)
    }
}

/// Pure drift-check + conditional apply over snapshots. `force` reapplies
/// regardless of drift (manual Reapply-now path).
pub fn check_once(
    api: &dyn crate::color::ColorApi,
    nv: &dyn crate::nvapi::NvColorApi,
    reader: &dyn StateReader,
    pins: &HashMap<String, String>,
    presets: &[crate::store::Preset],
    force: bool,
) -> Vec<EnforceEvent> {
    let mut events = Vec::new();
    for (edid_id, preset_id) in pins {
        let preset = match presets.iter().find(|p| &p.id == preset_id) {
            Some(p) => p,
            None => continue, // stale pin (preset gone without cascade): skip
        };
        if preset.edid_id != *edid_id {
            continue; // pin/preset mismatch: skip, never apply cross-monitor
        }
        if !api.is_connected(edid_id) {
            events.push(EnforceEvent { edid_id: edid_id.clone(), preset_id: preset_id.clone(), applied: false, error: None });
            continue;
        }
        let drifted = force || lut_drifted(preset, reader, edid_id) || color_drifted(preset, reader, edid_id);
        if !drifted {
            events.push(EnforceEvent { edid_id: edid_id.clone(), preset_id: preset_id.clone(), applied: false, error: None });
            continue;
        }
        let result = crate::color::apply_color(api, nv, preset);
        events.push(EnforceEvent {
            edid_id: edid_id.clone(),
            preset_id: preset_id.clone(),
            applied: result.gamma_applied || result.vibrance_applied,
            error: result.error,
        });
    }
    events
}
```

`lut_drifted` / `color_drifted` helpers (exact):

```rust
fn lut_drifted(preset: &crate::store::Preset, reader: &dyn StateReader, edid_id: &str) -> bool {
    match reader.read_lut(edid_id) {
        Ok(live) => {
            live != crate::color::build_gamma_ramp(
                preset.brightness,
                preset.contrast,
                preset.rgb_gains,
                preset.gamma,
            )
        }
        Err(_) => true, // unreadable state counts as drift (try to heal)
    }
}

fn color_drifted(preset: &crate::store::Preset, reader: &dyn StateReader, edid_id: &str) -> bool {
    if preset.vibrance == crate::nvapi::VIBRANCE_NEUTRAL
        && preset.hue_deg == crate::nvapi::HUE_NEUTRAL
    {
        return false; // unmanaged when neutral (mirrors apply skip rule)
    }
    match reader.read_color(edid_id) {
        Ok((v, h)) => v != preset.vibrance || h != preset.hue_deg,
        Err(_) => true,
    }
}
```

(`build_gamma_ramp` must be `pub(crate)` — Task 2 does that. Preset.rgb_gains is `[f64; 3]` (Copy) — pass directly.)

`enforce_loop` + production glue (exact):

```rust
/// Background task: check every 10s. Spawned once from setup (Task 4).
pub async fn enforce_loop(app: tauri::AppHandle) {
    use tauri::async_runtime::sleep;
    loop {
        check_and_enforce(&app);
        sleep(std::time::Duration::from_secs(10)).await;
    }
}

fn check_and_enforce(app: &tauri::AppHandle) {
    use tauri::Manager;
    let (pins, presets) = match app.try_state::<crate::store::AppStore>() {
        Some(s) => match s.0.lock() {
            Ok(store) => (store.list_pins(), store.list_presets()),
            Err(_) => return,
        },
        None => return,
    };
    if pins.is_empty() {
        return;
    }
    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let reader = RealStateReader;
    let _ = check_once(&api, &nv, &reader, &pins, &presets, false);
}
```

Hmm — `RealColorApi` construction: `let api = RealColorApi;` (unit struct, matches apply_preset_cmd precedent). `try_state` vs `state`: `app.state::<T>()` panics if missing; managed state always present → use `app.state::<...>()` directly? Safer `try_state` (exists on Manager in Tauri 2 — yes, `try_state` is available). Keep try_state.

Command (exact, in enforce.rs):

```rust
#[tauri::command]
pub fn reapply_now_cmd(state: tauri::State<'_, crate::store::AppStore>) -> Vec<EnforceEvent> {
    let (pins, presets) = match state.0.lock() {
        Ok(store) => (store.list_pins(), store.list_presets()),
        Err(e) => {
            return vec![EnforceEvent {
                edid_id: String::new(),
                preset_id: String::new(),
                applied: false,
                error: Some(format!("store lock: {e}")),
            }]
        }
    };
    let api = crate::color::RealColorApi;
    let nv = crate::nvapi::RealNvapi;
    let reader = RealStateReader;
    check_once(&api, &nv, &reader, &pins, &presets, true)
}
```

(`RealColorApi`/`RealNvapi` are unit structs in their modules and `pub` — verify field-less construction compiles as `RealColorApi` value: yes, precedent in apply_preset_cmd.)

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass (56 + 7 new = 63).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/enforce.rs src-tauri/src/lib.rs
git commit -m "feat: enforcer drift-check engine with manual reapply"
```

(`lib.rs` change in this task: `mod enforce;` + register `pin_preset_cmd, unpin_monitor_cmd, list_pins_cmd, reapply_now_cmd` in `generate_handler!`. Tray/plugins/loop-spawn stay Task 4.)

---

### Task 4: Tray, residency, plugins

**Files:**
- Modify: `src-tauri/src/lib.rs` (plugins, tray builder, close-to-tray, setup spawn)
- Modify: `src-tauri/Cargo.toml` (`tauri-plugin-single-instance = "2.0.0"`, `tauri-plugin-autostart = "2.0.0"`)
- Modify: `package.json` (`@tauri-apps/plugin-autostart` `^2` + lockfile via `npm install`)
- Modify: `src-tauri/capabilities/default.json` (3 autostart permissions)

**Interfaces:**
- Consumes: `enforce_loop`, `check_once` force path (Task 3), `AppStore`.
- Produces: resident app (tray, single-instance, autostart-capable, 10s loop).

- [ ] **Step 1: Dependency red state**

Add the two Cargo deps + run `cargo check`:
Run: `cargo check 2>&1 | Select-String "^error" | Select-Object -First 3` (from `src-tauri/`)
Expected: FAIL with unresolved `tauri_plugin_single_instance` / menu/tray paths until lib.rs is wired (red state — do not fix anything else yet).

- [ ] **Step 2: Implement** (exact `lib.rs` additions; keep every existing line)

Imports to add:

```rust
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
```

Builder changes (exact order — single-instance FIRST per its README):

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .manage(app_store)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("ChromaDeck")
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            // ... existing entries unchanged ...
            vibrance_supported_cmd,
            reset_monitor_cmd,
            pin_preset_cmd,
            unpin_monitor_cmd,
            list_pins_cmd,
            reapply_now_cmd,
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                crate::enforce::enforce_loop(handle).await;
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
```

(`pin_preset_cmd` etc. need `use store::{..., pin_preset_cmd, unpin_monitor_cmd, list_pins_cmd};` and `use enforce::reapply_now_cmd;` — extend the existing use block.)

Tray builder (exact, new `fn build_tray` above `run()`; menu IDs `tray-show`, `tray-reapply`, `tray-quit`):

```rust
fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "tray-show", "Show", true, None::<&str>)?;
    let reapply =
        MenuItem::with_id(app, "tray-reapply", "Reapply now", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &reapply,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("ChromaDeck")
        .icon(app.default_window_icon().cloned().unwrap())
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-quit" => app.exit(0),
            "tray-show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "tray-reapply" => {
                use tauri::Manager;
                if let Some(store) = app.try_state::<crate::store::AppStore>() {
                    if let Ok(s) = store.0.lock() {
                        let (pins, presets) = (s.list_pins(), s.list_presets());
                        drop(s);
                        let api = crate::color::RealColorApi;
                        let nv = crate::nvapi::RealNvapi;
                        let reader = crate::enforce::RealStateReader;
                        let _ = crate::enforce::check_once(
                            &api, &nv, &reader, &pins, &presets, true,
                        );
                    }
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;
    Ok(())
}
```

(`RealStateReader` must be `pub` in enforce.rs — Task 3 declares it `pub struct RealStateReader` ✓. `RealColorApi`/`RealNvapi` unit structs are `pub` ✓. If any Tauri API name mismatches the installed tauri 2.x version, consult docs.rs for the pinned version and use the documented name — then note the deviation in the report.)

`default.json` permissions append (exact strings):

```json
    "autostart:allow-enable",
    "autostart:allow-disable",
    "autostart:allow-is-enabled"
```

- [ ] **Step 3: Run checks**

Run: `cargo check 2>&1 | Select-String "^error|^warning" | Select-Object -First 5` (from `src-tauri/`); then `npm install` (repo root) for the autostart JS dep.
Expected: check clean; install clean.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock package.json package-lock.json src-tauri/capabilities/default.json
git commit -m "feat: tray residency with single-instance and autostart"
```

---

### Task 5: Frontend (pin UI, reset-unpin, footer, settings)

**Files:**
- Modify: `src/lib/tauri.ts` (4 wrappers), `src/lib/types.ts` (+`EnforceEvent`), `src/App.tsx` (pins state, footer, settings row), `src/components/MonitorList.tsx` (pins prop + pass-through), `src/components/PresetCard.tsx` (Pin/Unpin + badge), `src/components/MonitorList.tsx` (`MonitorResetButton` unpin-first)

**Interfaces:**
- Consumes: `pin_preset_cmd`/`unpin_monitor_cmd`/`list_pins_cmd`/`reapply_now_cmd` (Task 3–4), autostart JS bindings.
- Produces: working pin UX. No new Rust surface.

- [ ] **Step 1: No new pure logic — state the red baseline**

No new Vitest cases (no new pure functions; wiring covered by `tsc` + Task 6 manual pass). Verify baseline first:
Run: `npx tsc --noEmit; echo TSC-BASELINE` (from repo root)
Expected: clean (proves the starting point before edits).

- [ ] **Step 2: Implement** (exact edits)

`types.ts` append:

```ts
export interface EnforceEvent {
  edid_id: string;
  preset_id: string;
  applied: boolean;
  error?: string;
}
```

`tauri.ts` append (camelCase args per convention):

```ts
/** Pin a preset as the enforced default for its monitor. */
export async function pinPreset(edidId: string, presetId: string): Promise<void> {
  return invoke<void>("pin_preset_cmd", { edidId, presetId });
}

/** Remove enforcement for a monitor (no-op when unpinned). */
export async function unpinMonitor(edidId: string): Promise<void> {
  return invoke<void>("unpin_monitor_cmd", { edidId });
}

/** Map of edid_id -> preset_id for pinned monitors. */
export async function listPins(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("list_pins_cmd");
}

/** Force a full enforce pass now; returns per-pin outcomes. */
export async function reapplyNow(): Promise<EnforceEvent[]> {
  return invoke<EnforceEvent[]>("reapply_now_cmd");
}
```

Also update the `tauri.ts` header comment if it names covered commands (keep accurate, minimal).

`App.tsx` (exact edits — current imports already include `useEffect`):

```tsx
import { listMonitors, listPresets, listPins, reapplyNow } from "./lib/tauri";
import type { Monitor, Preset, EnforceEvent } from "./lib/types";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
```

State (next to the existing `useState` lines):

```tsx
const [pins, setPins] = useState<Record<string, string>>({});
const [reapplyMsg, setReapplyMsg] = useState<string | null>(null);
```

`fetchData` body becomes (only the `Promise.all` block changes):

```tsx
      const [monitorsData, presetsData, pinsData] = await Promise.all([
        listMonitors(),
        listPresets(),
        listPins(),
      ]);
      setMonitors(monitorsData);
      setPresets(presetsData);
      setPins(pinsData);
```

`<MonitorList ... />` gains two props:

```tsx
      <MonitorList
        monitors={monitors}
        presets={presets}
        loading={loading}
        onEdit={handleEdit}
        onRefresh={fetchData}
        onCreateNew={handleCreateNew}
        pins={pins}
        onPinChange={fetchData}
      />
```

Footer becomes (replaces the whole `<footer>...</footer>` block) — enforcing
count, reapply feedback, autostart toggle:

```tsx
      {/* Status bar */}
      <footer className="border-t border-neutral-800 px-6 py-2 flex items-center justify-between text-xs text-neutral-600">
        <span>
          {loading ? "Loading…" : `${presets.length} preset${presets.length !== 1 ? "s" : ""} · ${monitors.filter((m) => m.connected).length} monitor${monitors.filter((m) => m.connected).length !== 1 ? "s" : ""} connected · ${Object.keys(pins).length} pinned`}
        </span>
        <span className="inline-flex items-center gap-3">
          {reapplyMsg && <span className="text-neutral-400">{reapplyMsg}</span>}
          {!loading && (
            <span>
              Last refresh: {new Date().toLocaleTimeString()}
            </span>
          )}
          <button
            onClick={async () => {
              try {
                const events: EnforceEvent[] = await reapplyNow();
                const applied = events.filter((e) => e.applied).length;
                const firstErr = events.find((e) => e.error)?.error;
                setReapplyMsg(firstErr ? `Reapply: ${firstErr}` : `Reapplied ${applied}/${events.length}`);
              } catch (err) {
                setReapplyMsg(`Reapply failed: ${String(err)}`);
              }
              setTimeout(() => setReapplyMsg(null), 5000);
              fetchData();
            }}
            className="px-2 py-1 text-xs rounded-md text-neutral-500 hover:text-neutral-200 hover:bg-neutral-700 transition-colors"
            title="Re-run enforcement now"
          >
            Reapply now
          </button>
          <AutostartToggle />
        </span>
      </footer>
```

(`AutostartToggle` local component as specified below; `useEffect` is already
imported — no import change needed for it.)

`MonitorList.tsx` (exact): Props interface gains

```tsx
  pins: Record<string, string>;
  onPinChange: () => void;
```

signature destructure gains `pins, onPinChange`, and each `<PresetCard>`
gains `isPinned={pins[monitor.edid_id] === preset.id}` and
`onPinChange={onPinChange}`.

`AutostartToggle` (local component in `App.tsx`, exact):

```tsx
function AutostartToggle() {
  const [on, setOn] = useState<boolean | null>(null);
  useEffect(() => {
    isEnabled().then(setOn).catch(() => setOn(false));
  }, []);
  const toggle = async () => {
    try {
      if (on) await disable();
      else await enable();
      setOn(!on);
    } catch {
      // keep current state on failure
    }
  };
  return (
    <label className="inline-flex items-center gap-1.5 text-xs text-neutral-500 cursor-pointer">
      <input type="checkbox" checked={on ?? false} onChange={toggle} className="accent-indigo-500" />
      Start with Windows
    </label>
  );
}
```

(`useEffect` is already imported at App.tsx line 1 — no import change.)

`MonitorList.tsx`: Props gains `pins: Record<string, string>; onPinChange: () => void;`, passed from App; forward to each card: `isPinned={pins[monitor.edid_id] === preset.id}` `onPinChange={onPinChange}`. `MonitorResetButton` handler becomes unpin-first (exact):

```tsx
try {
  await unpinMonitor(edidId); // no-op when unpinned; Reset always disarms
} catch {
  // ignore — reset proceeds regardless
}
// ... existing resetMonitor flow unchanged ...
```

(import `unpinMonitor` alongside `resetMonitor`.)

`PresetCard.tsx`: Props gains `isPinned: boolean; onPinChange: () => void;`. Pin button next to Edit (exact classes mirroring Edit):

```tsx
<button
  onClick={async () => {
    try {
      if (isPinned) await unpinMonitor(preset.edid_id);
      else await pinPreset(preset.edid_id, preset.id);
      onPinChange();
    } catch {
      // silent (matches existing card error style)
    }
  }}
  className="px-2 py-1 text-xs font-medium rounded-md text-neutral-400 hover:text-neutral-200 hover:bg-neutral-700 transition-colors"
  title={isPinned ? "Stop enforcing this preset" : "Pin as enforced default"}
>
  {isPinned ? "Unpin" : "Pin"}
</button>
```

(import `pinPreset, unpinMonitor` in the existing tauri import line.) Badge next to the preset name (exact):

```tsx
{isPinned && (
  <span className="text-[10px] font-medium text-indigo-300 bg-indigo-900/40 border border-indigo-800/50 rounded px-1.5 py-0.5">
    Pinned
  </span>
)}
```

- [ ] **Step 3: Run checks**

Run: `npx tsc --noEmit; echo TSC; npx vitest run 2>&1 | Select-String "Tests |Test Files "` (from repo root)
Expected: tsc clean, 16/16 pass (no new cases by design).

- [ ] **Step 4: Commit**

```bash
git add src/lib/tauri.ts src/lib/types.ts src/App.tsx src/components/MonitorList.tsx src/components/PresetCard.tsx
git commit -m "feat: pin UI with reset-disarms and enforcing footer"
```

---

### Task 6: Hardware verification + release build

**Files:** none (verification only).

**Interfaces:** consumes the full stack.

- [ ] **Step 1: Unit gates re-run**

Run: `cargo test` (src-tauri), `npx tsc --noEmit`, `npx vitest run` (root).
Expected: 63+ Rust green, tsc clean, 16 Vitest green. (Counts: 56 + 7 enforce tests = 63; adjust report if the count differs and explain why.)

- [ ] **Step 2: Neutral preset produces zero writes**

Pin a neutral preset (vibrance 50/hue 0) on the external display via the UI. Externally clobber ONLY vibrance (NVCP or script to 75).
Expected: enforcer does NOT restore it (neutral = unmanaged) AND does not error; gamma LUT untouched. (Documents the unmanaged-neutral semantic live.)

- [ ] **Step 3: Clobber-and-restore proof**

Pin a non-neutral preset (e.g. vibrance 75). Clobber the gamma LUT to identity with an external script AND set vibrance to 50 externally. Wait ≤15s.
Expected: LUT returns to the preset ramp AND vibrance returns to 75 (verify the latter in NVCP). This is the money test.

- [ ] **Step 4: Reset disarms**

With a pin active, click Reset on that monitor.
Expected: identity restored AND pin badge disappears (unpinned); LUT stays identity across the next two ticks (no re-enforcement).

- [ ] **Step 5: Tray + residency click-through**

Close window → app stays (tray icon present) → left-click shows window → tray Reapply now works → tray Quit exits → relaunch second instance focuses first (single-instance) → autostart toggle on/off round-trips `isEnabled`.
Expected: all pass; note any deviation verbatim in the report.

- [ ] **Step 6: Release build**

Run: `npm run tauri build` (repo root). Expected: MSI + NSIS produced, no errors.

- [ ] **Step 7: Report (no commit — no code changes)**

Report per-step outcomes, both displays' probe states, and any residual. On ANY failure: STOP, return evidence, do not stack fixes (systematic-debugging Phase 4 rule).
