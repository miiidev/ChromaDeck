# Auto-Apply Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a mapped program becomes foreground, apply its preset to the monitor holding that window; revert to identity when no mapped program for that preset remains foreground.

**Architecture:** A resident 1-second Win32 foreground poll feeds a pure state machine. The state machine decides apply/revert/skip; the runtime executes through the existing colour pipeline (`apply_color`, which already skips ICC) and `reset_monitor`, then emits `auto-apply://outcome` for UI markers and toasts. State is in-memory only; the master switch and preset→program mappings are persisted.

**Tech Stack:** Rust (Tauri v2, `windows` crate 0.62), React 19 + TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-08-post-meetup-0-8-3-design.md` §2 (restored from `697b29e`)

## Global Constraints

- No new crates. Add only `windows` features `Win32_UI_WindowsAndMessaging` and `Win32_System_Threading`.
- Process identity is the executable base name, lowercased (`chrome.exe`). Full paths are reduced to their base name.
- One program maps to at most one preset; enforce in the store and mirror the error inline in the editor.
- A pinned monitor is never auto-touched. Pin beats rule.
- Auto-apply state is in-memory only. Never write `applied.json`; never re-assert after relaunch.
- Auto-apply skips the ICC step entirely (`apply_color`, not `apply_preset_detailed`).
- Master switch is persisted, default OFF, and exists in both the settings footer and the tray.
- Revert uses `reset_monitor` (identity colour; ICC untouched).
- The preset stays in the library while auto-applied; only manual applies hide it.
- Ignore ChromaDeck's own foreground window so opening the app never triggers a revert.
- Gates before merge: `cargo test`, `npm run test`, `npm run build`.

## Review Focus

These are the failure modes most likely to bite a real user, in order. Each has a test in the task that owns the code.

1. ChromaDeck itself becomes foreground while a mapped program is active — colour must not revert.
2. A mapped window moves from monitor A to monitor B — A reverts, B applies.
3. The mapped program's monitor is pinned — nothing changes; one informational toast.
4. A mapped program exits to Explorer — exactly one revert, no reset every tick.
5. The master switch is turned off mid-run — watching stops, markers clear, colour stays.
6. The same program is added to a second preset — rejected in store and editor.

---

### Task 0: Restore the lost 0.8.3 commits

**Status:** DONE before this plan.

`git cherry-pick a2c590a..1955ecd` restored the spec, diagnostics, collision fix, and reset-all commits. Frontend gates already green (169 tests, build). Cargo gate runs before Task 1 begins.

**Interfaces:** The collision fix is the prerequisite for `MonitorFromWindow → device_name → edid_id`; use `crate::monitor::list_monitors()` and `crate::store::base_edid_key`-style fallbacks that the restored code added.

---

### Task 1: Preset store — `attached_programs`

**Files:**
- Modify: `src-tauri/src/store.rs` (`Preset`, `PresetInput`, `create_preset`, `update_preset`)
- Create: `src-tauri/src/auto_apply.rs` (only `normalize_program_name` + tests in this task)
- Modify: `src-tauri/src/lib.rs` (`mod auto_apply;`)
- Modify: `src/lib/types.ts`, `src/lib/presetForm.ts`

**Interfaces:**
- Consumes: existing `StoreError::InvalidInput`.
- Produces: `Preset.attached_programs: Vec<String>`, `PresetInput.attached_programs: Vec<String>`; `pub fn normalize_program_name(raw: &str) -> Option<String>`.

- [ ] **Step 1: Write the failing normalisation test**

```rust
#[test]
fn normalizes_full_paths_and_case() {
    assert_eq!(normalize_program_name(r"C:\Program Files\Google\Chrome\chrome.EXE"), Some("chrome.exe".into()));
    assert_eq!(normalize_program_name("  chrome.exe "), Some("chrome.exe".into()));
    assert_eq!(normalize_program_name(""), None);
    assert_eq!(normalize_program_name("   "), None);
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test --quiet normalize_program_name`
Expected: FAIL — function/ module missing.

- [ ] **Step 3: Implement the type and helper**

Add `#[serde(default)] pub attached_programs: Vec<String>` to both `Preset` and `PresetInput`. `normalize_program_name`: trim, take substring after the last `/` or `\`, lowercase, return `None` for empty or whitespace-containing names.

- [ ] **Step 4: Persist and validate uniqueness**

In `create_preset` and `update_preset`, normalise every entry, drop empties, dedupe within the input, and reject a name already present on any *other* preset:

```rust
return Err(StoreError::InvalidInput(format!("{program} is already mapped to \"{}\"", other.name)));
```

Map the input into the stored preset. Add tests: creating a preset with `["chrome.exe"]` persists it; creating a second preset with `["CHROME.EXE"]` fails; updating a preset with its own existing program succeeds.

- [ ] **Step 5: Mirror the field in TypeScript**

Add `attached_programs?: string[]` to `Preset` and `attached_programs: string[]` to `PresetInput`; add `attached_programs: []` to `DEFAULT_PRESET_INPUT` and map `preset.attached_programs ?? []` in `presetToFormValues`.

- [ ] **Step 6: Run gates and commit**

Run: `cargo test --quiet`, `npm run test`, `npm run build`.
Commit: `feat: presets carry mapped programs`.

---

### Task 2: Persisted master switch

**Files:**
- Modify: `src-tauri/src/store.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `Store::auto_apply_enabled(&self) -> bool`, `Store::set_auto_apply_enabled(&mut self, enabled: bool) -> Result<(), StoreError>`, `settings.json` `{ "auto_apply_enabled": bool }`; commands `get_auto_apply_enabled_cmd` and `set_auto_apply_enabled_cmd`.

- [ ] **Step 1: Write failing store tests**

```rust
#[test]
fn auto_apply_defaults_off_and_persists() {
    let dir = temp_store();
    let mut store = Store::new(dir.clone()).unwrap();
    assert!(!store.auto_apply_enabled());
    store.set_auto_apply_enabled(true).unwrap();
    assert!(Store::new(dir).unwrap().auto_apply_enabled());
}
```

A corrupt `settings.json` must be backed up and leave the switch OFF (reuse `read_json_or_backup`).

- [ ] **Step 2: Implement**

Add `settings_path`, `auto_apply_enabled` to `Store`. Load with `read_json_or_backup`; flush only the settings file on change. `ensure_writable` applies to this write.

- [ ] **Step 3: Add commands**

`get_auto_apply_enabled_cmd(state) -> bool`; `set_auto_apply_enabled_cmd(state, enabled) -> Result<(), String>`.

- [ ] **Step 4: Run gates and commit**

`cargo test --quiet`; commit `feat: persist auto-apply master switch`.

---

### Task 3: Pure auto-apply state machine

**Files:**
- Modify: `src-tauri/src/auto_apply.rs`

**Interfaces:**
- Produces:

```rust
pub struct ObservedForeground { pub program: String, pub edid_id: String }
pub struct ActiveAuto { pub edid_id: String, pub preset_id: String, pub program: String }
pub enum AutoAction {
    Apply { edid_id: String, preset_id: String, program: String },
    Revert { edid_id: String, preset_id: String },
    Skip { edid_id: String, preset_id: String, program: String, reason: String },
}
pub fn resolve_mapping<'a>(presets: &'a [Preset], program: &str) -> Option<&'a Preset>;
pub fn transition(
    active: Option<&ActiveAuto>,
    presets: &[Preset],
    pins: &HashMap<String, String>,
    foreground: Option<&ObservedForeground>,
) -> Vec<AutoAction>;
```

- [ ] **Step 1: Write failing tests** covering the Review Focus cases:

```rust
#[test]
fn unmapped_foreground_reverts_active_once() { /* active Some + foreground explorer.exe -> [Revert] */ }

