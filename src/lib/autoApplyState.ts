/**
 * Pure reducer for auto-apply outcome events → the in-memory marker map.
 *
 * Auto-applied presets stay visible in the library, so this map is kept
 * separate from `appliedMap` (which hides presets). Keys are monitor EDIDs,
 * values are preset ids.
 */

import type { AutoApplyEvent } from "./tauri";

export type AutoMap = Record<string, string>;

export function applyAutoEvent(map: AutoMap, event: AutoApplyEvent): AutoMap {
  switch (event.kind) {
    case "applied": {
      if (!event.edid_id || !event.preset_id) return map;
      if (map[event.edid_id] === event.preset_id) return map;
      return { ...map, [event.edid_id]: event.preset_id };
    }
    case "reverted":
    case "error": {
      // A failed apply leaves nothing to mark; a failed revert clears the
      // marker too (the colour state is no longer trustworthy).
      if (!event.edid_id || !(event.edid_id in map)) return map;
      const next = { ...map };
      delete next[event.edid_id];
      return next;
    }
    case "cleared": {
      if (!event.edid_id) return Object.keys(map).length === 0 ? map : {};
      if (!(event.edid_id in map)) return map;
      const next = { ...map };
      delete next[event.edid_id];
      return next;
    }
    case "skipped":
    default:
      return map;
  }
}
