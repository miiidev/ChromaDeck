# Post-Meetup Fixes: Library Grid, Auto-Apply, Reset-All, Monitor Diagnostics — Design Spec

**Status:** approved in chat (2026-11-08); all builder-check questions answered.
**Batch:** feedback from demo-day meetups on v0.8.2. Five items; ordered by
diagnostic dependency, projector parked last (no hardware available).

## Goal

Address the five meetup notes:

1. Presets library requires too much scrolling in the long horizontal carousel.
2. Auto-apply: when a mapped program opens, its preset applies to the screen
   the program is displayed on. One preset may map many programs.
3. A "Reset all monitors" action.
4. Third-monitor test made the second monitor vanish from the list entirely.
5. Projector apply errors mentioning NVAPI — **deferred**, revisit with
   hardware (expected fix: graceful per-backend degradation instead of hard
   failure).

Non-goals: no pin-system redesign, no backend rework, no new dependencies
beyond what item 2 strictly needs (expect none — pure Win32 via `windows`
crate already in tree).

## 1. Presets library — dense grid with in-place filters (chosen: approach 1)

Approaches were 1) dense grid + filters, 2) search-first quick-jump on the
carousel, 3) compact grouped rows list. Chose 1: the complaint is about
*seeing* what exists, not finding a known preset; grid grows visible density
4–6× while keeping the apply-first card identity.

- Deck becomes a responsive CSS grid (compact card variant: swatch-strip
  preview, name, tag, actions on hover/keyboard focus).
- Existing search box and tag chips become live filters that prune the grid
  in place — no pagination, no page changes, zero-results state explained.
- Motion: pop-in/explode and dissolve carry over, density-scaled;
  reduced-motion path already handled and preserved.
- Carousel mode stays available behind a view toggle (grid default) — full
  cards retained for presentation moments.

## 2. Auto-apply — follow the window (chosen: approach A, not fixed-per-preset)

Add a `attached_programs: Vec<String>` (process base names, lowercased
compare) to presets; editable in PresetEditor as a chip row (reuse the
existing tag-chip interaction).

- **Detection:** background poll (~1s) reads the foreground window's process
  base name via `GetForegroundWindow` + `QueryFullProcessImageNameW`. Match
  against mappings; on a *new* match (program switch, not already-applied),
  apply the preset to the monitor the window currently sits on
  (`MonitorFromWindow` → adapter path → `edid_id`). Moving a window between
  screens re-derives the target.
- **Pipe:** auto fires go through the standard apply pipeline (same
  outcome/adjustment contract as manual apply, no user action needed) for
  gamma/vibrance/hue; per-backend outcomes report to a
  `auto-apply://outcome` event → small "Auto-applied `<preset>` to
  `<monitor>`" toast. Color changes must never be mysterious.
- **Conflicts:** a pinned monitor is never auto-touched; skip + info toast.
  Pin (user intent) beats rule (automation).
- **Revert on close:** when ALL mapped programs for that preset stop being
  foreground and that monitor is currently colored by auto-apply (tracked
  in-memory, not persisted), reset that monitor to system identity. If no
  program ever launched while we were watching (cold start mid-run),
  auto takes no action to avoid stomping existing state.
- **Bookkeeping (decided):** auto state is purely in-memory — never written
  to `applied.json` and never restarted after relaunch (after a restart the
  screen shows plain default until a mapped program opens again). While
  auto is active the monitor's status line shows the preset as active with
  an obvious **auto** marker; the preset stays visible in the library
  carrying the same marker (hand-applied presets leaving the library
  behavior is unchanged and manual-only). On revert, all markers clear.
- **ICC (decided):** the auto-apply arm skips the ICC step entirely, so
  revert-on-close leaves zero system residue.
- **Master switch (decided):** settings toggle + tray checkbox, persisted,
  default OFF, enabled explicitly. The feature is inert until a mapping
  exists; adding mappings does not auto-flip the switch.
- **Uniqueness (decided):** a program may map to only one preset at a
  time; PresetEditor rejects a duplicate chip with an inline
  "already mapped to `<preset>`" hint.

## 3. Reset-all monitors

Outcome (decided): **unpin everything** — a manual Reset-all is
user intent for a fully clean slate.

- New command `reset_all_monitors_cmd`: iterate all *connected* monitors
  through the existing per-monitor reset path **and clear every pin**
  (otherwise the enforcer repaints pinned monitors within its ~10s tick and
  the button visibly undoes itself). Returns the outcome list.
- ICC associations are not dissociated (consistent with the existing
  per-monitor reset philosophy; no new capability).
- UI: "Reset all" button in the monitor bar header with a confirm dialog —
  copy must state that pins are lifted: "resets every connected screen to
  default colors and removes all pins". Toast summarizing outcomes after.

## 4. Monitor-list collapse — diagnostics first

Hypothesis from review: no dedupe exists in the chain, so a monitor
vanishing *entirely* is either (a) two panels resolving the **same
`edid_id`** — the EDID registry reader matches only by model segment, so
same-model panels or blank-serial EDIDs collide and the app's everything-keyed-
on-edid_id model collapses them — or (b) `EnumDisplayDevicesW` child
enumeration temporarily dropping a child entry during hot-plug re-push.

- **Diagnostic readout (build first, decided):** per detected device: instance
  path, EDID blob presence/length, parsed serial/model/manufacturer,
  resolved `edid_id`, adapter device name + state flags — shown in the
  Identify readout **and** appended to a rolling diagnostics log
  (`data/diagnostics.log`, timestamped, appended on every enumeration
  snapshot change). The identity-collision and transient hot-plug cases both
  become capturable in a repro.
- **Collision case:** `edid_id` disambiguation — keep EDID identity, append a
  port-anchored discriminator only when two live monitors in the same
  snapshot resolve to the same key; alias/pin/applied lookups fall back
  gracefully (alias keeps working via EDID key match first).
- **Hard prerequisite (decided):** the collision fix lands **before**
  auto-apply, because `MonitorFromWindow → adapter → edid_id` targeting
  requires unique keys; under collisions auto-apply would target an
  arbitrary twin.
- **Enumeration case:** keep offline entries visible (merge already does);
  investigate the child-walk gap with the readout logs from a real repro.

## 5. Projector / NVAPI apply error — deferred

No projector available until next session. Expected shape of the fix per
`backend/mod.rs`: map "adapter not resolvable by NVAPI" (projector on
non-NVIDIA path) into per-backend warnings with ICC + OS-gamma fallback
still applying, surfaced as a partial-success outcome — not a hard error.
Revisit with hardware; capture the exact error string from the diagnostic
readout/toast.

## Sequencing

Diagnostics readout + rolling log (1) → collision fix if confirmed by a
repro → reset-all (small win, independent) → library grid (UI, independent)
→ auto-apply (largest; strictly after the collision fix) → projector when
hardware returns. Items reset-all and library grid don't block each other;
auto-apply lands last so unique monitor keys and the outcomes/toast
discipline from earlier items are in place.

## Testing / gates

- Rust: store CRUD for attached programs; pure fns for process-name matching
  and monitor-target derivation extracted and unit-tested; reset-all mock
  outcomes; collision-disambiguation unit tests.
- Frontend: grid filter pruning, zero-state, toggle persistence, toast wiring
  (vitest; node env per existing suite).
- Hardware: 2-panel same-model repro for the collision case; 3-monitor
  hot-plug repro with readout logs; mapped-program open/close cycle.
- Gate before merge: `cargo test`, `npm run test`, `npm run build` green.