#[test]
fn moving_window_reverts_old_and_applies_new() { /* active on DISPLAY1 -> [Revert(D1), Apply(D2)] */ }

#[test]
fn pinned_target_skips_and_reverts_previous() { /* pins has D2 -> [Revert(prev), Skip(D2)] */ }

#[test]
fn same_preset_same_monitor_is_a_noop() { /* different program, same preset+edid -> [] */ }

#[test]
fn no_active_and_unmapped_is_a_noop() { /* -> [] */ }
```

- [ ] **Step 2: Run and confirm failure** — `cargo test --quiet auto_apply`.

- [ ] **Step 3: Implement `transition`**

Rules in order: no foreground or unmapped → revert active if any. Mapped to pinned target → revert active if any, then `Skip` reason `"monitor is pinned"`. Mapped to the same preset on the same monitor as active → no action. Otherwise revert active if any, then `Apply`.

- [ ] **Step 4: Run and commit** — `cargo test --quiet`; commit `feat: pure auto-apply state machine`.

---

### Task 4: Win32 foreground + window-monitor probe

**Files:**
- Create: `src-tauri/src/foreground.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`

**Interfaces:**
- Produces: `pub fn foreground_window() -> Option<ObservedWindow>` where `ObservedWindow { program: String, device_name: Option<String> }`; `pub fn self_program() -> Option<String>`.

- [ ] **Step 1: Add Windows features** — `Win32_UI_WindowsAndMessaging`, `Win32_System_Threading`.

- [ ] **Step 2: Implement**

`GetForegroundWindow` → null is `None`. `GetWindowThreadProcessId` → PID. `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)`; `QueryFullProcessImageNameW`; `CloseHandle` on every path. Lowercase the base name with `normalize_program_name`.

Monitor: `MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY)`; `GetMonitorInfoW` with `MONITORINFOEXW`; return `szDevice` as `device_name`. `self_program()` resolves the current process image with the same helper.

- [ ] **Step 3: Extract a pure helper + test** — `program_from_path(path: &str) -> Option<String>` reuses `normalize_program_name`; test `C:\Games\Game.EXE → game.exe`.

- [ ] **Step 4: Commit** — `feat: read foreground window and its monitor`.

---

### Task 5: Watcher runtime, tray checkbox, commands

**Files:**
- Modify: `src-tauri/src/auto_apply.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Tasks 1–4; `crate::color::{RealColorApi, apply_color, reset_monitor}`, `crate::nvapi::RealNvapi`, `crate::monitor::list_monitors`.
- Produces: `pub async fn auto_apply_loop(app: AppHandle)`; `pub fn set_auto_apply(app: &AppHandle, enabled: bool) -> Result<(), String>`; commands `list_auto_applied_cmd`; event `auto-apply://outcome` with:

