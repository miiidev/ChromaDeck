import { describe, it, expect } from "vitest";
import { connectedEdidSignature } from "./connectedSignature";

describe("connectedEdidSignature", () => {
  it("is identical for equal-but-reordered connected monitor lists", () => {
    const a = [
      { edid_id: "EDID-B", connected: true },
      { edid_id: "EDID-A", connected: true },
    ];
    const b = [
      { edid_id: "EDID-A", connected: true },
      { edid_id: "EDID-B", connected: true },
    ];
    expect(connectedEdidSignature(a)).toBe(connectedEdidSignature(b));
  });

  it("ignores offline monitors", () => {
    const monitors = [
      { edid_id: "EDID-A", connected: true },
      { edid_id: "EDID-OFF", connected: false },
    ];
    expect(connectedEdidSignature(monitors)).toBe("EDID-A");
  });

  it("distinguishes different connected sets", () => {
    const a = [{ edid_id: "EDID-A", connected: true }];
    const b = [{ edid_id: "EDID-B", connected: true }];
    expect(connectedEdidSignature(a)).not.toBe(connectedEdidSignature(b));
  });

  it("returns an empty key when nothing is connected", () => {
    const monitors = [
      { edid_id: "EDID-A", connected: false },
      { edid_id: "EDID-B", connected: false },
    ];
    expect(connectedEdidSignature(monitors)).toBe("");
  });
});
