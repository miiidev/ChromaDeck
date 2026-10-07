import { describe, it, expect } from "vitest";
import { mergeMonitors } from "./monitorMerge";
import type { Monitor } from "./types";

/** Builder helper for test monitors. */
function mon(overrides: Partial<Monitor> & { edid_id: string }): Monitor {
  return {
    model: "Test Model",
    serial: "",
    connected: true,
    device_name: "DISPLAY1",
    alias: "",
    ...overrides,
  };
}

describe("mergeMonitors", () => {
  it("updates known monitors in place with fresh data", () => {
    const prev = [mon({ edid_id: "A", model: "Old Model" })];
    const fresh = [mon({ edid_id: "A", model: "New Model" })];
    const result = mergeMonitors(prev, fresh);
    expect(result).toHaveLength(1);
    expect(result[0].model).toBe("New Model");
    expect(result[0].connected).toBe(true);
  });

  it("keeps monitors absent from fresh as offline (vanish)", () => {
    const prev = [
      mon({ edid_id: "A", alias: "Left" }),
      mon({ edid_id: "B", alias: "Right" }),
    ];
    const fresh = [mon({ edid_id: "A" })];
    const result = mergeMonitors(prev, fresh);
    expect(result).toHaveLength(2);
    expect(result[0].edid_id).toBe("A");
    expect(result[0].connected).toBe(true);
    expect(result[1].edid_id).toBe("B");
    expect(result[1].connected).toBe(false);
    expect(result[1].alias).toBe("Right"); // other fields preserved
  });

  it("appends brand-new monitors from fresh", () => {
    const prev = [mon({ edid_id: "A" })];
    const fresh = [mon({ edid_id: "A" }), mon({ edid_id: "C" })];
    const result = mergeMonitors(prev, fresh);
    expect(result).toHaveLength(2);
    expect(result[1].edid_id).toBe("C");
    expect(result[1].connected).toBe(true);
  });

  it("re‑appeared monitor comes back online with fresh data", () => {
    // A was present, then absent (→offline), then reappears
    const prev = [
      mon({ edid_id: "B", connected: false, model: "Old", serial: "S1" }),
    ];
    const fresh = [mon({ edid_id: "B", model: "New Model", serial: "S2" })];
    const result = mergeMonitors(prev, fresh);
    expect(result).toHaveLength(1);
    expect(result[0].connected).toBe(true);
    expect(result[0].model).toBe("New Model");
    expect(result[0].serial).toBe("S2");
  });

  it("keeps all prev monitors when fresh is empty (all offline)", () => {
    const prev = [
      mon({ edid_id: "A", alias: "Alpha" }),
      mon({ edid_id: "B", alias: "Beta" }),
    ];
    const result = mergeMonitors(prev, []);
    expect(result).toHaveLength(2);
    expect(result[0].connected).toBe(false);
    expect(result[0].edid_id).toBe("A");
    expect(result[1].connected).toBe(false);
    expect(result[1].edid_id).toBe("B");
  });

  it("preserves alias and serial on offline entries", () => {
    const prev = [mon({ edid_id: "Z", alias: "My Monitor", serial: "SN001" })];
    const result = mergeMonitors(prev, []);
    expect(result[0].alias).toBe("My Monitor");
    expect(result[0].serial).toBe("SN001");
    expect(result[0].connected).toBe(false);
  });

  it("returns empty when both prev and fresh are empty", () => {
    expect(mergeMonitors([], [])).toEqual([]);
  });
});