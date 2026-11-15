import { splitCategory } from "./presetCard";

/**
 * Visual state machine for the monitor-silhouette card (Figma masters
 * 97:201–97:268, file vfF5g0hTALNbScBM4Nz1Y1). Pure so the card component
 * stays a renderer and every state treatment is unit-testable.
 */

export type SilhouettePhase =
  | "dragover"
  | "applying"
  | "error"
  | "pinned"
  | "auto"
  | "applied"
  | "empty"
  | "offline";

export interface SilhouetteState {
  phase: SilhouettePhase;
  /** Hex accent colour for the bezel border + screen tint, or null. */
  accent: string | null;
  /** Screen tint opacity (drag-over 0.10, applied/pinned/auto 0.08), or null. */
  tintOpacity: number | null;
  /** Uppercase status label, top-right of the screen. */
  statusLabel: string;
  /** Main screen line (preset title / prompt). */
  primaryLabel: string;
  /** Secondary line under primaryLabel (error note / category), or null. */
  secondaryLabel: string | null;
  /** True when the offline treatment (muted screen + stand) applies. */
  muted: boolean;
}

/** Approved silhouette geometry + tint values (Figma masters). */
export const SILHOUETTE = {
  screenH: 116,
  screenHCompact: 102,
  neckW: 21,
  neckH: 13,
  footW: 67,
  footH: 3,
  tintDrag: 0.1,
  tintApplied: 0.08,
} as const;

const HEX = /^#[0-9a-fA-F]{6}$/;

function tagColor(colorTag: string | undefined): string | null {
  return colorTag && HEX.test(colorTag) ? colorTag : null;
}

/** Category for the status label; falls back to the preset title, then a
 *  neutral word so the label never becomes colour-only. */
function assignedStatusLabel(name: string): string {
  const { category, title } = splitCategory(name);
  return (category || title || "ASSIGNED").toUpperCase();
}

export function deriveSilhouetteState(input: {
  connected: boolean;
  isDropHover: boolean;
  samePresetApplied: boolean;
  draggedTag: string | null;
  slotPreset: { name: string; color_tag?: string } | null;
  isPinned: boolean;
  isAuto: boolean;
  wasApplied: boolean;
  statusPhase: "applying" | "applied" | "error" | undefined;
  statusPresetName?: string;
  statusError?: string;
}): SilhouetteState {
  const { connected, slotPreset } = input;

  // Offline outranks everything: a pinned preset on a dark panel must never
  // read as a successful apply.
  if (!connected) {
    return {
      phase: "offline",
      accent: null,
      tintOpacity: null,
      statusLabel: "OFFLINE",
      primaryLabel: "Not connected",
      secondaryLabel: null,
      muted: true,
    };
  }

  const slotTag = tagColor(slotPreset?.color_tag);
  const draggedTag = tagColor(input.draggedTag ?? undefined);

  // Apply in flight: the wash uses the dragged preset's colour while it is
  // over the slot, otherwise the slot's own preset colour.
  if (input.statusPhase === "applying") {
    return {
      phase: "applying",
      accent: draggedTag ?? slotTag,
      tintOpacity: SILHOUETTE.tintDrag,
      statusLabel: "APPLYING",
      primaryLabel: `Applying ${input.statusPresetName ?? slotPreset?.name ?? "preset"}…`,
      secondaryLabel: null,
      muted: false,
    };
  }

  // A valid drop target adopts the dragged preset's identity.
  if (input.isDropHover && !input.samePresetApplied) {
    return {
      phase: "dragover",
      accent: draggedTag ?? slotTag,
      tintOpacity: SILHOUETTE.tintDrag,
      statusLabel: "DEFAULT",
      primaryLabel: slotPreset ? slotPreset.name : "Release to apply",
      secondaryLabel: null,
      muted: false,
    };
  }

  // Failure never fakes success: the previous preset row and its tint stay
  // visible, and the error note rides beneath the title.
  if (input.statusPhase === "error") {
    return {
      phase: "error",
      accent: slotTag,
      tintOpacity: slotPreset ? SILHOUETTE.tintApplied : null,
      statusLabel: "APPLY FAILED",
      primaryLabel: slotPreset?.name ?? "Apply failed",
      secondaryLabel: input.statusError ?? null,
      muted: false,
    };
  }

  if (input.isPinned && slotPreset) {
    return {
      phase: "pinned",
      accent: slotTag,
      tintOpacity: SILHOUETTE.tintApplied,
      statusLabel: assignedStatusLabel(slotPreset.name),
      primaryLabel: slotPreset.name,
      secondaryLabel: null,
      muted: false,
    };
  }

  if (input.isAuto && slotPreset) {
    return {
      phase: "auto",
      accent: slotTag,
      tintOpacity: SILHOUETTE.tintApplied,
      statusLabel: assignedStatusLabel(slotPreset.name),
      primaryLabel: slotPreset.name,
      secondaryLabel: null,
      muted: false,
    };
  }

  if (input.wasApplied && slotPreset) {
    return {
      phase: "applied",
      accent: slotTag,
      tintOpacity: SILHOUETTE.tintApplied,
      statusLabel: assignedStatusLabel(slotPreset.name),
      primaryLabel: slotPreset.name,
      secondaryLabel: null,
      muted: false,
    };
  }

  return {
    phase: "empty",
    accent: null,
    tintOpacity: null,
    statusLabel: "DEFAULT",
    primaryLabel: "Drop preset here",
    secondaryLabel: null,
    muted: false,
  };
}
