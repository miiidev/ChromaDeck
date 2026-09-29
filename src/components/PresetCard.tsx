import { useState } from "react";
import type { Monitor, Preset, ApplyResult } from "../lib/types";
import { applyPreset, deletePreset, createPreset } from "../lib/tauri";

interface Props {
  preset: Preset;
  monitor: Monitor;
  onEdit: (preset: Preset) => void;
  onRefreshParent: () => void;
}

export default function PresetCard({ preset, monitor, onEdit, onRefreshParent }: Props) {
  const [applying, setApplying] = useState(false);
  const [lastResult, setLastResult] = useState<ApplyResult | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const handleApply = async () => {
    if (!monitor.connected) return;
    setApplying(true);
    setLastResult(null);
    try {
      const result = await applyPreset(preset.id);
      setLastResult(result);
      if (result.error) {
        // Auto-clear error after 6s
        setTimeout(() => setLastResult(null), 6000);
      } else {
        // Auto-clear success after 3s
        setTimeout(() => setLastResult(null), 3000);
      }
    } catch (err) {
      setLastResult({ icc_applied: false, gamma_applied: false, error: String(err) });
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
      });
      onRefreshParent();
    } catch {
      // silent
    }
  };

  const handleDelete = async () => {
    try {
      await deletePreset(preset.id);
      setConfirmDelete(false);
      onRefreshParent();
    } catch {
      setConfirmDelete(false);
    }
  };

  const getApplyResultDisplay = () => {
    if (!lastResult) return null;
    if (lastResult.error) {
      return (
        <span className="text-xs text-red-400">
          Apply failed: {lastResult.error}
        </span>
      );
    }
    const parts: string[] = [];
    if (lastResult.icc_applied) parts.push("ICC applied");
    if (lastResult.gamma_applied) parts.push("gamma applied");
    return (
      <span className="text-xs text-emerald-400">
        {parts.join(" + ") || "Applied"}
      </span>
    );
  };

  return (
    <div className="flex items-center justify-between py-2 px-3 rounded-lg bg-neutral-800/40 hover:bg-neutral-800/60 transition-colors group">
      {/* Preset info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-neutral-200 truncate">
            {preset.name}
          </span>
          {preset.icc_hash && (
            <span className="text-xs text-neutral-500 truncate hidden sm:inline">
              ICC{preset.icc_filename ? `: ${preset.icc_filename}` : ""}
            </span>
          )}
        </div>
        <div className="flex items-center gap-3 text-xs text-neutral-500 mt-0.5">
          <span>γ {preset.gamma.toFixed(1)}</span>
          <span>B {preset.brightness.toFixed(2)}</span>
          <span>C {preset.contrast.toFixed(2)}</span>
          <span className="font-mono">
            RGB {preset.rgb_gains.map((v) => v.toFixed(1)).join("/")}
          </span>
        </div>
        {getApplyResultDisplay()}
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1 ml-4 shrink-0">
        {monitor.connected && (
          <button
            onClick={handleApply}
            disabled={applying}
            className={`px-3 py-1 text-xs font-medium rounded-md transition-colors ${
              applying
                ? "bg-indigo-800 text-indigo-300 cursor-wait"
                : "bg-indigo-600 hover:bg-indigo-500 text-white"
            }`}
          >
            {applying ? "Applying…" : "Apply"}
          </button>
        )}

        <button
          onClick={() => onEdit(preset)}
          className="px-2 py-1 text-xs font-medium rounded-md text-neutral-400 hover:text-neutral-200 hover:bg-neutral-700 transition-colors"
        >
          Edit
        </button>

        <button
          onClick={handleDuplicate}
          className="px-2 py-1 text-xs font-medium rounded-md text-neutral-500 hover:text-neutral-200 hover:bg-neutral-700 transition-colors"
          title="Duplicate preset"
        >
          Dup
        </button>

        {confirmDelete ? (
          <div className="flex items-center gap-1">
            <button
              onClick={handleDelete}
              className="px-2 py-1 text-xs font-medium rounded-md bg-red-700 hover:bg-red-600 text-white transition-colors"
            >
              {"Delete?"}
            </button>
            <button
              onClick={() => setConfirmDelete(false)}
              className="px-2 py-1 text-xs font-medium rounded-md text-neutral-500 hover:text-neutral-300"
            >
              Cancel
            </button>
          </div>
        ) : (
          <button
            onClick={() => setConfirmDelete(true)}
            className="px-2 py-1 text-xs font-medium rounded-md text-neutral-600 hover:text-red-400 hover:bg-neutral-700 transition-colors"
            title="Delete preset"
          >
            ×
          </button>
        )}
      </div>
    </div>
  );
}