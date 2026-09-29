# ChromaDeck V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows-only Tauri v2 GUI app that manages per-monitor color presets (bundled ICC + OS gamma/RGB tweaks) matched by EDID, applied manually.

**Architecture:** Tauri v2 frontend (React/Vite/Tailwind) for max styling freedom; Rust backend owns all Win32 work — EDID enumeration, ICC install/apply via Mscms, gamma ramp via GDI. Frontend never touches hardware directly, only via Tauri commands. Local JSON store + `profiles/` ICC folder.

**Tech Stack:** Tauri v2, React + Vite + Tailwind, Rust (windows, tauri-plugin-store or plain JSON), Windows Color System (Mscms.dll: `AssociateColorProfileWithDevice`, `SetICMProfile`), GDI `SetDeviceGammaRamp` / `GetDeviceGammaRamp`, EDID via `SetupAPI` / `WMI Win32_DesktopMonitor` + registry `EDID` blob.

**Spec:** Grilled decisions in this thread (color-only presets, per-monitor, bundle ICC, EDID matching, manual build, GUI-only apply, OS gamma-only, Tauri v2). No separate spec doc — this plan argues from those answers.

## Global Constraints

- Windows 10/11 only for V1, no macOS/Linux code paths
- No DDC/CI in V1 — OS gamma/RGB tweaks only
- No tray menu, hotkeys, or auto-switch rules in V1
- No whole-setup snapshots, no capture-current in V1
- ICC files must be bundled/copied into app, never referenced in place
- Presets keyed by monitor EDID serial, never display index

---

## File Structure

- `src-tauri/tauri.conf.json` — Tauri v2 app config, window, bundle
- `src-tauri/src/monitor.rs` — EDID enumeration, monitor list, stable ID
- `src-tauri/src/color.rs` — ICC install/apply, gamma get/set, precedence logic
- `src-tauri/src/store.rs` — JSON persistence for presets + monitors, ICC file hashing/copy
- `src-tauri/src/commands.rs` — Tauri command surface bridging frontend ↔ backend (else consolidated into `lib.rs`)
- `src-tauri/src/main.rs` — wiring only
- `src/App.tsx` — shell + routing (Library / Editor)
- `src/components/MonitorList.tsx` — grouped by physical monitor (EDID), offline state
- `src/components/PresetCard.tsx` — per-preset apply/edit/duplicate/delete/export
- `src/components/PresetEditor.tsx` — manual build form: name, ICC picker, RGB/brightness/gamma sliders
- `src/lib/tauri.ts` — typed invoke wrappers + types mirroring Rust structs
- `src/lib/types.ts` — `Preset`, `Monitor`, `GammaRamp` shared types

Interfaces locked:
- `list_monitors() -> Monitor[] { edid_id: string, model: string, serial: string, connected: bool }`
- `create_preset(input: PresetInput) -> Preset`, `update_preset(id, input) -> Preset`, `delete_preset(id)`, `list_presets() -> Preset[]`
- `apply_preset(id) -> ApplyResult { icc_applied: bool, gamma_applied: bool, error?: string }`
- `import_icc(path: string) -> { hash, stored_path }` — copies into app store
- `Preset { id, name, edid_id, icc_hash, icc_filename, brightness: f32, contrast: f32, rgb_gains: [f32,f32,f32], gamma: f32 }`

Precedence rule: ICC applied first, then OS gamma/RGB overlay on top. Editor must show this hint.

---

### Task 1: Scaffold Tauri v2 + React + Tailwind — COMPLETE (785b0c6)

Shell builds (MSI+NSIS ok), tsc clean, placeholder Library in `src/App.tsx`, shared types in `src/lib/types.ts`.

### Task 2: Monitor enumeration by EDID — COMPLETE (3bfca5c)

`src-tauri/src/monitor.rs` with SetupAPI + registry EDID, EnumDisplayDevices fallback, 7 tests pass, `list_monitors_cmd` registered, `listMonitors()` in `src/lib/tauri.ts`.

### Task 3: Store + bundled ICC management — COMPLETE (2db1132)

`src-tauri/src/store.rs` with CRUD + `import_icc` (sha256 copy), atomic flush, 12 tests (19 total with monitor), wrappers in `src/lib/tauri.ts`.

### Task 4: Apply path — ICC + OS gamma only

**Files:**
- Create: `src-tauri/src/color.rs`
- Modify: `src-tauri/src/lib.rs` (register command), `src/lib/tauri.ts` (add `applyPreset`)

**Interfaces:**
- Consumes: `Preset` from Task 3 store, `Monitor.edid_id` from Task 2
- Produces: `apply_preset(id) -> ApplyResult`

- [ ] **Step 1: Write failing apply test (mocked recorder)**

```rust
#[test]
fn apply_precedence_is_icc_then_gamma() {
  let log = apply_with_recorder(fake_preset());
  assert_eq!(log, vec!["icc", "gamma"]);
}
```

- [ ] **Step 2: Implement minimal `color.rs`**

```rust
pub fn apply_preset(p: &Preset) -> Result<ApplyResult> {
  associate_icc_with_device(&p.edid_id, &p.icc_hash)?; // Mscms AssociateColorProfileWithDeviceW
  set_gamma_ramp(&p.edid_id, p.brightness, p.rgb_gains, p.gamma)?; // GDI SetDeviceGammaRamp
  Ok(ApplyResult { icc_applied: true, gamma_applied: true, error: None })
}
```

- [ ] **Step 3: Handle offline monitor + missing ICC explicitly**

```rust
if !is_connected(&p.edid_id) { return Ok(ApplyResult::offline()); }
```

- [ ] **Step 4: Manual verify on 2 monitors**

Run: apply different ICC+gamma per display, reboot, re-apply
Expected: each display independent, no cross-talk

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/color.rs src-tauri/src/lib.rs src/lib/tauri.ts
git commit -m "feat: apply ICC plus gamma per EDID"
```

### Task 5: Full GUI — Library + Manual Editor (styling-freedom focus)

**Files:**
- Create: `src/components/MonitorList.tsx`, `src/components/PresetCard.tsx`, `src/components/PresetEditor.tsx`
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: `listMonitors, listPresets, applyPreset, createPreset` from `src/lib/tauri.ts`

- [ ] **Step 1: Write failing vitest for editor validation**

```ts
test("rejects gamma out of range", () => {
  expect(validatePreset({ gamma: 5 })).toContain("gamma");
});
```

- [ ] **Step 2: Build Library grouped by monitor + offline badge**

```tsx
// MonitorList.tsx groups presets by edid_id, shows "Offline — kept" when !connected
```

- [ ] **Step 3: Build manual editor (ICC picker + sliders + precedence hint)**

```tsx
// "ICC applied first, gamma/RGB overlay on top" hint text required
```

- [ ] **Step 4: Wire Apply + export/import buttons, verify click-to-apply <2s**

Run: `npm run tauri dev`, create 2 presets on 2 monitors, apply each
Expected: PASS, no tray needed

- [ ] **Step 5: Commit**

```bash
git add src/components src/App.tsx
git commit -m "feat: library plus manual preset editor GUI"
```

---

## Rulings log

- Ruling: Task 1 scaffold deleted `docs/` (create-tauri-app cleaned non-empty dir) — plan file recreated 2026-09-29 with completed-task record. Cost if wrong: none, source of truth is this file + git log.
