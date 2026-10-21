import { useState, useRef, useEffect } from "react";
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
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogFooter, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Copy, Pencil, PinOff, Trash2 } from "lucide-react";
import { cn } from "cn";

/**
 * Cell inside the card action strip (edit / duplicate / delete / unpin).
 * Hairline divider on the left, muted icon that brightens on hover.
 * Button-in-Button pattern for the trailing icon.
 */
const iconStripButton =
  "inline-flex size-7 shrink-0 items-center justify-center border-0 border-l border-border bg-transparent p-0 text-muted-foreground transition-colors first:border-l-0 hover:text-foreground active:scale-95 focus-visible:bg-surface-hover focus-visible:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring";

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
  exploding?: boolean;
  explodeDelayMs?: number;
  onDeleted?: (id: string) => void;
}

export default function PresetCard({
  preset, monitors, pins, onEdit, onRefreshParent, onPinChange,
  onApply, appliedMap, staggerEnter, staggerMs, onDuplicated,
  highlightEnter, selectable, selected, onToggleSelect,
  exploding, explodeDelayMs, onDeleted,
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
        saturation: preset.saturation,
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
        window.setTimeout(resolve, 300);
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

  // Custom tag color tints the whole pad. Format-guarded: a hand-edited
  // presets.json could hold a non-hex string, which must never reach CSS.
  // Flat 25% tint over the white card, no glow. Border follows the tag via
  // inset outline (not box-shadow, so the hard offset shadow and the
  // focus-visible ring keep working untouched).
  // Applied (mint fill) is a class below; an explicit tag tint wins over it
  // via inline style, same precedence as before.
  const tag = /^#[0-9a-fA-F]{6}$/.test(preset.color_tag ?? "")
    ? (preset.color_tag as string)
    : null;
  const padBackground = tag
    ? `color-mix(in srgb, var(--color-surface), ${tag} 25%)`
    : undefined;
  const padStyle: Record<string, string> | undefined =
    padBackground || tag
      ? ({
          ...(padBackground ? { backgroundColor: padBackground } : {}),
          ...(tag ? { outline: `2px solid ${tag}`, outlineOffset: "-2px" } : {}),
        } as Record<string, string>)
      : undefined;

  // Category split on first " - "
  const { category, title } = splitCategory(preset.name);

  // Title is a fixed 32px for every card; overflow is handled by
  // truncation + hover marquee + full-name tooltip, never by resizing.
  const titleSize = "32px";
  const titleTextClass = "text-foreground font-display font-extrabold";
  const titleTextStyle = {
    fontSize: titleSize,
    lineHeight: "1",
    letterSpacing: "-0.02em",
  } as Record<string, string>;

  // Marquee only when the title actually overflows: measure full text
  // against the button width (re-checked on resize + title change).
  const titleBtnRef = useRef<HTMLButtonElement>(null);
  const titleMeasureRef = useRef<HTMLSpanElement>(null);
  const [titleOverflows, setTitleOverflows] = useState(false);
  useEffect(() => {
    const btn = titleBtnRef.current;
    const measure = titleMeasureRef.current;
    if (!btn || !measure) {
      setTitleOverflows(false);
      return;
    }
    const check = () => setTitleOverflows(measure.scrollWidth > btn.clientWidth + 1);
    check();
    const ro = new ResizeObserver(check);
    ro.observe(btn);
    return () => ro.disconnect();
  }, [title, titleSize]);

  // Stat values
  const statValue = (key: string): number => {
    switch (key) {
      case "gamma": return preset.gamma;
      case "brightness": return preset.brightness;
      case "contrast": return preset.contrast;
      case "vibrance": return preset.vibrance;
      case "saturation": return preset.saturation;
      default: return 0;
    }
  };

  return (
    <div
      className={cn(
        staggerEnter ? "enter-stagger" : "",
        selectable && !detonating ? "card-tremble" : "",
      )}
      style={staggerEnter && staggerMs !== undefined
        ? ({ "--stagger-ms": `${staggerMs}ms` } as Record<string, string>)
        : undefined
      }
    >
      {/* ── Card shell (Double-Bezel) ───────────────────────────────────────────── */}
      <div
        data-slot="card"
        className={cn(
          "card-ring",
          "flex flex-col gap-3.5 w-full rounded-card border border-border bg-surface p-[18px] outline-none cursor-pointer",
          "focus-visible:ring-[3px] focus-visible:ring-ring",
          isActive ? "border-primary" : "",
          highlightEnter ? "pop-in" : "",
          detonating ? "card-explode" : "",
        )}
        style={{
          ...padStyle,
          viewTransitionName: `preset-card-${preset.id}`,
          // Delay ONLY while detonating: a shared animation-delay would
          // phase-shift the scroll-driven edge fade (tremble desync lived
          // here before and pushed every card's fade zone toward the end
          // of travel). Tremble runs sync now; explosion keeps its cascade.
          ...(detonating && explodeDelayMs !== undefined
            ? ({ animationDelay: `${explodeDelayMs}ms` } as Record<string, string>)
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
        <div className="flex items-center justify-between gap-2 h-8">
          {/* Left cluster: selection checkbox is ADDED in select mode;
              card content never changes with mode. */}
          <div className="flex min-w-0 items-center gap-1.5 overflow-hidden">
            {selectable && (
              // Decorative mirror of pad state; the pad itself toggles.
              <span className="pointer-events-none" aria-hidden="true">
                <Checkbox checked={selected ?? false} onCheckedChange={() => {}} tabIndex={-1}>
                  <CheckboxIndicator />
                </Checkbox>
              </span>
            )}
            <>
              {/* Category label — the only elastic item; it truncates so the
                  status chips next to it are never pushed under the strip. */}
              {category && (
                <span className="min-w-0 truncate text-[11px] font-mono font-bold tracking-[0.14em] uppercase text-muted-foreground">
                  {category}
                </span>
              )}

              {/* ICC tag */}
              {preset.icc_hash && (
                <span className="pop-in inline-flex shrink-0 items-center whitespace-nowrap rounded-full border border-border bg-surface px-2 py-px text-[10px] font-mono font-bold tracking-[0.1em] text-muted-foreground">
                  ICC
                </span>
              )}

              {/* IN USE marker: mint fill, dark text */}
              {isActive && (
                <span
                  className="pop-in inline-flex shrink-0 items-center whitespace-nowrap rounded-full bg-primary px-[7px] py-px text-[10px] font-mono font-bold tracking-[0.1em] text-primary-foreground"
                  title="Currently applied to a connected monitor"
                >
                  IN USE
                </span>
              )}
            </>
          </div>

          {/* Right: action icons — hidden in selection mode */}
          {!selectable && (
            <div className="flex shrink-0 items-center overflow-hidden rounded-lg border border-border bg-surface/50">
              {isPinned && (
                <button
                  type="button"
                  className={cn(iconStripButton, "pop-in")}
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
                  <PinOff className="size-[15px] stroke-[1.75]" />
                </button>
              )}

              <button
                type="button"
                className={iconStripButton}
                onClick={(e) => {
                  e.stopPropagation();
                  onEdit(preset);
                }}
                onKeyDown={(e) => e.stopPropagation()}
                title="Edit"
                aria-label={`Edit preset ${preset.name}`}
              >
                <Pencil className="size-[15px] stroke-[1.75]" />
              </button>

              <button
                type="button"
                className={iconStripButton}
                onClick={(e) => {
                  e.stopPropagation();
                  void handleDuplicate();
                }}
                onKeyDown={(e) => e.stopPropagation()}
                title="Duplicate preset"
                aria-label={`Duplicate preset ${preset.name}`}
              >
                <Copy className="size-[15px] stroke-[1.75]" />
              </button>

              <button
                type="button"
                className={cn(iconStripButton, "hover:text-destructive")}
                onClick={(e) => {
                  e.stopPropagation();
                  setShowDeleteModal(true);
                }}
                onKeyDown={(e) => e.stopPropagation()}
                title="Delete preset"
                aria-label={`Delete preset ${preset.name}`}
              >
                <Trash2 className="size-[15px] stroke-[1.75]" />
              </button>
            </div>
          )}
        </div>

        {/* ═══ Row 2: Title as apply button ═══ */}
        <button
          ref={titleBtnRef}
          type="button"
          className="preset-title relative flex items-center text-left w-full bg-transparent border-0 cursor-pointer rounded transition-opacity hover:opacity-85 focus-visible:outline-2 focus-visible:outline-foreground focus-visible:outline-offset-2"
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
            : `Apply preset ${preset.name}`}
          aria-pressed={selectable ? (selected ? "true" : "false") : undefined}
        >
          {/* Invisible full-text measurer for overflow detection */}
          <span
            ref={titleMeasureRef}
            aria-hidden="true"
            className={`absolute invisible whitespace-nowrap ${titleTextClass}`}
            style={titleTextStyle}
          >
            {title}
          </span>
          {titleOverflows ? (
            <span className="block w-full overflow-hidden whitespace-nowrap" title={preset.name}>
              <span className="marquee-track">
                <span className={`pr-10 ${titleTextClass}`} style={titleTextStyle}>
                  {title}
                </span>
                <span className={`pr-10 ${titleTextClass}`} style={titleTextStyle} aria-hidden="true">
                  {title}
                </span>
              </span>
            </span>
          ) : (
            <span
              className={`truncate ${titleTextClass}`}
              style={titleTextStyle}
              title={preset.name}
            >
              {title}
            </span>
          )}
        </button>

        {/* ═══ Row 3: Stat grid (identical in every mode) ═══ */}
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

        {/* ── Pin state (below grid, every mode) ── */}
        {isPinned && pinnedMonitorNames.length > 0 && (
          <span className="flex min-w-0 items-center gap-1.5 text-xs leading-tight text-muted-foreground">
            <Badge
              variant="pinned"
              className="h-4 shrink-0 px-1.5 text-[9px] tracking-[0.1em]"
              title={`Pinned to ${pinnedMonitorNames.join(", ")}`}
            >
              PINNED
            </Badge>
            <span className="truncate">→ {pinnedMonitorNames.join(", ")}</span>
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
              <Button
                type="button"
                data-slot="button"
                variant="outline"
                size="sm"
                onClick={() => setShowDeleteModal(false)}
                disabled={deleting}
              >
                <span className="btn-label">Cancel</span>
              </Button>
              <Button
                type="button"
                data-slot="button"
                variant="destructive"
                size="sm"
                onClick={() => void handleDelete()}
                disabled={deleting}
              >
                <span className="btn-label">{deleting ? "Deleting…" : "Delete"}</span>
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}