import { useEffect, useRef, type ReactNode } from "react";
import type { Monitor, Preset } from "../lib/types";
import PresetCard from "./PresetCard";
import { Skeleton } from "@/components/ui/skeleton";
import { Button } from "@/components/ui/button";
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
}

/** Loading skeleton */
function SkeletonRow() {
  return (
    <div className="rounded-lg border border-border bg-card p-4 space-y-3">
      <Skeleton className="h-4 w-1/3" />
      <Skeleton className="h-3 w-1/2" />
    </div>
  );
}

/** Full empty state */
function EmptyState({ onCreateNew }: { onCreateNew: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center space-y-4 empty-state-enter">
      <div className="rounded-lg border border-border bg-card p-5">
        <svg className="size-10 text-muted-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 12V5.25" />
        </svg>
      </div>
      <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-widest">No presets yet</h3>
      <p className="text-xs text-muted-foreground max-w-xs">
        Create your first colour preset to apply an ICC profile and gamma adjustment to a monitor.
      </p>
      <Button variant="secondary" size="sm" onClick={() => onCreateNew()}>
        Create preset
      </Button>
    </div>
  );
}

export default function MonitorList({ monitors, presets, loading, onEdit, onRefresh, onCreateNew, pins, onPinChange, onApply, appliedMap }: Props) {
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
      {/* Sticky header with melting background: solid up top so text and
          Create stay legible, dissolving to transparent over the bottom
          ~20px so scrolling cards emerge through a gradient, not an edge. */}
      <div className="flex items-center justify-between sticky top-0 z-10 pb-6 bg-[linear-gradient(to_bottom,var(--color-background)_60%,transparent)]">
          <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-widest mono">Library</h2>
          <Button variant="secondary" size="sm" onClick={() => onCreateNew()}>
            + Create
          </Button>
        </div>
        <EmptyState onCreateNew={onCreateNew} />
      </div>
    );
  }

  return (
    <LibraryScrollRoot
      header={
        <div className="flex items-center justify-between pb-2">
          <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-widest mono">Library</h2>
          <Button variant="secondary" size="sm" onClick={() => onCreateNew()}>
            + Create
          </Button>
        </div>
      }
    >
      {/* ── Global preset deck ──────────────────────────────────────── */}
      {presets.length === 0 ? (
        <div className="flex flex-col items-center justify-center flex-1 min-h-64 py-12 text-center space-y-3 rounded-lg border border-dashed bg-background">
          <p className="text-xs text-muted-foreground">No presets yet</p>
          <Button variant="secondary" size="sm" onClick={() => onCreateNew()}>
            Create preset
          </Button>
        </div>
      ) : (
        <div className="grid gap-3 p-2 [grid-template-columns:repeat(auto-fill,minmax(12rem,1fr))]">
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
                staggerMs={i * 80}
              />
            ));
          })()}
        </div>
      )}
    </LibraryScrollRoot>
  );
}

/** Scroll container with edge-fade overlays.
 * The header lives OUTSIDE the scrollport (static, always crisp); the grid
 * scrolls beneath sticky ::before/::after gradient overlays whose opacity
 * eases with scroll position. Position writes go straight to the dataset
 * (no re-render per scroll event) and are rAF-throttled. */
function LibraryScrollRoot({ header, children }: { header: ReactNode; children: ReactNode }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef(0);

  const updateFade = () => {
    const el = scrollRef.current;
    if (!el) return;
    el.dataset.atTop = String(el.scrollTop <= 8);
    el.dataset.atBottom = String(
      el.scrollHeight - el.scrollTop - el.clientHeight <= 8,
    );
  };

  const onScroll = () => {
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(updateFade);
  };

  useEffect(() => {
    updateFade();
    window.addEventListener("resize", updateFade);
    return () => {
      window.removeEventListener("resize", updateFade);
      cancelAnimationFrame(rafRef.current);
    };
  });

  return (
    <div
      className="flex-1 min-w-0 min-h-0 w-full flex flex-col shell-enter"
      style={{ ["--shell-delay" as string]: "120ms" }}
    >
      {header}
      <div
        ref={scrollRef}
        onScroll={onScroll}
        data-at-top="true"
        data-at-bottom="true"
        className="scroll-fades flex-1 min-h-0 overflow-y-auto"
      >
        {children}
      </div>
    </div>
  );
}