```rust
#[derive(Clone, Serialize)]
pub struct AutoApplyEvent {
    pub kind: String, // "applied" | "reverted" | "skipped" | "error" | "cleared"
    pub edid_id: String,
    pub preset_id: String,
    pub preset_name: String,
    pub monitor_name: String,
    pub program: String,
    pub error: Option<String>,
}
```

- [ ] **Step 1: Implement the loop**

Every 1 s: if disabled, clear active state once and emit `cleared` (no colour reset). Read foreground; if its program equals `self_program()`, skip the tick entirely. Resolve `device_name` to `edid_id` via `list_monitors()`. Build `(program, edid_id)`; if unchanged from the last tick, do nothing. Otherwise call `transition`, execute actions in order:

- `Apply` → find preset; `apply_color` (never ICC); on success set active, emit `applied`; on error emit `error`.
- `Revert` → `reset_monitor`; clear active; emit `reverted` (include error if reset failed).
- `Skip` → emit `skipped` with the reason.

If the active monitor disconnects, clear state and emit `cleared`.

- [ ] **Step 2: Add `list_auto_applied_cmd`** returning `Option<AutoAppliedInfo { edid_id, preset_id }>`.

- [ ] **Step 3: Tray checkbox**

Use `CheckMenuItem::with_id` labelled `Auto-apply`, checked from the store at build time. On click call `set_auto_apply`, sync the check state, and emit `auto-apply://enabled` with the new bool. `set_auto_apply` persists, updates the check item, and emits the same event.

- [ ] **Step 4: Spawn the loop** in `setup` next to `enforce_loop`.

- [ ] **Step 5: Commit** — `feat: auto-apply watcher and tray switch`.

---

### Task 6: Frontend bridge

**Files:**
- Modify: `src/lib/tauri.ts`

**Interfaces:**
- Produces: `getAutoApplyEnabled(): Promise<boolean>`, `setAutoApplyEnabled(enabled: boolean): Promise<void>`, `listAutoApplied(): Promise<{ edid_id: string; preset_id: string } | null>`, `AutoApplyEvent` interface.

- [ ] **Step 1: Add typed wrappers** matching the backend names. Browser preview: `false`, no-op, `null`.
- [ ] **Step 2: Commit** — `feat: frontend bridge for auto-apply`.

