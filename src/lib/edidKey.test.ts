import { describe, it, expect } from "vitest";
import { baseEdidKey, edidKeyMatch, valueFor } from "./edidKey";

describe("baseEdidKey", () => {
  it("strips the port discriminator at the first pipe", () => {
    expect(baseEdidKey("EDID:06AF-C7AD-00000000|MONITOR\\X\\1")).toBe(
      "EDID:06AF-C7AD-00000000",
    );
  });
  it("passes plain keys through", () => {
    expect(baseEdidKey("EDID:06AF-C7AD-00000000")).toBe("EDID:06AF-C7AD-00000000");
    expect(baseEdidKey("")).toBe("");
  });
});

describe("edidKeyMatch", () => {
  it("exact always wins, in either direction", () => {
    expect(edidKeyMatch("A", "A")).toBe(true);
    expect(edidKeyMatch("A|MON\\1", "A|MON\\1")).toBe(true);
  });
  it("legacy plain target matches a collided qualified entry", () => {
    expect(edidKeyMatch("A|MON\\8", "A")).toBe(true);
  });
  it("stale qualified target matches the surviving base entry", () => {
    expect(edidKeyMatch("A", "A|MON\\9")).toBe(true);
  });
  it("distinct identities never cross-match", () => {
    expect(edidKeyMatch("B|MON\\1", "A")).toBe(false);
    expect(edidKeyMatch("AA", "A")).toBe(false);
  });
});

describe("valueFor (map lookups under collided monitors)", () => {
  it("exact key wins", () => {
    expect(valueFor({ "A|MON\\1": "a" }, "A|MON\\1")).toBe("a");
  });
  it("legacy plain-key record shows on a qualified live monitor", () => {
    expect(valueFor({ "A": "legacy" }, "A|MON\\2")).toBe("legacy");
  });
  it("stale qualified record shows after the twin left (key reverted to base)", () => {
    expect(valueFor({ "A|MON\\1": "old" }, "A")).toBe("old");
  });
  it("qualified records beat no base fallback — but sibling record acts as fallback", () => {
    // Same insertion order as enumerate: a surviving sibling twin keeps
    // the identity visible (first-match legacy semantics).
    expect(valueFor({ "A|MON\\1": "twin", "B": "other" }, "A")).toBe("twin");
  });
  it("distinct identities stay distinct", () => {
    expect(valueFor({ "B|MON\\1": "b" }, "A|MON\\1")).toBeUndefined();
  });
});
