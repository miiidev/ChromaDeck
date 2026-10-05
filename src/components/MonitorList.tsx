import type { Monitor, Preset } from "../lib/types";
import PresetCard from "./PresetCard";

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
}

/** Loading skeleton */
function SkeletonRow() {
  return (
    <div className="animate-pulse border-2 border-ink bg-surface p-4 space-y-3">
      <div className="h-4 bg-surface-hover w-1/3" />
      <div className="h-3 bg-surface-hover w-1/2" />
    </div>
  );
}

/** Full empty state */
function EmptyState({ onCreateNew }: { onCreateNew: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center space-y-4">
      <div className="border-2 border-ink bg-surface p-5">
        <svg className="w-10 h-10 text-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 12V5.25" />
        </svg>
      </div>
      <h3 className="text-sm font-medium text-secondary uppercase tracking-widest font-heading">No presets yet</h3>
      <p className="text-xs text-muted max-w-xs">
        Create your first colour preset to apply an ICC profile and gamma adjustment to a monitor.
      </p>
      {/* CREATE = blue bg + white text (edit-like action) */}
      <button
        onClick={() => onCreateNew()}
        className="bauhaus-btn px-4 py-2 text-sm font-medium border-2 border-blue bg-primary-blue shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0"
        style={{ color: "white" }}
      >
        CREATE PRESET
      </button>
    </div>
  );
}

export default function MonitorList({ monitors, presets, loading, onEdit, onRefresh, onCreateNew, pins, onPinChange, onApply }: Props) {
  // ── Early return when loading ───────────────────────────────────
  if (loading) {
    return (
      <div className="flex-1 min-w-0 w-full space-y-4">
        <div className="h-6 bg-surface w-20 mb-6 animate-pulse" />
        <SkeletonRow />
        <SkeletonRow />
      </div>
    );
  }

  // Full empty state
  if (monitors.length === 0 && presets.length === 0) {
    return (
      <div className="flex-1 min-w-0 w-full space-y-6">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-medium text-secondary uppercase tracking-widest font-heading">Library</h2>
          <button
            onClick={() => onCreateNew()}
            className="bauhaus-btn px-4 py-2 text-sm font-medium border-2 border-blue bg-primary-blue shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0"
            style={{ color: "white" }}
          >
            + CREATE
          </button>
        </div>
        <EmptyState onCreateNew={onCreateNew} />
      </div>
    );
  }

  return (
    <div className="flex-1 min-w-0 w-full space-y-5">
      {/* ── Header row ──────────────────────────────────────────────── */}
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-secondary uppercase tracking-widest font-heading">Library</h2>
        <button
          onClick={() => onCreateNew()}
          className="bauhaus-btn px-4 py-2 text-sm font-medium border-2 border-blue bg-primary-blue shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0"
          style={{ color: "white" }}
        >
          + CREATE
        </button>
      </div>

      {/* ── Global preset deck ──────────────────────────────────────── */}
      {presets.length === 0 ? (
        <div className="flex flex-col items-center justify-center flex-1 min-h-64 py-12 text-center space-y-3 border-2 border-dashed border-ink bg-paper">
          <p className="text-xs text-muted">No presets yet</p>
          <button
            onClick={() => onCreateNew()}
            className="bauhaus-btn px-4 py-2 text-sm font-medium border-2 border-blue bg-primary-blue shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0"
            style={{ color: "white" }}
          >
            CREATE PRESET
          </button>
        </div>
      ) : (
        <div className="grid gap-4 [grid-template-columns:repeat(auto-fill,minmax(15rem,1fr))]">
          {presets.map((preset) => (
            <PresetCard
              key={preset.id}
              preset={preset}
              monitors={monitors}
              pins={pins}
              onEdit={onEdit}
              onRefreshParent={onRefresh}
              onPinChange={onPinChange}
              onApply={onApply}
            />
          ))}
        </div>
      )}
    </div>
  );
}