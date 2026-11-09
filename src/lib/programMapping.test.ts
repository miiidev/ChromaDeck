import { describe, expect, it } from "vitest";
import { addProgram, duplicateOwner, normalizeProgramName } from "./programMapping";
import type { Preset } from "./types";

function preset(id: string, name: string, programs: string[]): Preset {
  return {
    id,
    name,
    icc_hash: "",
    icc_filename: "",
    brightness: 50,
    contrast: 50,
    rgb_gains: [1, 1, 1],
    gamma: 1,
    vibrance: 50,
    saturation: 50,
    hue_deg: 0,
    attached_programs: programs,
  };
}

describe("normalizeProgramName", () => {
  it("takes the base name of a full path and lowercases it", () => {
    expect(
      normalizeProgramName("C:\\Program Files\\Google\\Chrome\\chrome.EXE"),
    ).toBe("chrome.exe");
  });

  it("trims surrounding whitespace", () => {
    expect(normalizeProgramName("  vlc.exe ")).toBe("vlc.exe");
  });

  it("rejects empty input and names with inner whitespace", () => {
    expect(normalizeProgramName("")).toBeNull();
    expect(normalizeProgramName("   ")).toBeNull();
    expect(normalizeProgramName("game launcher.exe")).toBeNull();
  });
});

describe("duplicateOwner", () => {
  const presets = [preset("p1", "Work", ["chrome.exe"]), preset("p2", "Play", [])];

  it("names the preset that already owns the program", () => {
    expect(duplicateOwner(presets, "CHROME.EXE")).toBe("Work");
  });

  it("ignores the preset being edited", () => {
    expect(duplicateOwner(presets, "chrome.exe", "p1")).toBeNull();
  });

  it("returns null for an unowned program", () => {
    expect(duplicateOwner(presets, "vlc.exe")).toBeNull();
  });
});

describe("addProgram", () => {
  const presets = [preset("p1", "Work", ["chrome.exe"]), preset("p2", "Play", [])];

  it("appends a normalized program", () => {
    expect(addProgram([], "C:\\Apps\\VLC.exe", presets, "p2")).toEqual({
      list: ["vlc.exe"],
    });
  });

  it("ignores a duplicate already in the draft list", () => {
    expect(addProgram(["vlc.exe"], "VLC.EXE", presets, "p2")).toEqual({
      list: ["vlc.exe"],
    });
  });

  it("rejects a program owned by another preset and names it", () => {
    expect(addProgram([], "chrome.exe", presets, "p2")).toEqual({
      list: [],
      error: 'chrome.exe is already mapped to "Work"',
    });
  });

  it("rejects malformed input with a hint", () => {
    expect(addProgram(["vlc.exe"], " ", presets, "p2").error).toContain(
      "executable name",
    );
  });
});
