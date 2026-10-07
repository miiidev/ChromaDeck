import { useState } from "react";
import type { Monitor, Preset } from "../lib/types";
import { deletePreset, createPreset, unpinMonitor } from "../lib/tauri";
import { Badge } from "@/components/ui/badge";
import { deviatingChips } from "../lib/presetChips";
import { Button } from "@/components/ui/button";
import { Checkbox, CheckboxIndicator } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogHeader, DialogFooter, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Copy, Pencil, PinOff, Trash2 } from "lucide-react";
import { cn } from "cn";

interface Props {
  preset: Preset;
  monitors: Monitor[];
  pins: Record<string, string>;
  onEdit: (preset: Preset) => void;
  onRefreshParent: () => void;
  onPinChange: () => void;
  onApply: (preset: Preset) => void;
  appliedMap: Record<string, string>;
  staggerEnter?: boolean;
  staggerMs?: number;
  onDuplicated?: (preset: Preset) => void;
  highlightEnter?: boolean;
  selectable?: boolean;
  selected?: boolean;
  onToggleSelect?: () => void;
  trembleDelayMs?: number;
  exploding?: boolean;
  explodeDelayMs?: number;
  onDeleted?: (id: string) => void;
}

export default function PresetCard({ preset, monitors, pins, onEdit, onRefreshParent, onPinChange, onApply, appliedMap, staggerEnter, staggerMs, onDuplicated, highlightEnter, selectable, selected, onToggleSelect, trembleDelayMs, exploding, explodeDelayMs, onDeleted }: Props) {
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [singleExploding, setSingleExploding] = useState(false);

  const handleDuplicate = async () => {
    try {
      const created = await createPreset({
        name: `${preset.name} (copy)`,
        brightness: preset.brightness,
        contrast: preset.contrast,
        rgb_gains: preset.rgb_gains,
        gamma: preset.gamma,
        vibrance: preset.vibrance,
        hue_deg: preset.hue_deg,
        color_tag: preset.color_tag,
      });
      // Optimistic insert (no library reload): the parent appends the new
      // card with its own entrance. Falls back to full refresh if unwired.
      if (onDuplicated) onDuplicated(created);
      else onRefreshParent();
    } catch {
      // silent
    }
  };

  const handleDelete = async () => {
    setDeleting(true);
    // Solo detonation: same explosion language as batch delete, then
    // optimistic removal (no library reload). Falls back to full refresh.
    setSingleExploding(true);
    try {
      await new Promise<void>((resolve) => {
        window.setTimeout(resolve, 450);
      });
      await deletePreset(preset.id);
      setShowDeleteModal(false);
      if (onDeleted) onDeleted(preset.id);
      else onRefreshParent();
    } catch {
      setShowDeleteModal(false);
    } finally {
      setDeleting(false);
      setSingleExploding(false);
    }
  };

  // Either detonation path (batch via parent, solo locally) triggers it.
  const detonating = exploding || singleExploding;
  const detonateDelayMs = explodeDelayMs ?? 0;

  const handleKeyDown = (e: { key: string; preventDefault: () => void }) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (selectable) onToggleSelect?.();
      else onApply(preset);
    }
  };

  // Find ALL monitors that have this preset pinned
  const pinnedEdidList = Object.entries(pins)
    .filter(([, pid]) => pid === preset.id)
    .map(([edid]) => edid);
  const isPinned = pinnedEdidList.length > 0;
  const isConnected = (edid: string) =>
    monitors.find((m) => m.edid_id === edid)?.connected ?? false;
  // In use = pinned to a connected monitor, or applied this session
  const isActive =
    pinnedEdidList.some(isConnected) ||
    Object.entries(appliedMap).some(
      ([edid, pid]) => pid === preset.id && isConnected(edid),
    );
  const pinnedMonitorNames = pinnedEdidList.map((edid) => {
    const m = monitors.find((m) => m.edid_id === edid);
    return m?.alias || m?.model || edid.slice(0, 12);
  });

  // Custom tag color tints the whole pad. Format-guarded: a hand-edited
  // presets.json could hold a non-hex string, which must never reach CSS.
  // IN USE keeps its accent ring + badge on top of the custom tint.
  // Glow uses filter: drop-shadow (not box-shadow) so it never overrides
  // the Tailwind ring utilities or the focus-visible ring.
  const tag = /^#[0-9a-fA-F]{6}$/.test(preset.color_tag ?? "")
    ? (preset.color_tag as string)
    : null;
  const padBackground = tag
    ? `color-mix(in srgb, var(--color-card), ${tag} 14%)`
    : isActive
      ? "color-mix(in srgb, var(--color-card), var(--color-primary) 12%)"
      : undefined;
  const padStyle: Record<string, string> | undefined =
    padBackground || tag
      ? ({
          ...(padBackground ? { backgroundColor: padBackground } : {}),
          ...(tag
            ? {
                filter: `drop-shadow(0 0 8px color-mix(in srgb, ${tag} 55%, transparent))`,
              }
            : {}),
        } as Record<string, string>)
      : undefined;

  return (
    <div
      className={cn(
        staggerEnter ? "enter-stagger" : "",
      )}
      style={staggerEnter && staggerMs !== undefined
        ? ({ "--stagger-ms": `${staggerMs}ms` } as Record<string, string>)
        : undefined
      }
    >
      {/* ── Tap pad: whole surface applies the preset ─────────────── */}
      <div
        data-slot="card"
        className={cn(
          "card-ring",
          "flex flex-col rounded-lg overflow-hidden bg-card ring-1 ring-foreground/10 text-sm text-card-foreground outline-none focus-visible:ring-3 focus-visible:ring-ring/50 min-h-48",
          isActive ? "ring-1 ring-primary/60" : "",
          highlightEnter ? "pop-in" : "",
          selectable && !detonating ? "card-tremble" : "",
          detonating ? "card-explode" : "",
        )}
        style={{
          ...padStyle,
          // Stable per-card transition name: lets View Transitions match
          // this card across snapshots so survivors glide when siblings
          // are deleted. Inert unless a transition runs.
          viewTransitionName: `preset-card-${preset.id}`,
          // Negative delay starts each card mid-wobble so the grid
          // trembles out of sync. Explosion delay wins while detonating.
          // Inert unless a matching animation class runs.
          ...((detonating ? detonateDelayMs : trembleDelayMs) !== undefined
            ? ({ animationDelay: `${detonating ? detonateDelayMs : trembleDelayMs}ms` } as Record<string, string>)
            : {}),
        }}
        role="button"
        tabIndex={0}
        onClick={() => { if (selectable) onToggleSelect?.(); else onApply(preset); }}
        onKeyDown={handleKeyDown}
        aria-label={selectable ? `Select preset ${preset.name}` : `Apply preset ${preset.name}`}
        aria-pressed={selectable && selected ? "true" : "false"}
      >
        {/* Deviating-parameter chips — only what differs from neutral */}
        <div className="flex flex-wrap items-center gap-1 px-1.5 pt-1.5">
          {(() => {
            const chips = deviatingChips({
              brightness: preset.brightness,
              contrast: preset.contrast,
              gamma: preset.gamma,
              rgb_gains: preset.rgb_gains,
              vibrance: preset.vibrance,
              hue_deg: preset.hue_deg,
            });
            if (chips.length === 0) {
              return (
                <span className="rounded-md border border-border px-1 py-px text-[10px] mono text-muted-foreground">
                  Neutral
                </span>
              );
            }
            return chips.map((chip) => (
              <span
                key={chip.key}
                title={chip.title}
                className="rounded-md border border-border bg-muted px-1 py-px text-[10px] mono text-foreground"
              >
                {chip.label}
              </span>
            ));
          })()}
        </div>

        {/* Name + inline status markers — one line */}
        <div className="flex items-center gap-1.5 shrink-0 px-1.5 py-0.5">
          {selectable && (
            <span className="pointer-events-none">
              <Checkbox checked={selected ?? false} onCheckedChange={() => {}}>
                <CheckboxIndicator />
              </Checkbox>
            </span>
          )}
          <span className="truncate text-sm font-medium leading-tight" title={preset.name}>
            {preset.name}
          </span>
          <div className="flex items-center gap-1 shrink-0">
            {isActive && (
              <Badge className="pop-in" title="Currently applied to a connected monitor">
                IN USE
              </Badge>
            )}
            {isPinned && (
              <span
                className="status-dot-pinned"
                title={`Pinned to ${pinnedMonitorNames.join(", ")}`}
              />
            )}
            {preset.icc_hash && (
              <Badge variant="outline" className="pop-in" title={preset.icc_filename}>
                ICC
              </Badge>
)}
          </div>
        </div>

        {/* Pin target hint */}
        {pinnedMonitorNames.length > 0 && (
          <span className="px-1.5 text-xs text-muted-foreground leading-tight" title={`Pinned to ${pinnedMonitorNames.join(", ")}`}>
            → {pinnedMonitorNames.join(", ")}
          </span>
        )}

        {/* ── Action cluster — compact icon buttons, in-flow row ─── */}
        {/* Row is click-through (empty space still applies the preset);
            each button re-enables pointer events and stops propagation so
            its press never bubbles up into an apply.
            Hidden entirely in selection mode. */}
        {!selectable && (
        <div
          className="flex items-center justify-end gap-1.5 px-1.5 pb-1.5 pt-1 mt-auto pointer-events-none"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          {isPinned && (
            <span className="pop-in">
              <Button
                variant="outline"
                size="icon-xs"
                className="pointer-events-auto"
                onClick={async (e) => {
                  e.stopPropagation();
                  try {
                    for (const edid of pinnedEdidList) {
                      await unpinMonitor(edid);
                    }
                    onPinChange();
                  } catch {
                    // silent
                  }
                }}
                title="Unpin from all monitors"
                aria-label={`Unpin preset ${preset.name} from all monitors`}
              >
                <PinOff />
              </Button>
            </span>
          )}

          {/* EDIT = secondary */}
          <Button
            variant="secondary"
            size="icon-xs"
            className="pointer-events-auto"
            onClick={(e) => {
              e.stopPropagation();
              onEdit(preset);
            }}
            title="Edit"
            aria-label={`Edit preset ${preset.name}`}
          >
            <Pencil />
          </Button>

          {/* DUP = outline */}
          <Button
            variant="outline"
            size="icon-xs"
            className="pointer-events-auto"
            onClick={(e) => {
              e.stopPropagation();
              void handleDuplicate();
            }}
            title="Duplicate preset"
            aria-label={`Duplicate preset ${preset.name}`}
          >
            <Copy />
          </Button>

          {/* DEL = destructive */}
          <Button
            variant="destructive"
            size="icon-xs"
            className="pointer-events-auto"
            onClick={(e) => {
              e.stopPropagation();
              setShowDeleteModal(true);
            }}
            title="Delete preset"
            aria-label={`Delete preset ${preset.name}`}
          >
            <Trash2 />
          </Button>
</div>
        )}
      </div>

      {/* Delete confirmation dialog */}
      {showDeleteModal && (
        <Dialog
          open
          onOpenChange={(open) => { if (!open && !deleting) setShowDeleteModal(false); }}
        >
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Delete preset?</DialogTitle>
              <DialogDescription>
                Permanently delete <span className="font-medium text-foreground">"{preset.name}"</span>?
                This cannot be undone.
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button variant="outline" size="sm" onClick={() => setShowDeleteModal(false)} disabled={deleting}>
                Cancel
              </Button>
              <Button variant="destructive" size="sm" onClick={() => void handleDelete()} disabled={deleting}>
                {deleting ? "Deleting…" : "Delete"}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}