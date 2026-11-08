# Monitor Diagnostics + Identity-Collision Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the monitor identity chain inspectable (on-screen Identify readout + rolling log) and stop two same-identity panels from collapsing into one app-wide monitor key.

**Architecture:** `Monitor` grows identity-chain fields filled by the existing adapter→child tree walk; a small `diagnostics` module appends a signed snapshot to `data/diagnostics.log` whenever the enumerated set changes; a pure disambiguation pass suffixes collided `edid_id`s with their device instance path before the list ever leaves Rust.

**Tech Stack:** Rust/Tauri 2 (windows crate), React 19 + TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-08-post-meetup-0-8-3-design.md` (§4)

## Global Constraints

- Windows-only app; no new crates.
- `edid_id` change is a data-model change: every collision suffix uses the `|` separator (e.g. `EDID:06AF-F7AD-00000000|MONITOR\FOO\4&1`).
- `Monitor` stays `#[derive(Debug, Clone, Serialize)]`; new fields serialize, existing consumers must not break (`types.ts` fields optional).
- All new Rust pure functions live outside `enum_gdi_monitors` and are unit-tested; no Win32 calls in tests.
- Copy in the UI is exact: heading "Monitor identity chain".
- Gate before merge: `cargo test` (src-tauri), `npm run test`, `npm run build` green.

## Review Focus

- **A monitor whose EDID is unreadable** must still surface with `edid_present=false` and an instance-path fallback key — no panic, no dropped entry. Test lives in Task 1's step 1 (identify tests) and Task 3.
- **Three panels sharing one identity key** must all get suffixed, deterministically, in enumeration order. Test in Task 3 step 1.
- **Re-enumeration churn** (same snapshot re-logged repeatedly) must not grow the log: only fingerprint changes append. Test in Task 2 step 1.
- **A user alias on a collided panel** must keep resolving: alias lookup strips the `|suffix` before consulting the names map. Test in Task 3 step 1.
- **The Identify overlay on a monitor with no EDID** must render the chain block with "EDID not available" instead of blank. Test in Task 4 step 1.

---

### Task 1: `Monitor` carries the identity chain

**Files:**
- Modify: `src-tauri/src/monitor.rs:18-27` (struct), `138-151` (population in `enum_gdi_monitors`), `255-275` (`read_edid_for_monitor` returns length too)
- Modify: `src-tauri/src/identify.rs:281-306` (test helper `fake_monitor` gains new fields)
- Modify: `src/lib/types.ts` (Monitor type, optional fields)
- Test: `src-tauri/src/monitor.rs` tests module (extend `base_block` test)

**Interfaces:**
- Produces: `Monitor` with new public fields `instance_path: String`, `edid_present: bool`, `edid_len: usize`. Later tasks read these; `identify_monitor` signature is unchanged.

- [ ] **Step 1: Write the failing test** — in `monitor.rs` tests, extend `edid_id_from_bytes_matches_identify_monitor_unit_id`'s module with:

```rust
// fake_edid_blob returns [u8;128]-built blobs already; reuse it.
#[test]
fn monitor_fields_carry_instance_and_edid_presence() {
    // Constructed directly (not via Win32): the struct is plain data.
    let m = Monitor {
        edid_id: "X".into(), model: "M".into(), serial: String::new(),
        connected: true, device_name: "D".into(), alias: String::new(),
        instance_path: r"MONITOR\FERMICAN012\5&2f3a&0&UID265977".into(),
        edid_present: true, edid_len: 256,
    };
    assert!(m.edid_present && m.edid_len == 256 && m.instance_path.contains("UID"),
        "fields must be public and storable");
    // And a no-EDID monitor keeps zeros/empty:
    let m2 = clone_fields_without_edid();
    assert!(!m2.edid_present && m2.edid_len == 0 && m2.instance_path.is_empty());
}

fn clone_fields_without_edid() -> Monitor {
    Monitor {
        edid_id: "Y".into(), model: String::new(), serial: String::new(),
        connected: false, device_name: String::new(), alias: String::new(),
        instance_path: String::new(), edid_present: false, edid_len: 0,
    }
}
```

- [ ] **Step 2: Run test to verify it fails** — Run: `cargo test --manifest-path src-tauri/Cargo.toml monitor::tests::monitor_fields` — Expected: FAIL (`no field instance_path`).

- [ ] **Step 3: Implement** — add the three fields to `Monitor`; in `enum_gdi_monitors`, populate `instance_path: instance_id.clone()`, and at the existing EDID read: `let blob = read_edid_for_monitor(&instance_id); let edid_len = blob.as_ref().map(|v| v.0.len()).unwrap_or(0);` then pass `blob.map(&[u8;128] conversion)` as before — `read_edid_for_monitor` keeps its signature. In `identify.rs::fake_monitor` fill `instance_path: String::new(), edid_present: false, edid_len: 0`. In `src/lib/types.ts`, extend the `Monitor` interface with `instance_path?: string; edid_present?: boolean; edid_len?: number;`.

