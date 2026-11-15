import { describe, it, expect } from "vitest";
import { deriveSilhouetteState, SILHOUETTE } from "./monitorSilhouette";

const preset = { name: "Warm Cinema", color_tag: "#fb923c" };
const untagged = { name: "Cool Room", color_tag: "" };

describe("deriveSilhouetteState", () => {
  it("offline wins over everything, even a pin", () => {
    const s = deriveSilhouetteState({
      connected: false,
      isDropHover: false,
      samePresetApplied: false,
      draggedTag: null,
      slotPreset: preset,
      isPinned: true,
      isAuto: false,
      wasApplied: false,
      statusPhase: undefined,
    });
    expect(s.phase).toBe("offline");
    expect(s.muted).toBe(true);
    expect(s.accent).toBeNull();
    expect(s.tintOpacity).toBeNull();
    expect(s.statusLabel).toBe("OFFLINE");
  });

  it("drag over takes the dragged preset's tag at 10% tint", () => {
    const s = deriveSilhouetteState({
      connected: true,
      isDropHover: true,
      samePresetApplied: false,
      draggedTag: "#22d3ee",
      slotPreset: null,
      isPinned: false,
      isAuto: false,
      wasApplied: false,
      statusPhase: undefined,
    });
    expect(s.phase).toBe("dragover");
    expect(s.accent).toBe("#22d3ee");
    expect(s.tintOpacity).toBe(SILHOUETTE.tintDrag);
    expect(s.statusLabel).toBe("DEFAULT");
    expect(s.primaryLabel).toBe("Release to apply");
  });

  it("drag over the same preset that is already applied is not a drop target", () => {
    const s = deriveSilhouetteState({
      connected: true,
      isDropHover: true,
      samePresetApplied: true,
      draggedTag: "#22d3ee",
      slotPreset: preset,
      isPinned: false,
      isAuto: false,
      wasApplied: true,
      statusPhase: undefined,
    });
    expect(s.phase).toBe("applied");
    expect(s.accent).toBe("#fb923c");
  });

  it("applying shows the in-flight preset name", () => {
    const s = deriveSilhouetteState({
      connected: true,
      isDropHover: true,
      samePresetApplied: false,
      draggedTag: "#22d3ee",
      slotPreset: null,
      isPinned: false,
      isAuto: false,
      wasApplied: false,
      statusPhase: "applying",
      statusPresetName: "Warm Cinema",
    });
    expect(s.phase).toBe("applying");
    expect(s.accent).toBe("#22d3ee");
    expect(s.primaryLabel).toBe("Applying Warm Cinema…");
    expect(s.statusLabel).toBe("APPLYING");
  });

  it("error retains the previous preset's tint and carries the error note", () => {
    const s = deriveSilhouetteState({
      connected: true,
      isDropHover: false,
      samePresetApplied: false,
      draggedTag: null,
      slotPreset: preset,
      isPinned: false,
      isAuto: false,
      wasApplied: false,
      statusPhase: "error",
      statusError: "nvapi not loaded",
    });
    expect(s.phase).toBe("error");
    expect(s.accent).toBe("#fb923c");
    expect(s.tintOpacity).toBe(SILHOUETTE.tintApplied);
    expect(s.statusLabel).toBe("APPLY FAILED");
    expect(s.secondaryLabel).toBe("nvapi not loaded");
  });

  it("pinned shows the preset category when split succeeds", () => {
    const s = deriveSilhouetteState({
      connected: true,
      isDropHover: false,
      samePresetApplied: false,
      draggedTag: null,
      slotPreset: { name: "Movie - Warm Cinema", color_tag: "#fb923c" },
      isPinned: true,
      isAuto: false,
      wasApplied: false,
      statusPhase: undefined,
    });
    expect(s.phase).toBe("pinned");
    expect(s.accent).toBe("#fb923c");
    expect(s.tintOpacity).toBe(SILHOUETTE.tintApplied);
    expect(s.statusLabel).toBe("MOVIE");
  });

  it("auto-apply keeps its own tinted treatment", () => {
    const s = deriveSilhouetteState({
      connected: true,
      isDropHover: false,
      samePresetApplied: false,
      draggedTag: null,
      slotPreset: untagged,
      isPinned: false,
      isAuto: true,
      wasApplied: false,
      statusPhase: undefined,
    });
    expect(s.phase).toBe("auto");
    expect(s.accent).toBeNull();
    expect(s.tintOpacity).toBe(SILHOUETTE.tintApplied);
    expect(s.muted).toBe(false);
  });

  it("empty connected monitor prompts for a drop", () => {
    const s = deriveSilhouetteState({
      connected: true,
      isDropHover: false,
      samePresetApplied: false,
      draggedTag: null,
      slotPreset: null,
      isPinned: false,
      isAuto: false,
      wasApplied: false,
      statusPhase: undefined,
    });
    expect(s.phase).toBe("empty");
    expect(s.accent).toBeNull();
    expect(s.tintOpacity).toBeNull();
    expect(s.statusLabel).toBe("DEFAULT");
    expect(s.primaryLabel).toBe("Drop preset here");
  });

  it("an invalid colour tag yields no accent", () => {
    const s = deriveSilhouetteState({
      connected: true,
      isDropHover: false,
      samePresetApplied: false,
      draggedTag: "not-a-colour",
      slotPreset: { name: "Warm Cinema", color_tag: "javascript:alert(1)" },
      isPinned: false,
      isAuto: false,
      wasApplied: true,
      statusPhase: undefined,
    });
    expect(s.phase).toBe("applied");
    expect(s.accent).toBeNull();
  });

  it("geometry constants match the approved Figma masters", () => {
    expect(SILHOUETTE.screenH).toBe(116);
    expect(SILHOUETTE.screenHCompact).toBe(102);
    expect(SILHOUETTE.neckW).toBe(21);
    expect(SILHOUETTE.neckH).toBe(13);
    expect(SILHOUETTE.footW).toBe(67);
    expect(SILHOUETTE.footH).toBe(3);
    expect(SILHOUETTE.tintDrag).toBe(0.1);
    expect(SILHOUETTE.tintApplied).toBe(0.08);
  });
});
