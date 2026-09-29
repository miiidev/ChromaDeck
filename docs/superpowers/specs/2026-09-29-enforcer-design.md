# Preset Enforcer (Resident Re-apply) — Design Spec

**Status:** sections 1–5 approved in chat (2026-09-29); awaiting spec review.
**Approach:** in-process enforcer (discarded: separate watcher process,
scheduled-task hacks).

## Goal

Keep the pinned preset per monitor actually applied despite
state evaporation (exclusive-fullscreen games resetting the gamma LUT,
HDR toggles, dock/undock, driver updates). No focus hooking in v1.

## Background (researched)

DisplayCAL (3s reloader), novideo_srgb (resident reapply + display-change
handling), and VibranceGUI (tray-resident, desktop-default + revert)
all converge on: stored presets are truth, live driver state is not,
and a resident agent re-applies. Our vibrance/hue application is
proven bit-exact; the failure mode is something else rewriting state
after us.

## 1. Pin state

- Store gains `pinned: { edid_id: preset_id }` (`HashMap<String, String>`),
  persisted alongside presets, default empty.
- Pin is offered per preset card (a preset belongs to exactly one
  monitor via its own `edid_id`, so no cross-monitor pinning exists).
  Unpin replaces Pin where pinned. UI shows a pinned badge.
- Deleting a preset removes any pin pointing at it (cascade).
- Pins survive monitor offline periods and are enforced on return.
- **Reset unpins that monitor**: Reset restores identity AND stops
  enforcement there, otherwise the enforcer would undo the reset
  within one tick.

## 2. Enforcer loop

- Single async task, 10s tick. Each tick: re-enumerate monitors
  (topology changes detected here — no Win32 message hooks), then for
  each pinned + connected preset, drift-check and reapply only on drift
  (compare-before-write: no flicker, no redundant driver writes).
- Drift check: freshly-built ramp (`build_gamma_ramp`, floor included)
  memcmp'd against `GetDeviceGammaRamp`; DVC current level and hue
  angle read vs preset values (exact integer compare).
- Reapply covers gamma + vibrance/hue only. ICC associations persist
  across LUT resets, so ICC stays manual-Apply-only.
- Shared `apply_color(api, nv, preset)` extracted from `apply_preset`
  and reused by the enforcer (ICC step stays in `apply_preset` only).
- Manual "Reapply now" forces a full pass unconditionally.

## 3. Tray + residency

- System tray menu: Show / Reapply now / Quit. Window close hides to
  tray instead of quitting. Tooltip shows enforcing count.
- Single-instance: relaunch focuses the existing window
  (`tauri-plugin-single-instance`).
- Autostart toggle via `tauri-plugin-autostart`, default OFF
  (residency is explicit opt-in).
- New Rust deps only: the two Tauri plugins above. No new OS APIs.

## 4. Frontend

- `PresetCard`: Pin/Unpin button + pinned badge.
- Footer: enforcing indicator (count of pinned monitors).
- Settings row (Library header or footer): autostart checkbox +
  Reapply-now button (mirrors tray actions).
- New wrappers in `src/lib/tauri.ts` for pin/unpin/pinned-list/
  reapply-now/autostart-state (camelCase args per convention).

## 5. Error handling

- Offline pinned monitor: skipped silently until it returns.
- NVAPI unavailable at enforce time: reported once per newly-seen
  failure (no per-tick error spam), gamma still enforced.
- Enforcer never touches unpinned monitors.

## 6. Testing

- Unit: pin/unpin CRUD, delete-cascade, drift-compare pure fns,
  neutral-skip preserved, Reset-unpins.
- Mock-command tests for new Tauri commands.
- Hardware (this machine): externally clobber the LUT (script),
  observe auto-restore within one tick; neutral preset produces zero
  driver writes across ticks; dock/undock if available.
- Gate before merge: `cargo test`, `npx tsc --noEmit`,
  `npx vitest run`, `npm run tauri build` all green.

## Accepted risks / deferred

- 10s poll may feel slow after fullscreen exit; foreground-window
  tracking is the documented follow-up, not v1.
- Exact LUT memcmp assumes round-trip fidelity (observed true here);
  if a driver quantizes, fall back to tolerance compare.
