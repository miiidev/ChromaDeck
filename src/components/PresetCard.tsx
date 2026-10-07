import { useState } from "react";
import type { Monitor, Preset } from "../lib/types";
import { deletePreset, createPreset, unpinMonitor } from "../lib/tauri";
import {
  splitCategory,
  formatStat,
  isNeutral,
  STAT_KEYS,
  STAT_LABELS,
} from "../lib/presetCard";
import StatTile from "./StatTile";
import { Checkbox, CheckboxIndicator } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogHeader, DialogFooter, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Copy, Pencil, PinOff, Trash2, Pin } from "lucide-react";
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

export default function PresetCard({
  preset, monitors, pins, onEdit, onRefreshParent, onPinChange,
  onApply, appliedMap, staggerEnter, staggerMs, onDuplicated,
  highlightEnter, selectable, selected, onToggleSelect,
  trembleDelayMs, exploding, explodeDelayMs, onDeleted,
}: Props) {
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
      if (onDuplicated) onDuplicated(created);
      else onRefreshParent();
    } catch {
      // silent
    }
  };

  const handleDelete = async () => {
    setDeleting(true);
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
  const isActive =
    pinnedEdidList.some(isConnected) ||
    Object.entries(appliedMap).some(
      ([edid, pid]) => pid === preset.id && isConnected(edid),
    );
  const pinnedMonitorNames = pinnedEdidList.map((edid) => {
    const m = monitors.find((m) => m.edid_id === edid);
    return m?.alias || m?.model || edid.slice(0, 12);
  });

  // Color tag tint + glow (same as before)
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

  // Category split on first " - "
  const { category, title } = splitCategory(preset.name);

  // Title font-size based on length
  const titleLen = title.length;
  const titleSize = titleLen > 13 ? "26px" : titleLen > 9 ? "32px" : "36px";

  // Stat values
  const statValue = (key: string): number => {
    switch (key) {
      case "gamma": return preset.gamma;
      case "brightness": return preset.brightness;
      case "contrast": return preset.contrast;
      case "vibrance": return preset.vibrance;
      default: return 0;
    }
  };

  return (
    <div
      className={cn(staggerEnter ? "enter-stagger" : "")}
      style={staggerEnter && staggerMs !== undefined
        ? ({ "--stagger-ms": `${staggerMs}ms` } as Record<string, string>)
        : undefined
      }
    >
      {/* ── Card shell ───────────────────────────────────────────── */}
      <div
        data-slot="card"
        className={cn(
          "card-ring",
          "flex flex-col gap-3.5 w-full rounded-xl border-2 border-[#272b38] bg-[#0e1118] p-[18px] outline-none",
          "focus-visible:ring-3 focus-visible:ring-ring/50",
          isActive ? "ring-1 ring-primary/60" : "",
          highlightEnter ? "pop-in" : "",
          selectable && !detonating ? "card-tremble" : "",
          detonating ? "card-explode" : "",
        )}
        style={{
          ...padStyle,
          viewTransitionName: `preset-card-${preset.id}`,
          ...((detonating ? detonateDelayMs : trembleDelayMs) !== undefined
            ? ({ animationDelay: `${detonating ? detonateDelayMs : trembleDelayMs}ms` } as Record<string, string>)
            : {}),
          // Pinned: accent border + hard offset shadow
          ...(isPinned
            ? ({
                borderColor: "#2f8bff",
                boxShadow: "6px 6px 0 #2f8bff",
              } as Record<string, string>)
            : {}),
        }}
        role="button"
        tabIndex={0}
        onClick={() => {
          if (selectable) onToggleSelect?.();
          else onApply(preset);
        }}
        onKeyDown={handleKeyDown}
        aria-label={selectable ? `Select preset ${preset.name}` : `Apply preset ${preset.name}`}
        aria-pressed={selectable ? (selected ? "true" : "false") : undefined}
      >
        {/* ═══ Row 1: Header ═══ */}
        <div className="flex items-center justify-between h-8">
          {/* Left cluster: selection checkbox is ADDED in select mode;
              card content never changes with mode. */}
          <div className="flex items-center gap-1.5">
            {selectable && (
              // Decorative mirror of pad state; the pad itself toggles.
              <span className="pointer-events-none" aria-hidden="true">
                <Checkbox checked={selected ?? false} onCheckedChange={() => {}} tabIndex={-1}>
                  <CheckboxIndicator />
                </Checkbox>
              </span>
            )}
            <>
              {/* Filled pin icon when pinned */}
              {isPinned && (
                  <span
                    className="inline-flex items-center gap-1"
                    title={`Pinned to ${pinnedMonitorNames.join(", ")}`}
                    aria-label={`Preset pinned to ${pinnedMonitorNames.join(", ")}`}
                  >
                    <Pin className="size-[14px] text-[#2f8bff]" strokeWidth={2} />
                    <span className="sr-only">Pinned</span>
                  </span>
                )}

                {/* Category label */}
                {category && (
                  <span className="text-[11px] font-mono font-bold tracking-[0.14em] uppercase text-[#8b92a6]">
                    {category}
                  </span>
                )}

                {/* ICC tag */}
                {preset.icc_hash && (
                  <span className="pop-in rounded-[6px] border border-[#3a4054] px-[6px] py-px text-[10px] font-mono tracking-[0.1em] text-[#8b92a6]">
                    ICC
                  </span>
                )}

                {/* IN USE marker: solid accent badge, dark text for contrast */}
                {isActive && (
                  <span
                    className="pop-in rounded-[6px] bg-[#2f8bff] px-[6px] py-px text-[10px] font-mono font-bold tracking-[0.1em] text-[#06202e]"
                    title="Currently applied to a connected monitor"
                  >
                    IN USE
                  </span>
                )}
              </>
          </div>

          {/* Right: action icons — hidden in selection mode */}
          {!selectable && (
            <div className="flex items-center gap-1.5">
              {isPinned && (
                <span className="pop-in">
                  <button
                    type="button"
                    className="inline-flex items-center justify-center size-8 rounded-lg bg-transparent text-[#6b7285] hover:text-[#2f8bff] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#2f8bff]/60" data-slot="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      void (async () => {
                        try {
                          for (const edid of pinnedEdidList) {
                            await unpinMonitor(edid);
                          }
                          onPinChange();
                        } catch { /* silent */ }
                      })();
                    }}
                    title="Unpin from all monitors"
                    aria-label={`Unpin preset ${preset.name} from all monitors`}
                  >
                    <PinOff className="size-4 stroke-2" />
                  </button>
                </span>
              )}

              <button
                type="button"
                className="inline-flex items-center justify-center size-8 rounded-lg bg-transparent text-[#6b7285] hover:text-[#2f8bff] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#2f8bff]/60" data-slot="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onEdit(preset);
                }}
                onKeyDown={(e) => e.stopPropagation()}
                title="Edit"
                aria-label={`Edit preset ${preset.name}`}
              >
                <Pencil className="size-4 stroke-2" />
              </button>

              <button
                type="button"
                className="inline-flex items-center justify-center size-8 rounded-lg bg-transparent text-[#6b7285] hover:text-[#2f8bff] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#2f8bff]/60" data-slot="button"
                onClick={(e) => {
                  e.stopPropagation();
                  void handleDuplicate();
                }}
                onKeyDown={(e) => e.stopPropagation()}
                title="Duplicate preset"
                aria-label={`Duplicate preset ${preset.name}`}
              >
                <Copy className="size-4 stroke-2" />
              </button>

              <button
                type="button"
                className="inline-flex items-center justify-center size-8 rounded-lg bg-transparent text-[#6b7285] hover:text-[#ff2e7e] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#2f8bff]/60" data-slot="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setShowDeleteModal(true);
                }}
                onKeyDown={(e) => e.stopPropagation()}
                title="Delete preset"
                aria-label={`Delete preset ${preset.name}`}
              >
                <Trash2 className="size-4 stroke-2" />
              </button>
            </div>
          )}
        </div>

        {/* ═══ Row 2: Title as apply button ═══ */}
        <button
          type="button"
          className="flex items-center text-left w-full bg-transparent border-0 cursor-pointer rounded transition-opacity hover:opacity-85 focus-visible:outline-2 focus-visible:outline-[#2f8bff] focus-visible:outline-offset-2"
          onClick={(e) => {
            // Pad already applies/toggles; don't fire twice.
            e.stopPropagation();
            if (selectable) onToggleSelect?.();
            else onApply(preset);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") e.stopPropagation();
          }}
          aria-label={selectable
            ? `Select preset ${preset.name}`
            : `Apply preset ${preset.name}`
          }
          aria-pressed={selectable ? (selected ? "true" : "false") : undefined}
        >
          <span
            className="truncate text-[#f2f5fa] font-display font-extrabold"
            style={{
              fontSize: titleSize,
              lineHeight: "1",
              letterSpacing: "-0.02em",
            } as Record<string, string>}
            title={preset.name}
          >
            {title}
          </span>
        </button>

        {/* ═══ Row 3: 2×2 Stat grid (identical in every mode) ═══ */}
        <div className="mt-auto grid grid-cols-2 gap-2">
            {STAT_KEYS.map((key) => {
              const value = statValue(key);
              return (
                <StatTile
                  key={key}
                  label={STAT_LABELS[key]}
                  value={formatStat(key, value)}
                  neutral={isNeutral(key, value)}
                />
              );
            })}
        </div>

        {/* ── Pin target hint (below grid, every mode) ── */}
        {isPinned && pinnedMonitorNames.length > 0 && (
          <span className="text-xs text-muted-foreground leading-tight">
            → {pinnedMonitorNames.join(", ")}
          </span>
        )}
      </div>

      {/* ── Delete confirmation dialog ─────────────────────────── */}
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
              <button
                type="button"
                data-slot="button"
                className="inline-flex items-center justify-center rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#2f8bff]/60"
                onClick={() => setShowDeleteModal(false)}
                disabled={deleting}
              >
                Cancel
              </button>
              <button
                type="button"
                data-slot="button"
                className="inline-flex items-center justify-center rounded-lg bg-destructive/20 px-3 py-1.5 text-xs font-medium text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#2f8bff]/60"
                onClick={() => void handleDelete()}
                disabled={deleting}
              >
                {deleting ? "Deleting…" : "Delete"}
              </button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}