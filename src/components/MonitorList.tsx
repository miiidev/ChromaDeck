import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Monitor, Preset } from "../lib/types";
import PresetCard from "./PresetCard";
import { deletePreset } from "../lib/tauri";
import { Skeleton } from "@/components/ui/skeleton";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogFooter, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import WindowDots from "@/components/ui/window-dots";
import { consumeStagger } from "../lib/motion";

interface Props {
  monitors: Monitor[];
  presets: Preset[];
  loading: boolean;
  onEdit: (preset: Preset) => void;
  onRefresh: () => void;
  onCreateNew: () => void;
  pins: Record<string, string>;
  onPinChange: () => void;
  onApply: (preset: Preset) => void;
  appliedMap: Record<string, string>;
  onPresetDuplicated?: (preset: Preset) => void;
  onBatchDeleted?: (deletedIds: string[]) => void;
  onPresetDeleted?: (id: string) => void;
  lastAddedId?: string | null;
}

/** Loading skeleton */
function SkeletonRow() {
  return (
    <div className="rounded-lg border-2 border-ink bg-white p-4 space-y-3">
      <Skeleton className="h-4 w-1/3" />
      <Skeleton className="h-3 w-1/2" />
    </div>
  );
}

/** Full empty state */
function EmptyState({ onCreateNew }: { onCreateNew: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center space-y-4 empty-state-enter">
      <div className="rounded-brutal border-2 border-ink bg-white p-5 shadow-brutal">
        <svg className="size-10 text-ink" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 12V5.25" />
        </svg>
      </div>
      <h3 className="font-display text-lg font-extrabold uppercase tracking-wide text-ink">No presets yet</h3>
      <p className="text-xs text-muted-foreground max-w-xs">
        Create your first colour preset to apply an ICC profile and gamma adjustment to a monitor.
      </p>
      <Button variant="default" size="sm" onClick={() => onCreateNew()}>
        Create preset
      </Button>
    </div>
  );
}

