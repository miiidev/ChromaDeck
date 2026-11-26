import { describe, expect, it } from "vitest";
import { findShortcutConflict, shortcutFromKeyEvent } from "./shortcut";

describe("shortcutFromKeyEvent", () => {
  it("formats canonical modifier order and uses physical Digit/Key codes under Shift", () => {
    expect(
      shortcutFromKeyEvent({
        key: "!",
        code: "Digit1",
        ctrlKey: true,
        altKey: false,
        shiftKey: true,
        metaKey: false,
      }),
    ).toBe("Ctrl+Shift+1");

    expect(
      shortcutFromKeyEvent({
        key: "f2",
        code: "F2",
        ctrlKey: true,
        altKey: true,
        shiftKey: false,
        metaKey: false,
      }),
    ).toBe("Ctrl+Alt+F2");
  });

  it("rejects bare keys and modifier-only presses", () => {
    expect(
      shortcutFromKeyEvent({
        key: "F1",
        code: "F1",
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        metaKey: false,
      }),
    ).toBeNull();

    expect(
      shortcutFromKeyEvent({
        key: "Control",
        code: "ControlLeft",
        ctrlKey: true,
        altKey: false,
        shiftKey: false,
        metaKey: false,
      }),
    ).toBeNull();
  });
});

describe("findShortcutConflict", () => {
  const presets = [
    { id: "p1", name: "Cinema", shortcut: "Ctrl+Alt+F1" },
    { id: "p2", name: "Gaming", shortcut: null },
  ];

  it("detects conflict on another preset but allows editing the same preset", () => {
    expect(findShortcutConflict("Ctrl+Alt+F1", presets, "p2")).toContain("Cinema");
    expect(findShortcutConflict("Ctrl+Alt+F1", presets, "p1")).toBeNull();
    expect(findShortcutConflict("Ctrl+Alt+F2", presets, "p2")).toBeNull();
  });
});
