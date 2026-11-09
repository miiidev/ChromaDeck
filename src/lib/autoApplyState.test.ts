import { describe, expect, it } from "vitest";
import { applyAutoEvent, type AutoMap } from "./autoApplyState";
import type { AutoApplyEvent } from "./tauri";

function event(
  kind: AutoApplyEvent["kind"],
  edid: string,
  presetId = "",
): AutoApplyEvent {
  return {
    kind,
    edid_id: edid,
    preset_id: presetId,
    preset_name: "",
    monitor_name: "",
    program: "",
    error: null,
  };
}

describe("applyAutoEvent", () => {
  it("marks the monitor on applied", () => {
    expect(applyAutoEvent({}, event("applied", "D1", "p1"))).toEqual({
      D1: "p1",
    });
  });

  it("keeps the same object when nothing changes", () => {
    const map: AutoMap = { D1: "p1" };
    expect(applyAutoEvent(map, event("applied", "D1", "p1"))).toBe(map);
  });

  it("clears the monitor on reverted", () => {
    expect(applyAutoEvent({ D1: "p1", D2: "p2" }, event("reverted", "D1"))).toEqual({
      D2: "p2",
    });
  });

  it("clears the monitor on error", () => {
    expect(applyAutoEvent({ D1: "p1" }, event("error", "D1"))).toEqual({});
  });

  it("clears everything on a broadcast cleared", () => {
    expect(applyAutoEvent({ D1: "p1", D2: "p2" }, event("cleared", ""))).toEqual({});
  });

  it("clears only one monitor on a targeted cleared", () => {
    expect(applyAutoEvent({ D1: "p1", D2: "p2" }, event("cleared", "D2"))).toEqual({
      D1: "p1",
    });
  });

  it("leaves the map untouched on skipped", () => {
    const map: AutoMap = { D1: "p1" };
    expect(applyAutoEvent(map, event("skipped", "D2"))).toBe(map);
  });
});