export default function MonitorList({ monitors, presets, loading, onEdit, onRefresh, onCreateNew, pins, onPinChange, onApply, appliedMap, onPresetDuplicated, onBatchDeleted, onPresetDeleted, lastAddedId }: Props) {
  const [selecting, setSelecting] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [showBatchDeleteModal, setShowBatchDeleteModal] = useState(false);
  const [deletingBatch, setDeletingBatch] = useState(false);
  const [batchDeleteErrors, setBatchDeleteErrors] = useState<string[]>([]);
  const [explodingIds, setExplodingIds] = useState<Set<string>>(new Set());
  const fuseRef = useRef<number | null>(null);

  const clearFuse = () => {
    if (fuseRef.current !== null) {
      window.clearTimeout(fuseRef.current);
      fuseRef.current = null;
    }
    setExplodingIds(new Set());
  };

  // Clear a mid-fuse detonation if the component unmounts.
  useEffect(() => () => {
    if (fuseRef.current !== null) window.clearTimeout(fuseRef.current);
  }, []);

  const exitSelecting = () => {
    clearFuse();
    setSelecting(false);
    setSelectedIds(new Set());
    setBatchDeleteErrors([]);
  };

  // ── Escape exits selection mode ──────────────────────────────────
  useEffect(() => {
    if (!selecting) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        exitSelecting();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [selecting]);

  const toggleSelect = (id: string) => {
    // Dead cards can't be (de)selected mid-explosion.
    if (explodingIds.has(id)) return;
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const computeInUse = (presetId: string): boolean => {
    const pinnedToConnected = Object.entries(pins)
      .some(([edid, pid]) => pid === presetId && monitors.some((m) => m.edid_id === edid && m.connected));
    const appliedToConnected = Object.entries(appliedMap)
      .some(([edid, pid]) => pid === presetId && monitors.some((m) => m.edid_id === edid && m.connected));
    return pinnedToConnected || appliedToConnected;
  };

  // Two-phase detonation: light the fuse (cards explode visually), wait
  // out the animation cascade, then perform the deletes and refresh.
  // Stays in selection mode afterwards — Cancel/Escape still exit.
  const EXPLODE_MS = 450;
  const EXPLODE_STEP_MS = 40;
  const EXPLODE_CAP = 10;

  const handleBatchDelete = async () => {
    const idsToDelete = [...selectedIds];
    if (idsToDelete.length === 0) {
      setShowBatchDeleteModal(false);
      return;
    }
    setShowBatchDeleteModal(false);
    setExplodingIds(new Set(idsToDelete));
    setDeletingBatch(true);
    const cascadeMs = Math.min(idsToDelete.length, EXPLODE_CAP) * EXPLODE_STEP_MS;
    await new Promise<void>((resolve) => {
      fuseRef.current = window.setTimeout(() => {
        fuseRef.current = null;
        resolve();
      }, EXPLODE_MS + cascadeMs);
    });

    const failed: { id: string; name: string }[] = [];
    for (const id of idsToDelete) {
      try {
        await deletePreset(id);
      } catch {
        const p = presets.find((p) => p.id === id);
        failed.push({ id, name: p?.name || id });
      }
    }

    setExplodingIds(new Set());
    setDeletingBatch(false);

    const succeededIds = idsToDelete.filter((id) => !failed.some((f) => f.id === id));
    if (failed.length === 0) {
      // Revived cards (delete failed) come back via refresh below.
      setSelectedIds(new Set());
      setBatchDeleteErrors([]);
    } else {
      // Keep selection on failed items; clear successfully deleted ones
      const failedIds = new Set(failed.map((f) => f.id));
      setSelectedIds(failedIds);
      setBatchDeleteErrors(failed.map((f) => f.name));
    }

    // No full reload: drop the deleted cards locally (the explosion already
    // played) and let the parent sync quietly. Falls back to refresh.
    if (onBatchDeleted) onBatchDeleted(succeededIds);
    else onRefresh();
  };

  // ── Early return when loading ───────────────────────────────────
  if (loading) {
    return (
      <div className="flex-1 min-w-0 min-h-0 w-full space-y-4 overflow-y-auto shell-enter" style={{ ["--shell-delay" as string]: "120ms" }}>
        <Skeleton className="h-6 w-20 mb-6" />
        <SkeletonRow />
        <SkeletonRow />
      </div>
    );
  }

  // Full empty state
  if (monitors.length === 0 && presets.length === 0) {
    return (
      <div className="flex-1 min-w-0 min-h-0 w-full space-y-6 overflow-y-auto shell-enter" style={{ ["--shell-delay" as string]: "120ms" }}>
      {/* Sticky header: solid paper so text and Create stay legible over
          scrolling cards, with a hard rule underneath instead of an edge. */}
      <div className="flex items-center justify-between sticky top-0 z-10 pb-4 bg-paper border-b-2 border-ink">
          <span className="inline-flex items-center gap-2">
            <WindowDots />
            <h2 className="font-display text-base font-extrabold uppercase tracking-wide text-ink mono">Library</h2>
          </span>
          <Button variant="default" size="sm" onClick={() => onCreateNew()}>
            + Create
          </Button>
        </div>
        <EmptyState onCreateNew={onCreateNew} />
      </div>
    );
  }

  return (
    // Flex constraints live HERE (not just on LibraryScrollRoot): this div
    // is the flex item in App's content column, and without flex-1 min-h-0
    // it grows unbounded (min-height:auto) and the scrollport below never
    // gets a height bound — killing library scroll. See: batch-delete wrap.
    <div
      className="flex-1 min-w-0 min-h-0 w-full flex flex-col"
      onKeyDown={(e) => {
        if (e.key === "Escape" && selecting) {
          e.preventDefault();
          e.stopPropagation();
          setSelecting(false);
          setSelectedIds(new Set());
          setBatchDeleteErrors([]);
        }
      }}
    >
          {batchDeleteErrors.length > 0 && (
            <div className="px-1.5 pb-2">
              <span className="text-xs font-bold text-danger-ink block validation-slide">
                Failed to delete: {batchDeleteErrors.join(", ")}
              </span>
            </div>
          )}
      <LibraryScrollRoot
        header={
          selecting ? (
            <div className="flex items-center justify-between pb-2">
              <span className="inline-flex items-center gap-2">
                <WindowDots />
                <h2 className="font-display text-base font-extrabold uppercase tracking-wide text-ink mono">Library</h2>
              </span>
              <div className="flex items-center gap-2">
                <Button variant="outline" size="sm" onClick={exitSelecting} disabled={deletingBatch}>
                  Cancel
                </Button>
                <Button variant="destructive" size="sm" disabled={selectedIds.size === 0 || deletingBatch} onClick={() => setShowBatchDeleteModal(true)}>
                  {deletingBatch ? "Deleting…" : `Delete (${selectedIds.size})`}
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-between pb-2">
              <span className="inline-flex items-center gap-2">
                <WindowDots />
                <h2 className="font-display text-base font-extrabold uppercase tracking-wide text-ink mono">Library</h2>
              </span>
              <div className="flex items-center gap-2">
                <Button variant="secondary" size="sm" onClick={() => { setSelecting(true); setBatchDeleteErrors([]); }}>
                  Select
                </Button>
                <Button variant="default" size="sm" onClick={() => onCreateNew()}>
                  + Create
                </Button>
              </div>
            </div>
          )
        }
      >
        {/* ── Global preset deck ──────────────────────────────────────── */}
        {presets.length === 0 ? (
          <div className="flex flex-col items-center justify-center flex-1 min-h-64 py-12 text-center space-y-3 rounded-lg border-2 border-dashed border-ink bg-white">
            <p className="text-xs text-muted-foreground">No presets yet</p>
            <Button variant="default" size="sm" onClick={() => onCreateNew()}>
              Create preset
            </Button>
          </div>
        ) : (
          <div className="preset-deck grid gap-3 p-2 [grid-template-columns:repeat(auto-fill,minmax(12rem,1fr))]">
            {/* p-2 breathing room: rings, focus rings, and tag glows paint
                outside the card box and would clip at the scrollport walls
                (first/last columns sit flush against the edge). No negative
                margin here: it would push the grid past the scrollport and
                cause horizontal overflow. */}
            {(() => {
              const isStagger = !loading && presets.length > 0 && consumeStagger();
              return presets.map((preset, i) => (
                <PresetCard
                  key={preset.id}
                  preset={preset}
                  monitors={monitors}
                  pins={pins}
                  onEdit={onEdit}
                  onRefreshParent={onRefresh}
                  onPinChange={onPinChange}
                  onApply={onApply}
                  appliedMap={appliedMap}
                  staggerEnter={isStagger}
                  staggerMs={Math.min(i, 5) * 60}
                onDuplicated={onPresetDuplicated}
                onDeleted={onPresetDeleted}
                highlightEnter={!isStagger && lastAddedId === preset.id}
                exploding={explodingIds.has(preset.id)}
                explodeDelayMs={Math.min(i, 10) * 40}
                  selectable={selecting}
                  selected={selectedIds.has(preset.id)}
                  onToggleSelect={() => toggleSelect(preset.id)}
                />
              ));
            })()}
          </div>
        )}

        {/* ── Batch delete confirmation dialog ───────────────────── */}
        {(() => {
          if (!showBatchDeleteModal) return null;
          const selectedArr = presets.filter((p) => selectedIds.has(p.id));
          const nameList = selectedArr.slice(0, 5).map((p) => `"${p.name}"`).join(", ");
          const nameOverflow = selectedArr.length > 5 ? `, and ${selectedArr.length - 5} more` : "";
          const pinnedCount = selectedArr.filter((p) => Object.values(pins).includes(p.id)).length;
          const inUseCount = selectedArr.filter((p) => computeInUse(p.id)).length;
          const hasWarning = pinnedCount > 0 || inUseCount > 0;
          return (
            <Dialog open onOpenChange={(open) => { if (!open && !deletingBatch) setShowBatchDeleteModal(false); }}>
              <DialogContent>
                <DialogHeader>
                  <DialogTitle>Delete {selectedIds.size} preset{selectedIds.size !== 1 ? "s" : ""}?</DialogTitle>
                  <DialogDescription>
                    Permanently delete {nameList}{nameOverflow}?
                    {hasWarning && (
                      <span className="block text-xs text-destructive mt-1">
                        Warning: {[
                          pinnedCount > 0 && `${pinnedCount} pinned`,
                          inUseCount > 0 && `${inUseCount} in use`,
                        ].filter(Boolean).join(", ")}.
                      </span>
                    )}
                  </DialogDescription>
                </DialogHeader>
                <DialogFooter>
                  <Button variant="outline" size="sm" onClick={() => setShowBatchDeleteModal(false)} disabled={deletingBatch}>
                    Cancel
                  </Button>
                  <Button variant="destructive" size="sm" onClick={() => void handleBatchDelete()} disabled={deletingBatch}>
                    {deletingBatch ? "Deleting…" : `Delete ${selectedIds.size}`}
                  </Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>
          );
        })()}
      </LibraryScrollRoot>
    </div>
  );
}

/** Plain scroll container. Edge fading is fully declarative now
 * (scroll-driven card animations in CSS); no scroll listeners, no
 * rAF, no dataset writes. The header lives outside the scrollport.
 *
 * When the runtime does NOT support CSS animation-timeline: view()
 * (suspected in older WebView2), an IntersectionObserver fallback
 * writes per-card --edge-o CSS variables and the .no-view-timeline
 * class activates companion CSS rules. Feature-detect runs once on
 * mount; on supporting runtimes the fallback installs nothing. */
function LibraryScrollRoot({ header, children }: { header: ReactNode; children: ReactNode }) {
  const scrollRef = useRef<HTMLDivElement>(null);

  // ── IntersectionObserver fallback for unsupported runtimes ─────
  useEffect(() => {
    const scroll = scrollRef.current;
    if (!scroll) return;

    // 1. Feature-detect CSS scroll-driven animations
    const supportsViewTimeline =
      typeof CSS !== "undefined" &&
      typeof CSS.supports === "function" &&
      CSS.supports("animation-timeline", "view()");
    if (supportsViewTimeline) return; // CSS handles it — install nothing

    // 2. Runtime guard: IntersectionObserver / MutationObserver present
    if (typeof IntersectionObserver === "undefined" || typeof MutationObserver === "undefined") return;

    // 3. Respect prefers-reduced-motion
    try {
      const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
      if (mq.matches) return; // reduced motion — cards stay fully opaque
    } catch {
      /* matchMedia unavailable — fall through */
    }

    // 4. Activate fallback CSS rules on the scroll container
    scroll.classList.add("no-view-timeline");

    // 5. Dense thresholds: 0 to 1 step 0.05
    const thresholds: number[] = [0, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9, 0.95, 1];

    // 6. Create IntersectionObserver with scroll container as root
    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const el = entry.target as HTMLElement;
          // Map ratio r → min(1, r/0.35): fully opaque through the
          // middle ~65%, fade occupies the outer ~35% at each edge,
          // mirroring the 35/65 keyframe shoulders of card-edge-vanish.
          const opacity = Math.min(1, entry.intersectionRatio / 0.35);
          el.style.setProperty("--edge-o", String(opacity));
        }
      },
      {
        root: scroll,
        threshold: thresholds,
      },
    );

    // 7. Sync IO observations with .preset-deck children (initial + on changes)
    const syncCards = () => {
      const deck = scroll.querySelector(".preset-deck");
      if (!deck) return;
      io.disconnect();
      for (const child of deck.children) {
        io.observe(child);
      }
    };

    syncCards();

    // 8. Watch for grid changes (duplicate → new card, delete → removal)
    const mut = new MutationObserver(() => syncCards());
    mut.observe(scroll, { childList: true, subtree: true });

    // 9. Cleanup on unmount — no observer leaks
    return () => {
      io.disconnect();
      mut.disconnect();
      scroll.classList.remove("no-view-timeline");
    };
  }, []);

  return (
    <div
      className="flex-1 min-w-0 min-h-0 w-full flex flex-col shell-enter"
      style={{ ["--shell-delay" as string]: "120ms" }}
    >
      {header}
      <div
        ref={scrollRef}
        className="flex-1 min-h-0 overflow-y-auto overflow-x-clip"
      >
        {children}
      </div>
    </div>
  );
}