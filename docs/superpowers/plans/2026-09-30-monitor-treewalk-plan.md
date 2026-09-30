# Real-Monitor Enumeration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** List physical monitors by real EDID names via an adapter→monitor tree walk, with stable identities and a one-shot wipe (with backup) of GPU-keyed legacy data.

**Architecture:** Rewrite only the enumeration core of `monitor.rs` (SetupAPI iteration and enrich-matching deleted; EDID parsing and registry helpers kept); add a sentinel-gated backup-and-wipe in `Store::new`; the `Monitor` struct and every downstream consumer stay byte-identical in shape.

**Tech Stack:** Rust + `windows` crate 0.62 (`Win32_Graphics_Gdi` already enabled; no new dependencies) + `winreg` (already used).

**Spec:** `docs/superpowers/specs/2026-09-30-monitor-enumeration-design.md` — the plan argues from the spec; executors read both.

## Global Constraints

- Windows 10/11 only, x64 only.
- `Monitor { edid_id, model, serial, connected, device_name }` field names, types, and order are frozen — no frontend or downstream changes.
- `device_name` always carries the driving adapter `\\.\DISPLAYn`; never a GPU row, never empty-for-hardware (empty only if truly unresolvable, which the walk prevents by construction).
- TDD with failing tests first wherever hardware-independent; hardware proof in Task 3.
- No NVAPI SDK, no new crates. `extern "system"` for any new FFI (none expected — all calls already imported).
- Commit per task with the message given; full gate before merge: `cargo test`, `npx tsc --noEmit`, `npx vitest run`, `npm run tauri build`.

---

## File Structure

- Modify: `src-tauri/src/monitor.rs` — delete SetupAPI iteration + enrich + adapter fallback; add tree walk + pure `identify_monitor`; keep EDID parsers, registry reader, `wide_to_string`, struct, command, existing tests.
- Modify: `src-tauri/src/store.rs` — sentinel-gated backup-and-wipe in `Store::new` only.
- Untouched (verified by Task 3): frontend, `color.rs`, `nvapi.rs`, `enforce.rs`, store schema, Tauri commands.

---

### Task 1: Adapter→monitor tree walk

**Files:**
- Modify: `src-tauri/src/monitor.rs` (only file)

**Interfaces:**
- Consumes: `DISPLAY_DEVICEW`, `EnumDisplayDevicesW`, `DISPLAY_DEVICE_ATTACHED_TO_DESKTOP` (all via existing `Gdi::*` import); `parse_edid_serial`, `parse_edid_model_name`, `wide_to_string`, `EdidBlob`, `winreg` key iteration (`enum_keys`) — all kept; new `read_edid_for_monitor` defined in this task.
- Produces:
  - `fn enum_gdi_monitors() -> Vec<Monitor>` (private)
  - `fn identify_monitor(edid: Option<&[u8; 128]>, instance_id: &str, device_string: &str) -> (String, String, String)` — returns `(edid_id, model, serial)`; pure, no Win32
  - `pub fn list_monitors() -> Vec<Monitor>` (body replaced: delegates to `enum_gdi_monitors()`; signature unchanged)
  - `pub fn list_monitors_cmd()` unchanged

- [ ] **Step 1: Write failing tests** (append to `monitor.rs` tests mod; `fake_edid_blob` / `fake_edid_blob_with_model` helpers exist — reuse them)