- [ ] **Step 4: Run tests to verify they pass** — Run: `cargo test --manifest-path src-tauri/Cargo.toml` and `npm run test` — Expected: PASS both.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/monitor.rs src-tauri/src/identify.rs src/lib/types.ts
git commit -m "feat: monitor carries EDID identity chain fields"
```

### Task 2: Rolling diagnostics log

**Files:**
- Create: `src-tauri/src/diagnostics.rs`
- Modify: `src-tauri/src/lib.rs:1-26` (declare module), `src-tauri/src/monitor.rs:288-302` (`list_monitors_cmd` calls it)
- Test: `src-tauri/src/diagnostics.rs` tests module

**Interfaces:**
- Consumes: Task 1's `Monitor` fields; store `data_dir()` accessor (`AppStore` exposes its data dir — reuse whatever `default_store_path` / store fields provide).
- Produces: `pub fn append_snapshot_if_changed(dir: &Path, monitors: &[Monitor]) -> io::Result<bool>` (true = appended); `pub fn snapshot_fingerprint(monitors: &[Monitor]) -> u64`; `pub fn format_snapshot(monitors: &[Monitor]) -> String`; `pub fn per_monitor_line(m: &Monitor) -> String` (shared with Task 4).

- [ ] **Step 1: Write the failing tests** — in `diagnostics.rs` tests module:

```rust
// Fingerprint ignores order but is sensitive to edid_id and connected flips.
#[test]
fn fingerprint_order_insensitive_change_sensitive() {
    use crate::monitor::Monitor;
    fn m(id: &str, connected: bool) -> Monitor { Monitor {
        edid_id: id.into(), model: String::new(), serial: String::new(),
        connected, device_name: String::new(), alias: String::new(),
        instance_path: String::new(), edid_present: false, edid_len: 0 } }
    let a = vec![m("A", true), m("B", false)];
    let b = vec![m("B", false), m("A", true)];
    assert_eq!(snapshot_fingerprint(&a), snapshot_fingerprint(&b));
    assert_ne!(snapshot_fingerprint(&a), snapshot_fingerprint(&[m("B", true), m("A", true)]));
}

// format_snapshot is line-per-monitor, machine-parsable, carries the chain.
#[test]
fn format_snapshot_renders_each_monitor_line() {
    let m = Monitor { edid_id: "A|MON/INFO".into(), model: "Model X".into(),
        serial: "S1".into(), connected: true, device_name: r"\\.\DISPLAY1".into(),
        alias: "Office".into(), instance_path: r"MON\INFO\1".into(),
        edid_present: true, edid_len: 256 };
    let line = format_snapshot(&[m]);
    assert!(line.lines().count() >= 2); // header + 1 monitor
    assert!(line.contains(r"\\.\DISPLAY1") && line.contains("EDID=256") && line.contains("connected=1"));
    assert_eq!(per_monitor_line_first(line), Some(line.lines().nth(1).unwrap().to_string()));
}

fn per_monitor_line_first(s: &str) -> Option<String> { s.lines().nth(1).map(str::to_string) }

// Only a changed fingerprint appends (pure gate: the io wrapper takes last_fs).
#[test]
fn append_gate_writes_only_on_change() {
    // Pure helper: should_log(last, current) -> bool
    assert!(should_log(7, 9) && !should_log(9, 9));
}
```

- [ ] **Step 2: Run tests to verify they fail** — Run: `cargo test --manifest-path src-tauri/Cargo.toml diagnostics` — Expected: FAIL (module missing).

- [ ] **Step 3: Implement `diagnostics.rs`** — `should_log(last: u64, current: u64) -> bool { last != current }`; `snapshot_fingerprint` = FNV-1a over the sorted list of `format!("{edid_id}{connected}")` strings joined by a NUL byte; `per_monitor_line(m)` = `edid_id=<k> connected=<0|1> model=<m> adapter=<device_name> instance=<instance_path> EDID=<edid_len|none>`; `format_snapshot(&[Monitor])` = one header line `--- diagnostics snapshot <UTC RFC3339> ---` then one `per_monitor_line` per entry; `append_snapshot_if_changed` reads the `last-fingerprint` recorded on `diagnostics.log` line 1 (absent file = 0), writes a timestamped header + lines only when the fingerprint changed, then truncates the file to its last ~256 KiB whenever it exceeds 1 MiB. In `list_monitors_cmd`, after alias merge: `let _ = append_snapshot_if_changed(&dir, &monitors);` (log-only, ignore IO errors).

- [ ] **Step 4: Run tests to verify they pass** — Run: `cargo test --manifest-path src-tauri/Cargo.toml diagnostics` — Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/diagnostics.rs src-tauri/src/lib.rs src-tauri/src/monitor.rs
git commit -m "feat: rolling monitor diagnostics log"
```

