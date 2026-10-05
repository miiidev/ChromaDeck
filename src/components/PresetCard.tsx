import { useState, useEffect } from "react";
import type { Monitor, Preset } from "../lib/types";
import { deletePreset, createPreset, unpinMonitor } from "../lib/tauri";

interface Props {
  preset: Preset;
  monitors: Monitor[];
  pins: Record<string, string>;
  onEdit: (preset: Preset) => void;
  onRefreshParent: () => void;
  onPinChange: () => void;
  onApply: (preset: Preset) => void;
  appliedMap: Record<string, string>;
}

export default function PresetCard({ preset, monitors, pins, onEdit, onRefreshParent, onPinChange, onApply, appliedMap }: Props) {
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    if (!showDeleteModal) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShowDeleteModal(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [showDeleteModal]);

  const handleDuplicate = async () => {
    try {
      await createPreset({
        name: `${preset.name} (copy)`,
        brightness: preset.brightness,
        contrast: preset.contrast,
        rgb_gains: preset.rgb_gains,
        gamma: preset.gamma,
        vibrance: preset.vibrance,
        hue_deg: preset.hue_deg,
      });
      onRefreshParent();
    } catch {
      // silent
    }
  };

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await deletePreset(preset.id);
      setShowDeleteModal(false);
      onRefreshParent();
    } catch {
      setShowDeleteModal(false);
    } finally {
      setDeleting(false);
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

  return (
    <div className="border-2 border-ink bg-surface shadow-card motion-reduce:shadow-[2px_2px_0px_var(--shadow-clr)] flex flex-col min-h-44 relative">
      {/* 6px left rail: green if in use, red if pinned, blue otherwise */}
      {isActive ? (
        <div className="card-rail-active" title="Currently in use" />
      ) : isPinned ? (
        <div className="card-rail-pinned" title="Pinned (monitor offline)" />
      ) : (
        <div className="card-rail-default" />
      )}

      {/* Card body — stacked info */}
      <div className="p-3 space-y-2">
        {/* Name + badges row */}
        <div className="flex items-start justify-between gap-2">
          <span className="text-sm font-medium text-ink truncate leading-tight font-body">
            {preset.name}
          </span>
          <div className="flex items-center gap-1 shrink-0">
            {isPinned && (
              <span className="text-[10px] font-medium text-blue border border-blue px-1.5 py-0.5 leading-none uppercase tracking-widest" title={`Pinned to ${pinnedMonitorNames.join(", ")}`}>
                PINNED
              </span>
            )}
            {preset.icc_hash && (
              <span className="text-[10px] font-mono text-secondary border border-ink px-1.5 py-0.5 leading-none truncate max-w-[72px]" title={preset.icc_filename}>
                ICC
              </span>
            )}
          </div>
        </div>

        {/* Parameter row — mono numerals */}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted font-mono">
          <span>γ{preset.gamma.toFixed(1)}</span>
          <span>B{preset.brightness.toFixed(0)}</span>
          <span>C{preset.contrast.toFixed(0)}</span>
          <span>RGB {preset.rgb_gains.map((v) => v.toFixed(1)).join("/")}</span>
          <span>V{preset.vibrance.toFixed(0)}</span>
          <span>H{preset.hue_deg.toFixed(0)}°</span>
        </div>

        {/* Pin target hint */}
        {pinnedMonitorNames.length > 0 && (
          <span className="text-[10px] text-blue/70 block mt-0.5">
            → {pinnedMonitorNames.join(", ")}
          </span>
        )}
      </div>

      {/* Action buttons row */}
      <div className="flex flex-wrap items-center gap-1 px-3 pb-3">
        {/* APPLY = red bg + white text */}
        <button
          onClick={() => onApply(preset)}
          className="bauhaus-btn px-3 py-1 text-xs font-medium border-2 border-red bg-primary-red shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 hover:brightness-110"
          style={{ color: "white" }}
        >
          APPLY
        </button>

        {isPinned && (
          /* UNPIN = yellow bg + ink text */
          <button
            onClick={async () => {
              try {
                for (const edid of pinnedEdidList) {
                  await unpinMonitor(edid);
                }
                onPinChange();
              } catch {
                // silent
              }
            }}
            className="bauhaus-btn px-2 py-1 text-xs font-medium border-2 border-yellow bg-primary-yellow text-ink shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 hover:brightness-110"
            title="Unpin from all monitors"
          >
            UNPIN
          </button>
        )}

        {/* EDIT = blue bg + white text */}
        <button
          onClick={() => onEdit(preset)}
          className="bauhaus-btn px-2 py-1 text-xs font-medium border-2 border-blue bg-primary-blue shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 hover:brightness-110"
          style={{ color: "white" }}
        >
          EDIT
        </button>

        {/* DUP = paper/ink outline */}
        <button
          onClick={handleDuplicate}
          className="bauhaus-btn px-2 py-1 text-xs font-medium border-2 border-ink bg-surface text-secondary hover:bg-surface-hover shadow-btn"
          title="Duplicate preset"
        >
          DUP
        </button>

        {/* DEL = ink bg + red text + red border */}
        <button
          onClick={() => setShowDeleteModal(true)}
          className="bauhaus-btn px-2 py-1 text-xs font-medium border-2 border-red bg-ink text-red hover:bg-red shadow-btn"
          style={{ color: "white", backgroundColor: "var(--red)" }}
          title="Delete preset"
          onMouseEnter={(e) => { (e.target as HTMLElement).style.backgroundColor = "var(--red)"; (e.target as HTMLElement).style.color = "white"; }}
          onMouseLeave={(e) => { (e.target as HTMLElement).style.backgroundColor = ""; (e.target as HTMLElement).style.color = ""; }}
        >
          DEL
        </button>
      </div>

      {/* Delete confirmation popup */}
      {showDeleteModal && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-ink/70 p-4"
          onClick={() => { if (!deleting) setShowDeleteModal(false); }}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label={`Delete preset ${preset.name}`}
            className="w-full max-w-xs border-2 border-ink bg-surface p-4 shadow-modal"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-bold uppercase tracking-widest text-ink font-heading">
              Delete preset?
            </h3>
            <p className="mt-2 text-xs text-muted leading-relaxed">
              Permanently delete <span className="font-medium text-ink">"{preset.name}"</span>?
              This cannot be undone.
            </p>
            <div className="mt-4 flex items-center justify-end gap-2">
              <button
                onClick={() => setShowDeleteModal(false)}
                disabled={deleting}
                className="bauhaus-btn px-3 py-1.5 text-xs font-medium border-2 border-ink bg-surface text-secondary hover:bg-surface-hover disabled:opacity-50"
              >
                CANCEL
              </button>
              <button
                onClick={() => void handleDelete()}
                disabled={deleting}
                className="bauhaus-btn px-3 py-1.5 text-xs font-medium border-2 border-red shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-primary-red disabled:opacity-50 disabled:cursor-wait"
                style={{ color: "white" }}
              >
                {deleting ? "DELETING…" : "DELETE"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}