```rust
    // ── identify_monitor ─────────────────────────────────────────────

    #[test]
    fn identify_prefers_serial_and_edid_name() {
        let mut buf = fake_edid_blob("ABC123");
        buf[0x36] = 0x00;
        buf[0x36 + 1] = 0x00;
        buf[0x36 + 2] = 0x00;
        buf[0x36 + 3] = 0xFC;
        buf[0x36 + 4..0x36 + 4 + 7].copy_from_slice(b"MyPanel");
        buf[0x36 + 4 + 7] = 0x0A;
        let (id, model, serial) =
            identify_monitor(Some(&*buf), "MONITOR\\X\\0", "GDI Name");
        assert_eq!(id, "ABC123");
        assert_eq!(model, "MyPanel");
        assert_eq!(serial, "ABC123");
    }

    #[test]
    fn identify_falls_back_to_unit_id_without_serial() {
        let mut buf = Box::new([0u8; 128]);
        buf[0..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        buf[8] = 0x04;
        buf[9] = 0x41; // mfr 0x0441
        buf[10] = 0x34;
        buf[11] = 0x12; // product LE 0x1234
        buf[12] = 0x78;
        buf[13] = 0x56;
        buf[14] = 0x34;
        buf[15] = 0x12; // serial LE 0x12345678
        let (id, model, serial) =
            identify_monitor(Some(&*buf), "MONITOR\\X\\0", "GDI Name");
        assert_eq!(id, "EDID:0441-1234-12345678");
        assert_eq!(model, "GDI Name");
        assert_eq!(serial, "");
    }

    #[test]
    fn identify_without_edid_uses_instance_path() {
        let (id, model, serial) =
            identify_monitor(None, "MONITOR\\FOO\\1&2&3", "Generic Monitor");
        assert_eq!(id, "MONITOR\\FOO\\1&2&3");
        assert_eq!(model, "Generic Monitor");
        assert_eq!(serial, "");
    }

    #[test]
    fn identify_empty_device_string_falls_back_to_instance() {
        let (id, model, _) = identify_monitor(None, "MONITOR\\FOO\\1", "");
        assert_eq!(id, "MONITOR\\FOO\\1");
        assert_eq!(model, "MONITOR\\FOO\\1");
    }

    #[test]
    fn manufacturer_decodes_three_letters() {
        let mut buf = Box::new([0u8; 128]);
        buf[8] = 0x04;
        buf[9] = 0x43; // 0x0443 → A=1, B=2, C=3
        assert_eq!(parse_edid_manufacturer(&buf), "ABC");
    }

```rust
/// Take the base 128-byte EDID block from a registry blob.
///
/// Hardware root cause, second half (2026-09-30): real blobs are usually
/// 256 bytes (base + extension); a direct `try_into()` to `[u8; 128]`
/// rejects them by length, silently dropping EDIDs that parsed fine.
/// The base block (bytes 0–127) is what every parser consumes.
fn base_block(blob: Vec<u8>) -> Option<[u8; 128]> {
    blob.get(..128)?.try_into().ok().copied()
}
```

```rust
    #[test]
    fn base_block_accepts_256_byte_blob() {
        // Base block carries a serial descriptor; extension half is zeros.
        let mut blob = vec![0u8; 256];
        blob[0..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        let off = 0x48;
        blob[off] = 0x00;
        blob[off + 1] = 0x00;
        blob[off + 2] = 0x00;
        blob[off + 3] = 0xFF;
        blob[off + 4..off + 4 + 6].copy_from_slice(b"ABC123");
        blob[off + 4 + 6] = 0x0A;
        let base = base_block(blob).expect("first 128 bytes must parse");
        assert_eq!(parse_edid_serial(&base).as_deref(), Some("ABC123"));
    }
```

(Also update Step 4 counts: existing 7 EDID tests + 7 new = 14 in monitor.rs.)

    #[test]
    fn identify_constructs_name_without_descriptor() {
        let mut buf = Box::new([0u8; 128]);
        buf[0..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        buf[8] = 0x04;
        buf[9] = 0x43;
        buf[10] = 0x34;
        buf[11] = 0x12; // product 0x1234
        let (id, model, serial) = identify_monitor(Some(&*buf), "MONITOR\\X\\0", "GDI Name");
        assert_eq!(id, "EDID:0443-1234-00000000");
        assert_eq!(model, "ABC 1234");
        assert_eq!(serial, "");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test identify_ 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: compile FAIL (`identify_monitor` undefined — red state).

- [ ] **Step 3: Implement**

(a) Add the pure identity functions (exact — place after `wide_to_string`):

```rust
/// Manufacturer 3-letter code from EDID bytes 8–9 (big-endian 5-bit packed,
/// A=1). Used for constructed display names on nameless panels.
fn parse_edid_manufacturer(edid: &[u8; 128]) -> String {
    let raw = u16::from_be_bytes([edid[8], edid[9]]);
    [(raw >> 10) & 0x1F, (raw >> 5) & 0x1F, raw & 0x1F]
        .iter()
        .map(|&c| (b'A' + c as u8 - 1) as char)
        .collect()
}

/// Best display name when the EDID name descriptor is absent: a constructed
/// `MFR PRODUCT` tag (laptop panels typically carry no name). `None` when
/// the EDID carries no manufacturer (blank block).
fn constructed_model(ed: &[u8; 128]) -> Option<String> {
    if ed[8] == 0 && ed[9] == 0 {
        return None;
    }
    Some(format!(
        "{} {:04X}",
        parse_edid_manufacturer(ed),
        u16::from_le_bytes([ed[10], ed[11]])
    ))
}

/// Resolve stable identity + display name from EDID (when available),
/// instance path, and GDI device string. Pure function — fully testable.
///
/// Chain: EDID serial → manufacturer+product+numeric serial → instance path.
/// Name chain: EDID name → constructed MFR+product → device string →
/// instance path.
fn identify_monitor(
    edid: Option<&[u8; 128]>,
    instance_id: &str,
    device_string: &str,
) -> (String, String, String) {
    if let Some(ed) = edid {
        let serial = parse_edid_serial(ed).unwrap_or_default();
        let mut model = parse_edid_model_name(ed).unwrap_or_default();
        if model.is_empty() {
            model = constructed_model(ed).unwrap_or_default();
        }
        if model.is_empty() {
            model = device_string.to_string();
        }
        if !serial.is_empty() {
            return (serial.clone(), model, serial);
        }
        let mfr = u16::from_be_bytes([ed[8], ed[9]]);
        let prod = u16::from_le_bytes([ed[10], ed[11]]);
        let ser = u32::from_le_bytes([ed[12], ed[13], ed[14], ed[15]]);
        let unit = format!("EDID:{mfr:04X}-{prod:04X}-{ser:08X}");
        return (unit, model, String::new());
    }
    let model = if device_string.is_empty() {
        instance_id.to_string()
    } else {
        device_string.to_string()
    };
    (instance_id.to_string(), model, String::new())
}
```

(b) Replace the enumeration core (exact — replaces `list_monitors` body, `enum_setupapi_monitors`, `get_device_instance_id`, `get_device_desc`, `enrich_with_device_names`, `enum_fallback_displays` — delete all five):

```rust
/// Enumerate all connected monitors.  Adapter→monitor tree walk: each
/// adapter's child entries are physical monitors with real names.
/// Adapters without children are skipped, so GPU rows never appear.
pub fn list_monitors() -> Vec<Monitor> {
    enum_gdi_monitors()
}

fn enum_gdi_monitors() -> Vec<Monitor> {
    let mut monitors = Vec::new();
    for adapter_index in 0.. {
        let mut adapter = DISPLAY_DEVICEW::default();
        adapter.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        // SAFETY: EnumDisplayDevicesW with NULL enumerates adapters.
        let ok = unsafe { EnumDisplayDevicesW(None, adapter_index, &mut adapter, 0) };
        if !ok.as_bool() {
            break;
        }
        let adapter_name = wide_to_string(&adapter.DeviceName);
        if adapter_name.is_empty() {
            continue;
        }
        // Ruling (plan refines spec §1 wording): attachment is an
        // adapter-output property, so the adapter flag alone governs
        // `connected`. Child entries do not reliably carry
        // ATTACHED_TO_DESKTOP (commonly 0); requiring it would misreport
        // live monitors as disconnected.
        let adapter_attached = (adapter.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP)
            == DISPLAY_DEVICE_ATTACHED_TO_DESKTOP;
        let wide_adapter: Vec<u16> = OsString::from(&adapter_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut child_index = 0;
        loop {
            let mut child = DISPLAY_DEVICEW::default();
            child.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
            // SAFETY: child enumeration for a valid adapter name string.
            let ok = unsafe {
                EnumDisplayDevicesW(
                    PCWSTR::from_raw(wide_adapter.as_ptr()),
                    child_index,
                    &mut child,
                    0,
                )
            };
            if !ok.as_bool() {
                break;
            }
            child_index += 1;
            let instance_id = wide_to_string(&child.DeviceID);
            if instance_id.is_empty() {
                continue;
            }
            let device_string = wide_to_string(&child.DeviceString);
            let edid: Option<[u8; 128]> = read_edid_for_monitor(&instance_id)
                .and_then(base_block);
            let (edid_id, model, serial) =
                identify_monitor(edid.as_ref(), &instance_id, &device_string);
            monitors.push(Monitor {
                edid_id,
                model,
                serial,
                connected: adapter_attached,
                device_name: adapter_name.clone(),
            });
        }
    }
    monitors
}
```

(c) Imports + helper swap: delete `use windows::Win32::Devices::DeviceAndDriverInstallation::*;` and the `GUID_DEVINTERFACE_MONITOR` const (lines 14, 17–19); add `use windows::core::PCWSTR;`. DELETE the old `read_edid_registry(instance_id)` fn (its `Enum\<instance_id>` path cannot work — no `Enum\MONITOR` key exists) and add its replacement below. Keep `serde`, `OsString`/`OsStringExt`, `io`, `winreg::*`, `Gdi::*`, `parse_*`, `wide_to_string`, `EdidBlob`, struct, command, all 7 existing tests untouched. Update the file header comment to describe the tree walk (replace lines 1–3 comment, keep code identical elsewhere).

Replacement EDID reader (exact — hardware root cause, 2026-09-30: the `MONITOR\...` DeviceID namespace has no `Enum\MONITOR` registry key, so direct lookup always missed; monitor EDIDs live under `Enum\DISPLAY\<model>\<instance>\Device Parameters\EDID`):

```rust
/// Read the EDID blob for a monitor DeviceID (`MONITOR\<model>\...`).
/// Resolves via `Enum\DISPLAY\<model>`, subkeys sorted for determinism,
/// first blob ≥128 bytes wins. Identical-twin panels are an accepted
/// limitation: either twin's EDID may attach (documented).
fn read_edid_for_monitor(device_id: &str) -> Option<Vec<u8>> {
    let model = device_id.split('\\').nth(1)?;
    let base = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(
            format!(r"SYSTEM\CurrentControlSet\Enum\DISPLAY\{model}"),
            KEY_READ,
        )
        .ok()?;
    let mut names: Vec<String> = base.enum_keys().filter_map(|k| k.ok()).collect();
    names.sort();
    for name in names {
        let params = base
            .open_subkey_with_flags(format!("{name}\\Device Parameters"), KEY_READ)
            .ok()?;
        let blob: EdidBlob = params.get_value("EDID").ok()?;
        if blob.0.len() >= 128 {
            return Some(blob.0);
        }
    }
    None
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass (existing 7 EDID tests + 7 new = 14 in monitor.rs; full suite green). Then `cargo check 2>&1 | Select-String "^error|^warning"` — expect clean (no dead code: every kept helper is used; `read_edid_registry` is deleted, not left orphaned).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/monitor.rs
git commit -m "feat: adapter-to-monitor tree-walk enumeration"
```

---

### Task 2: One-shot backup-and-wipe with sentinel

**Files:**
- Modify: `src-tauri/src/store.rs` (`Store::new` only + tests)

**Interfaces:**
- Consumes: existing `local_timestamp()`, `flush()`, `flush_pins()`, `test_store_dir_unique()` (all verified present in source).
- Produces: sentinel-gated wipe; no API changes.

- [ ] **Step 1: Write failing test** (in `store.rs` tests mod)

```rust
    #[test]
    fn first_run_backs_up_and_wipes_legacy_data() {
        let dir = test_store_dir_unique();
        std::fs::write(
            dir.join("presets.json"),
            r#"[{"id":"a","name":"Old","edid_id":"PCI\\VEN_10DE&DEV_1","icc_hash":"","icc_filename":"","brightness":55.0,"contrast":60.0,"rgb_gains":[1.0,1.0,1.0],"gamma":1.0,"vibrance":100.0,"hue_deg":0.0,"color_model":"nvcp-v1"}]"#,
        )
        .unwrap();
        std::fs::write(dir.join("pins.json"), r#"{"PCI\\VEN_X":"a"}"#).unwrap();
        let store = Store::new(dir.clone()).unwrap();
        assert!(store.list_presets().is_empty());
        assert!(store.list_pins().is_empty());
        let mut bak_presets = false;
        let mut bak_pins = false;
        for e in std::fs::read_dir(&dir).unwrap() {
            let n = e.unwrap().file_name().to_string_lossy().into_owned();
            if n.starts_with("presets.json.bak-") {
                bak_presets = true;
            }
            if n.starts_with("pins.json.bak-") {
                bak_pins = true;
            }
        }
        assert!(bak_presets && bak_pins, "both backups must exist");
        assert!(dir.join(".monitor-enumeration-v2").exists());
        // Second open: sentinel respected, no duplicate wipe activity.
        let store2 = Store::new(dir).unwrap();
        assert!(store2.list_presets().is_empty());
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test first_run_backs_up 2>&1 | Select-String "test result|error\[|FAILED"` (from `src-tauri/`)
Expected: FAIL (assertion — no sentinel logic yet; presets load normally — red state).

- [ ] **Step 3: Implement** (exact — insert immediately after `let mut store = Store {...};` construction, BEFORE the legacy-migration `if needs_migration` block so backups preserve pre-migration originals; also relocate the `let needs_migration = ...` binding to just after this block)

```rust
        // ── Monitor-enumeration generation wipe (one-shot, user-approved)
        // First run under real-monitor identities: back up legacy data,
        // then start empty. Sentinel makes it exactly-once.
        if !data_dir.join(".monitor-enumeration-v2").exists() {
            if !store.presets.is_empty() {
                let bak = format!("presets.json.bak-{}", local_timestamp());
                let _ = std::fs::copy(&store.presets_path, store.data_dir.join(&bak));
                store.presets = Vec::new();
                store.flush()?;
            }
            if !store.pinned.is_empty() {
                let bak = format!("pins.json.bak-{}", local_timestamp());
                let _ = std::fs::copy(&store.pins_path, store.data_dir.join(&bak));
                store.pinned = HashMap::new();
                store.flush_pins()?;
            }
            let _ = std::fs::write(data_dir.join(".monitor-enumeration-v2"), b"");
        }
```

(`HashMap` already imported in store.rs; `local_timestamp()`, `flush()`, `flush_pins()` verified present; `?` propagates `StoreError` exactly like neighboring code. Corrupt `presets.json` loads as empty per existing behavior → no backup, sentinel still written — correct: nothing to preserve.)

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | Select-String "test result|FAILED"` (from `src-tauri/`)
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/store.rs
git commit -m "feat: one-shot backup-and-wipe for monitor identities"
```

---

### Task 3: Hardware verification + release build

**Files:** none (verification only).

- [ ] **Step 1: Unit gates re-run**

Run: `cargo test` (src-tauri), `npx tsc --noEmit`, `npx vitest run` (root).
Expected: all green (record exact counts in the report; monitor.rs holds 11 tests: 7 existing + 4 new).

- [ ] **Step 2: Real names, no GPU rows**

Run: `npm run tauri dev` (repo root). Open the app → monitor list AND the preset-editor monitor dropdown must show physical panels by real names: the external panel by its EDID name (observed `24B36XE`-class on this hardware), the laptop panel by its constructed `MFR PRODUCT` tag (it carries no EDID name — `Generic PnP Monitor` must NOT appear where EDID data exists). Zero entries named like a GPU ("NVIDIA GeForce…", "Intel … Graphics"). Screenshot or transcribe the list into the report.

- [ ] **Step 3: ID stability across Refresh**

Click Refresh (or restart the app). Record `edid_id`s: identical across runs. Reboot-level stability is by EDID construction; Refresh covers the app side.

- [ ] **Step 4: Correct per-monitor targeting**

Create one preset per monitor with visibly different brightness (e.g. 30 vs 80). Apply each: confirm ONLY the targeted monitor dims (the other stays put). This proves `device_name` still routes gamma to the right adapter output under the new enumeration.

- [ ] **Step 5: Legacy wipe observed (first launch only)**

Before first launch with the new build, confirm `%APPDATA%\ChromaDeck\presets.json` holds GPU-keyed presets. After first launch: `presets.json.bak-<timestamp>` exists with the originals, `presets.json` is `[]`, `.monitor-enumeration-v2` exists, and second launch creates no further backups.

- [ ] **Step 6: Release build**

Run: `npm run tauri build` (repo root). Expected: MSI + NSIS produced, no errors.

- [ ] **Step 7: Report (no commit — no code changes)**

Report per-step outcomes with the recorded monitor names/IDs. On ANY failure: STOP, return evidence, do not stack fixes.