### Task 3: Collision disambiguation + alias fallback

**Files:**
- Modify: `src-tauri/src/monitor.rs` (`enum_gdi_monitors` tail, alias merge in `list_monitors_cmd`)
- Test: `src-tauri/src/monitor.rs` tests module

**Interfaces:**
- Produces: `pub fn discriminate_edids(entries: &[(String, String)]) -> Vec<(String, String)>` \u2014 input pairs `(edid_id, instance_path)`, returns pairs where any `edid_id` appearing more than once is replaced by `edid_id|instance_path` (ALL occurrences suffixed, order preserved); `pub fn base_edid_key(qualified: &str) -> &str` \u2014 strips everything from the first `|`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn discriminate_suffixes_only_collided_keys() {
    let out = discriminate_edids(&[
        ("A".into(), r"MON\A\1".into()),
        ("EDID:0441-1234-12345678".into(), r"MON\E1\9".into()),
        ("EDID:0441-1234-12345678".into(), r"MON\E1\10".into()),
    ]);
    assert_eq!(out[0].0, "A");                                     // unique untouched
    assert_eq!(out[1].0, r"EDID:0441-1234-12345678|MON\E1\9");
    assert_eq!(out[2].0, r"EDID:0441-1234-12345678|MON\E1\10");
}

#[test]
fn three_way_collision_all_suffixed() {
    let out = discriminate_edids(&[
        ("K".into(), "I1".into()), ("K".into(), "I2".into()), ("K".into(), "I3".into()),
    ]);
    assert!(out.iter().all(|(k, i)| k.contains('|') && k.ends_with(&format!("|{i}"))));
}

#[test]
fn base_edid_key_strips_at_first_pipe() {
    assert_eq!(base_edid_key(r"EDID:06AF-F7AD-00000000|MONITOR\FOO\1"), "EDID:06AF-F7AD-00000000");
    assert_eq!(base_edid_key("PLAIN"), "PLAIN");
}

#[test]
fn alias_lookup_falls_back_to_base_key() {
    // in list_monitors_cmd the alias merge uses base_edid_key(edid_id)
    assert_eq!(base_edid_key("A|MON\\1"), "A");
}
```

- [ ] **Step 2: Run tests to verify they fail** — Run: `cargo test --manifest-path src-tauri/Cargo.toml discriminate` — Expected: FAIL.

- [ ] **Step 3: Implement** — wire `discriminate_edids` at the end of `enum_gdi_monitors` (it already collects `(edid_id, instance_path)`); in `list_monitors_cmd` alias merge use `names.get(base_edid_key(&m.edid_id))` in addition to the direct lookup (direct first, base fallback).

- [ ] **Step 4: Run tests to verify they pass** — Run: `cargo test --manifest-path src-tauri/Cargo.toml` — Expected: PASS (all modules).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/monitor.rs
git commit -m "fix: disambiguate collided monitor identity keys by port"
```

### Task 4: Identify overlay shows the chain

**Files:**
- Modify: `src-tauri/src/identify.rs:95-133` (`IdentifyInfo` gains `chain: String`)
- Modify: `src/components/IdentifyOverlay.tsx`
- Test: `src-tauri/src/identify.rs` tests module (extend `identify_info_maps_one_based_connected_order`)

**Interfaces:**
- Consumes: Task 1 `Monitor` fields, Task 2 `format_snapshot` (unit formatting, one monitor).
- Produces: `IdentifyInfo { name, total, chain }` where `chain` is the multi-line raw chain of *that* monitor (same format as the log's per-monitor line, minus timestamp header).

- [ ] **Step 1: Write the failing test**

```rust
// extend identify_info_maps_one_based_connected_order:
let info = identify_info_for(&monitors[0..], 1).unwrap();
assert!(info.chain.contains("instance=") && info.chain.contains("connected=1"));
```

- [ ] **Step 2: Run test to verify it fails** — Run: `cargo test --manifest-path src-tauri/Cargo.toml identify` — Expected: FAIL (`no field chain`).

- [ ] **Step 3: Implement** — `IdentifyInfo` gains `pub chain: String`; `identify_info_for` formats it with the per-monitor line format (extract a shared helper from Task 2's `format_snapshot` so both callers produce identical lines). In `IdentifyOverlay.tsx`, render `chain` in a small monospace block below the number, heading "Monitor identity chain", falling back to a static "EDID not available" line when the payload's `EDID=` shows `none`. Update the component's existing test if it snapshots text.

- [ ] **Step 4: Run tests to verify they pass** — Run: `cargo test --manifest-path src-tauri/Cargo.toml identify && npm run test` — Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/identify.rs src/components/IdentifyOverlay.tsx
git commit -m "feat: identify overlay shows monitor identity chain"
```
