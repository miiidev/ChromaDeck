# Real-Monitor Enumeration — Design Spec

**Status:** design approved in chat (2026-09-30); awaiting spec review.
**Approach:** C — adapter→monitor tree walk via EnumDisplayDevices.
Rejected: A (fix SetupAPI-first — proven flaky on real hardware),
B (DisplayConfig-centric — heaviest marshaling for no identity gain).

## Goal

The display list shows physical monitors by real name (EDID model),
never GPU adapters. Identity is EDID-based and stable across
dock/reconnect/reboot. Existing GPU-keyed presets are wiped with a
backup (explicit user call).

## Background (verified this session)

- Current fallback enumerates `EnumDisplayDevices(NULL, i)` only, which
  yields ADAPTER entries (e.g. "NVIDIA GeForce RTX 3050",
  `PCI\VEN_...` IDs). Presets keyed by these IDs identify GPUs, not
  monitors.
- `EnumDisplayDevices(adapterName, j)` yields child MONITOR entries
  with `DeviceString` (human name) and `DeviceID` (instance path
  `MONITOR\...`, usable for EDID registry lookup).
- The driving adapter's `\\.\DISPLAYn` GDI name is structurally in hand
  during the walk — gamma DC creation and NVAPI displayId lookup keep
  working with no correlation step.

## 1. Enumeration rewrite (`monitor.rs` only)

- Outer loop over adapters (`EnumDisplayDevicesW(None, i)`); inner loop
  over children (`EnumDisplayDevicesW(adapterName, j)`). Adapters with
  zero children are skipped entirely (no GPU rows ever).
- Each monitor entry: `{ edid_id, model, serial, connected, device_name }`
  - `edid_id`: EDID serial descriptor (`0xFF`) → else manufacturer +
    product + numeric serial → else full instance path. First
    non-empty wins.
  - `model`: EDID name descriptor (`0xFC`) → else child `DeviceString`.
  - `serial`: EDID serial string (may be empty).
  - `connected`: child `StateFlags & ATTACHED_TO_DESKTOP`, plus adapter
    must also be attached.
  - `device_name`: parent adapter `\\.\DISPLAYn` (unchanged field name
    and shape).
- Delete the SetupAPI iteration and `enrich_with_device_names`; delete
  the adapter-level fallback. EDID parsing helpers and registry reader
  stay as-is (now fed instance paths from child `DeviceID`s).

## 2. Identity stability

- EDID serial survives dock/reconnect/reboot and GPU renumbering
  (`\\.\DISPLAYn` indices may shift; identity must never depend on them
  — the app already looks up device names fresh on every apply).
- Fallback chain guarantees a non-empty `edid_id` for every listed
  monitor, including EDID-less/virtual panels.

## 3. Downstream contract (unchanged)

- `Monitor` struct field names and shapes are frozen. NVAPI displayId
  lookup, gamma DC creation, vibrance probe, pins, store, and export
  all consume `edid_id` + `device_name` as before — only the contents
  become correct. No changes outside `monitor.rs` except the store
  backup step below.

## 4. Old-data handling (user-approved: wipe and restart)

- Detection is an empty sentinel file `.monitor-enumeration-v2` in the
  store directory (no schema change to `presets.json`, which stays a
  bare array). If `presets.json` exists, is non-empty, and the sentinel
  is absent: copy it to `presets.json.bak-<yyyymmdd-HHMMSS>`, start with
  an empty preset list, and write the sentinel. Same for a non-empty
  `pins.json` (backup, then empty). Runs after the normal store load so
  a corrupt `presets.json` still degrades to empty as today.
- No ID migration attempted. The backup preserves the old data for
  manual reference.

## 5. Error handling

- Adapter enumeration failure → empty list (UI shows existing empty
  state), never a panic, never GPU rows.
- Child with unreadable EDID → fallbacks per §1, still listed.
- Unit-testable: EDID fallback chain (extend existing tests), adapter
  JSON fixtures for grouping logic if extracted as pure functions.

## 6. Testing

- Unit: EDID serial → model+serial → instance-path fallback chain;
  adapter-without-children skipped (mocked EnumDisplayDevices data
  where possible, pure-function extraction where not).
- Hardware (this machine): monitor list must show the AOC panel and the
  laptop panel by real model names, zero GPU rows; IDs stable across
  Refresh; apply a test preset to each and confirm correct targeting.
- Gate before merge: `cargo test`, `npx tsc --noEmit`,
  `npx vitest run`, `npm run tauri build` all green.

## Accepted risks

- Some exotic panels report no EDID at all — covered by the fallback
  chain, identified by instance path.
- If a future Windows changes child-enumeration behavior, the empty
  list degrades to the existing empty UI state (no crash path added).
