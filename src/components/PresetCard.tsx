import { useState, useEffect } from "react";
import type { Monitor, Preset, ApplyResult } from "../lib/types";
import { applyPreset, deletePreset, createPreset, pinPreset, unpinMonitor } from "../lib/tauri";

interface Props {
  preset: Preset;
  monitor: Monitor;
  onEdit: (preset: Preset) => void;
  onRefreshParent: () => void;
  isPinned: boolean;
  onPinChange: () => void;
}

export default function PresetCard({ preset, monitor, onEdit, onRefreshParent, isPinned, onPinChange }: Props) {
  const [applying, setApplying] = useState(false);
  const [lastResult, setLastResult] = useState<ApplyResult | null>(null);
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

  const handleApply = async () => {
    if (!monitor.connected) return;
    setApplying(true);
    setLastResult(null);
    try {
      const result = await applyPreset(preset.id);
      setLastResult(result);
      if (result.error) {
        setTimeout(() => setLastResult(null), 6000);
      } else {
        setTimeout(() => setLastResult(null), 3000);
      }
    } catch (err) {
      setLastResult({ icc_applied: false, gamma_applied: false, vibrance_applied: false, error: String(err) });
      setTimeout(() => setLastResult(null), 6000);
    } finally {
      setApplying(false);
    }
  };

  const handleDuplicate = async () => {
    try {
      await createPreset({
        name: `${preset.name} (copy)`,
        edid_id: preset.edid_id,
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

  const getApplyResultDisplay = () => {
    if (!lastResult) return null;
    if (lastResult.error) {
      return (
        <span className="text-xs text-red-400 mt-1 block">
          Apply failed: {lastResult.error}
        </span>
      );
    }
    const parts: string[] = [];
    if (lastResult.icc_applied) parts.push("ICC applied");
    if (lastResult.gamma_applied) parts.push("gamma applied");
    if (lastResult.vibrance_applied) parts.push("vibrance applied");
    return (
      <span className="text-xs text-emerald-400 mt-1 block">
        {parts.join(" + ") || "Applied"}
      </span>
    );
  };

  const canApply = monitor.connected && !applying;

  return (
    <div className="border-2 border-neutral-200 bg-neutral-900 shadow-[4px_4px_0px_#e5e7eb] motion-reduce:shadow-[2px_2px_0px_#e5e7eb] flex flex-col min-h-44">
      {/* Pad face — tap to apply */}
      <div
        role="button"
        tabIndex={canApply ? 0 : undefined}
        aria-disabled={!canApply}
        aria-label={`Apply preset ${preset.name}`}
        onClick={() => {
          if (canApply) void handleApply();
        }}
        onKeyDown={(e) => {
          if (canApply && (e.key === "Enter" || e.key === " ")) {
            e.preventDefault();
            void handleApply();
          }
        }}
        className={`flex-1 select-none focus-visible:outline-2 focus-visible:outline-lime-400 focus-visible:outline-offset-2 ${
          canApply ? "cursor-pointer hover:bg-neutral-800/60" : "cursor-not-allowed"
        }`}
      >
      {/* Card body — stacked info */}
      <div className="p-3 space-y-2">
        {/* Name + badges row */}
        <div className="flex items-start justify-between gap-2">
          <span className="text-sm font-medium text-neutral-200 truncate leading-tight">
            {preset.name}
          </span>
          <div className="flex items-center gap-1 shrink-0">
            {isPinned && (
              <span className="text-[10px] font-medium text-lime-400 border border-lime-400 px-1.5 py-0.5 leading-none uppercase tracking-widest">
                PINNED
              </span>
            )}
            {preset.icc_hash && (
              <span className="text-[10px] font-mono text-neutral-500 border border-neutral-600 px-1.5 py-0.5 leading-none truncate max-w-[72px]" title={preset.icc_filename}>
                ICC
              </span>
            )}
          </div>
        </div>

        {/* Parameter row — mono numerals */}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-neutral-500 font-mono">
          <span>γ{preset.gamma.toFixed(1)}</span>
          <span>B{preset.brightness.toFixed(0)}</span>
          <span>C{preset.contrast.toFixed(0)}</span>
          <span>RGB {preset.rgb_gains.map((v) => v.toFixed(1)).join("/")}</span>
          <span>V{preset.vibrance.toFixed(0)}</span>
          <span>H{preset.hue_deg.toFixed(0)}°</span>
        </div>

        {/* Apply result feedback */}
        {getApplyResultDisplay()}
      </div>
      </div>

      {/* Action buttons row — wrap, brutalist */}
      <div className="flex flex-wrap items-center gap-1 px-3 pb-3">
        {monitor.connected && (
          <button
            onClick={handleApply}
            disabled={applying}
            className={`brutalist-btn px-3 py-1 text-xs font-medium border-2 border-lime-400 shadow-[2px_2px_0px_#a3e635] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 ${
              applying
                ? "bg-lime-800 text-lime-300 cursor-wait"
                : "bg-lime-400 text-black hover:bg-lime-300"
            }`}
          >
            {applying ? "APPLY…" : "APPLY"}
          </button>
        )}

        <button
          onClick={async () => {
            try {
              if (isPinned) await unpinMonitor(preset.edid_id);
              else await pinPreset(preset.edid_id, preset.id);
              onPinChange();
            } catch {
              // silent
            }
          }}
          className={`brutalist-btn px-2 py-1 text-xs font-medium border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 ${
            isPinned
              ? "bg-amber-900/40 text-amber-300 hover:bg-amber-800/60"
              : "bg-neutral-800 text-neutral-300 hover:bg-neutral-700"
          }`}
          title={isPinned ? "Stop enforcing this preset" : "Pin as enforced default"}
        >
          {isPinned ? "UNPIN" : "PIN"}
        </button>

        <button
          onClick={() => onEdit(preset)}
          className="brutalist-btn px-2 py-1 text-xs font-medium border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-neutral-800 text-neutral-300 hover:bg-neutral-700"
        >
          EDIT
        </button>

        <button
          onClick={handleDuplicate}
          className="brutalist-btn px-2 py-1 text-xs font-medium border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-neutral-800 text-neutral-500 hover:bg-neutral-700"
          title="Duplicate preset"
        >
          DUP
        </button>

        <button
          onClick={() => setShowDeleteModal(true)}
          className="brutalist-btn px-2 py-1 text-xs font-medium border-2 border-red-500 shadow-[2px_2px_0px_#ef4444] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-neutral-800 text-red-400 hover:bg-red-500 hover:text-black"
          title="Delete preset"
        >
          DEL
        </button>
      </div>

      {/* Delete confirmation popup */}
      {showDeleteModal && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
          onClick={() => {
            if (!deleting) setShowDeleteModal(false);
          }}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label={`Delete preset ${preset.name}`}
            className="w-full max-w-xs border-2 border-neutral-200 bg-neutral-900 p-4 shadow-[6px_6px_0px_#e5e7eb]"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-bold uppercase tracking-widest text-neutral-100">
              Delete preset?
            </h3>
            <p className="mt-2 text-xs text-neutral-400 leading-relaxed">
              Permanently delete <span className="font-medium text-neutral-200">“{preset.name}”</span>?
              This cannot be undone.
            </p>
            <div className="mt-4 flex items-center justify-end gap-2">
              <button
                onClick={() => setShowDeleteModal(false)}
                disabled={deleting}
                className="brutalist-btn px-3 py-1.5 text-xs font-medium border-2 border-neutral-200 bg-neutral-800 text-neutral-300 hover:bg-neutral-700 disabled:opacity-50"
              >
                CANCEL
              </button>
              <button
                onClick={() => void handleDelete()}
                disabled={deleting}
                className="brutalist-btn px-3 py-1.5 text-xs font-medium border-2 border-red-500 shadow-[2px_2px_0px_#ef4444] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-red-600 text-white hover:bg-red-500 disabled:opacity-50 disabled:cursor-wait"
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