---

### Task 7: Preset editor program chips

**Files:**
- Create: `src/lib/programMapping.ts`
- Create: `src/lib/programMapping.test.ts`
- Modify: `src/components/PresetEditor.tsx`, `src/lib/types.ts`

**Interfaces:**
- Produces: `normalizeProgramName(raw: string): string | null`, `addProgram(list: string[], raw: string): { list: string[]; error?: string }`, `duplicateOwner(presets: Preset[], program: string, selfId?: string): string | null`.

- [ ] **Step 1: Write failing Vitest cases** — full path → base name; duplicate inside the list ignored silently; program owned by another preset → error naming that preset.
- [ ] **Step 2: Implement the pure helpers.**
- [ ] **Step 3: Add the editor section** after the tag/shape block: a `PROGRAMS` label, removable chips, a text input + `Add`, and `Browse…` using `open({ multiple: true, filters: [{ name: "Programs", extensions: ["exe"] }] })`. Selected paths are normalised to base names. A rejected duplicate renders inline: `chrome.exe is already mapped to "Work"`.
- [ ] **Step 4: Run Vitest, typecheck, commit** — `npm run test`, `npm run build`; commit `feat: map programs to presets in the editor`.

---

### Task 8: Markers, toggle, and outcome toast

**Files:**
- Modify: `src/App.tsx`, `src/components/MonitorSidebar.tsx`, `src/components/MonitorList.tsx`, `src/components/PresetCard.tsx`
- Test: `src/lib/autoApplyState.test.ts` (new, pure reducer)
- Create: `src/lib/autoApplyState.ts`

**Interfaces:**
- Produces: `autoMap: Record<string, string>` (edid → preset id) in `App`; reducer `applyAutoEvent(map, event) -> map` (applied sets, reverted/cleared/error deletes, skipped leaves unchanged).

- [ ] **Step 1: Write failing reducer tests** for each `kind`.
- [ ] **Step 2: Implement the reducer and App wiring**

App state `autoMap`; on mount fetch `listAutoApplied()`; `listen("auto-apply://outcome")` updates the map and shows a transient message: `Auto-applied <preset> to <monitor>`, `Reverted <monitor>`, or the skip reason. `listen("auto-apply://enabled")` flips the toggle. Pass `autoMap` to `MonitorSidebar` and `MonitorList`.

- [ ] **Step 3: Footer toggle** — checkbox labelled `Auto-apply` beside `Autostart`, initialised from `getAutoApplyEnabled()`, calling `setAutoApplyEnabled`.

- [ ] **Step 4: Monitor status** — in `MonitorCard`, an active auto entry takes display precedence when the monitor is not pinned: show the preset's category chip plus a small `AUTO` mono tag. Do not feed auto into `appliedMap`.

- [ ] **Step 5: Library marker** — pass `autoApplied={Object.values(autoMap).includes(preset.id)}` to `PresetCard`; render a tiny `AUTO` badge. Do not remove the preset from `libraryPresets` for auto applies.

- [ ] **Step 6: Run gates and commit** — `npm run test`, `npm run build`; commit `feat: auto-apply markers, switch, and toast`.

---

### Task 9: Version bump and final verification

**Files:**
- Modify: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`

- [ ] **Step 1:** Bump all three to `0.8.3`.
- [ ] **Step 2:** Run `cargo test`, `npm run test`, `npm run build`.
- [ ] **Step 3:** Visual QA of the editor chips, footer toggle, and monitor `AUTO` marker against the running app.
- [ ] **Step 4:** Commit `release: v0.8.3 auto-apply`.

---

## Self-Review

- **Spec coverage:** attached programs (T1), master switch + tray (T2/T5), foreground poll (T4/T5), window monitor targeting (T4), pin conflict (T3), revert on close (T3/T5), in-memory bookkeeping (T5), ICC skipped (T5), uniqueness (T1/T7), markers/toast (T8).
- **Deferred by spec:** nothing else in §2. Projector (§5) stays deferred.
- **Self-process rule** is a builder call, recorded here so it is not re-litigated: ChromaDeck's own foreground window is ignored to avoid self-inflicted reverts.
- **Cold start:** the watcher seeds its last-seen key from the first tick without acting, so a program already foreground at launch is never applied until it changes.
