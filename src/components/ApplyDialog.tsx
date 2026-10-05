import { useState, useEffect } from "react";
import type { Monitor, Preset, ApplyResult } from "../lib/types";
import { applyPreset, pinPreset, unpinMonitor } from "../lib/tauri";

interface Props {
  preset: Preset;
  monitors: Monitor[];
  pins: Record<string, string>;
  initialEdid?: string | null;
  onClose: () => void;
  onApplied: (info: { presetId: string; edid: string } | null) => void;
}

// In-session memory: reuse the last-selected target per preset
const lastTargetByPreset = new Map<string, string>();

export default function ApplyDialog({ preset, monitors, pins, initialEdid, onClose, onApplied }: Props) {
  const [selectedEdid, setSelectedEdid] = useState<string>("");
  const [pinToggle, setPinToggle] = useState(false);
  const [applying, setApplying] = useState(false);
  const [lastResult, setLastResult] = useState<ApplyResult | null>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Initialise target: explicit sidebar choice → in-session memory → first connected
  useEffect(() => {
    if (selectedEdid) return;
    const explicitOk = initialEdid
      ? monitors.some((m) => m.edid_id === initialEdid && m.connected)
      : false;
    if (explicitOk && initialEdid) {
      setSelectedEdid(initialEdid);
      setPinToggle(pins[initialEdid] === preset.id);
      return;
    }
    const remembered = lastTargetByPreset.get(preset.id);
    const rememberedOk = remembered
      ? monitors.some((m) => m.edid_id === remembered && m.connected)
      : false;
    const fallback = monitors.find((m) => m.connected);
    const target = rememberedOk && remembered ? remembered : fallback?.edid_id;
    if (target) {
      setSelectedEdid(target);
      setPinToggle(pins[target] === preset.id);
    }
  }, [monitors, selectedEdid, preset.id, pins, initialEdid]);

  const handleApply = async () => {
    if (!selectedEdid || applying) return;
    setApplying(true);
    setLastResult(null);
    let applied: { presetId: string; edid: string } | null = null;
    try {
      const result = await applyPreset(preset.id, selectedEdid);
      setLastResult(result);

      if (!result.error) {
        applied = { presetId: preset.id, edid: selectedEdid };
        // Pin / unpin based on toggle
        const wasPinned = pins[selectedEdid] === preset.id;
        if (pinToggle && !wasPinned) {
          try { await pinPreset(selectedEdid, preset.id); } catch { /* best-effort */ }
        } else if (!pinToggle && wasPinned) {
          try { await unpinMonitor(selectedEdid); } catch { /* best-effort */ }
        }
        // Remember this target for next dialog open
        lastTargetByPreset.set(preset.id, selectedEdid);
      }
    } catch (err) {
      setLastResult({
        icc_applied: false,
        gamma_applied: false,
        vibrance_applied: false,
        error: String(err),
      });
    } finally {
      setApplying(false);
      onApplied(applied); // report result, refresh data without closing
    }
  };

  const isPinnedOnTarget = pins[selectedEdid] === preset.id;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/70 p-4"
      onClick={() => { if (!applying) onClose(); }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={`Apply preset ${preset.name}`}
        className="w-full max-w-sm border-2 border-ink bg-surface shadow-modal motion-reduce:shadow-[3px_3px_0px_var(--shadow-clr)]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* ── Header ────────────────────────────────────────────── */}
        <div className="flex items-center justify-between px-5 py-4 border-b-2 border-ink">
          <div className="min-w-0 flex-1">
            <h2 className="text-sm font-semibold text-ink truncate font-heading">
              Apply: {preset.name}
            </h2>
            <p className="mt-0.5 text-[10px] text-secondary font-mono uppercase tracking-widest">
              Select target monitor
            </p>
          </div>
          <button
            onClick={onClose}
            disabled={applying}
            aria-label="Close apply dialog"
            className="bauhaus-btn p-1.5 border-2 border-ink bg-surface text-secondary hover:bg-surface-hover disabled:opacity-50"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* ── Monitor list ──────────────────────────────────────── */}
        <div className="px-5 py-4 space-y-2">
          <p className="text-[10px] font-medium text-secondary uppercase tracking-widest mb-1">
            Monitors
          </p>
          {monitors.length === 0 && (
            <p className="text-xs text-yellow">No monitors detected.</p>
          )}
          {monitors.map((m) => {
            const isSelected = m.edid_id === selectedEdid;
            const isPinnedHere = pins[m.edid_id] === preset.id;
            return (
              <label
                key={m.edid_id}
                className={`flex items-center gap-3 px-3 py-2.5 border-2 cursor-pointer select-none ${
                  isSelected
                    ? "border-blue bg-blue/10"
                    : m.connected
                      ? "border-ink bg-surface hover:bg-surface-hover"
                      : "border-ink bg-ink/30 text-muted"
                }`}
              >
                <input
                  type="radio"
                  name="target-monitor"
                  value={m.edid_id}
                  checked={isSelected}
                  disabled={!m.connected}
                  onChange={() => {
                    if (m.connected) {
                      setSelectedEdid(m.edid_id);
                      setPinToggle(pins[m.edid_id] === preset.id);
                      setLastResult(null);
                    }
                  }}
                  className="accent-blue"
                />
                <span className="flex-1 min-w-0 truncate text-sm font-medium text-ink">
                  {m.alias || m.model || m.device_name}
                </span>
                <span className="inline-flex items-center gap-1 shrink-0">
                  {isPinnedHere && (
                    <span className="text-[10px] font-medium text-blue border border-blue px-1 py-0.5 leading-none uppercase tracking-widest">
                      PINNED
                    </span>
                  )}
                  {m.connected ? (
                    <span className="status-dot status-dot-connected" title="Connected" />
                  ) : (
                    <span className="text-[10px] text-secondary uppercase tracking-widest">
                      OFFLINE
                    </span>
                  )}
                </span>
              </label>
            );
          })}
        </div>

        {/* ── Pin toggle ────────────────────────────────────────── */}
        <div className="px-5 py-3 flex items-center gap-3 border-t-2 border-ink">
          <label className="inline-flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={pinToggle}
              onChange={(e) => setPinToggle(e.target.checked)}
              className="accent-blue"
            />
            <span className="text-[10px] font-medium text-secondary uppercase tracking-widest">
              PIN TO MONITOR
            </span>
          </label>
          {isPinnedOnTarget && pinToggle && (
            <span className="text-[10px] text-blue">Already pinned</span>
          )}
        </div>

        {/* ── Result feedback — persistent ──────────────────────── */}
        {lastResult && (
          <div className="px-5 py-2" aria-live="polite">
            {lastResult.error ? (
              <span className="text-xs text-red block">Apply failed: {lastResult.error}</span>
            ) : (
              <span className="text-xs text-blue block">
                {[
                  lastResult.icc_applied && "ICC applied",
                  lastResult.gamma_applied && "gamma applied",
                  lastResult.vibrance_applied && "vibrance applied",
                ]
                  .filter(Boolean)
                  .join(" + ") || "Applied"}
                {pinToggle ? " · PINNED" : ""}
              </span>
            )}
          </div>
        )}

        {/* ── Actions ───────────────────────────────────────────── */}
        <div className="flex items-center justify-end gap-3 px-5 py-4 border-t-2 border-ink">
          <button
            onClick={onClose}
            disabled={applying}
            className="bauhaus-btn px-4 py-2 text-sm font-medium border-2 border-ink bg-surface text-secondary hover:bg-surface-hover disabled:opacity-50"
          >
            CANCEL
          </button>
          {lastResult && !lastResult.error && (
            <button
              onClick={onClose}
              className="bauhaus-btn px-4 py-2 text-sm font-medium border-2 border-ink bg-surface text-secondary hover:bg-surface-hover shadow-btn"
            >
              DONE
            </button>
          )}
          {/* APPLY = red bg + white text */}
          <button
            onClick={handleApply}
            disabled={!selectedEdid || applying}
            className={`bauhaus-btn px-6 py-2 text-sm font-medium border-2 border-red shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 ${
              applying ? "opacity-50 cursor-wait" : ""
            }`}
            style={{ color: "white", backgroundColor: "var(--red)" }}
          >
            {applying ? "APPLYING…" : "APPLY"}
          </button>
        </div>
      </div>
    </div>
  );
}