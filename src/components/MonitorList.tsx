import { useState, useRef, useEffect, useCallback } from "react";
import type { Monitor, Preset } from "../lib/types";
import { resetMonitor, setMonitorName, unpinMonitor } from "../lib/tauri";
import PresetCard from "./PresetCard";

interface Props {
  monitors: Monitor[];
  presets: Preset[];
  loading: boolean;
  onEdit: (preset: Preset) => void;
  onRefresh: () => void;
  onCreateNew: (edidId: string) => void;
  pins: Record<string, string>;
  onPinChange: () => void;
}

/** Loading skeleton */
function SkeletonRow() {
  return (
    <div className="animate-pulse border-2 border-neutral-700 bg-neutral-900 p-4 space-y-3">
      <div className="h-4 bg-neutral-800 w-1/3" />
      <div className="h-3 bg-neutral-800 w-1/2" />
    </div>
  );
}

/** Full empty state */
function EmptyState({ onCreateNew, monitors }: { onCreateNew: (edidId: string) => void; monitors: Monitor[] }) {
  const firstConnected = monitors.find((m) => m.connected);
  const targetId = firstConnected?.edid_id ?? "";
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center space-y-4">
      <div className="border-2 border-neutral-600 bg-neutral-900 p-5">
        <svg className="w-10 h-10 text-neutral-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 12V5.25" />
        </svg>
      </div>
      <h3 className="text-sm font-medium text-neutral-400 uppercase tracking-widest">No presets yet</h3>
      <p className="text-xs text-neutral-600 max-w-xs">
        Create your first colour preset to apply an ICC profile and gamma adjustment to a monitor.
      </p>
      <button
        onClick={() => onCreateNew(targetId)}
        className="brutalist-btn px-4 py-2 text-sm font-medium border-2 border-lime-400 shadow-[2px_2px_0px_#a3e635] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-lime-400 text-black hover:bg-lime-300"
      >
        CREATE PRESET
      </button>
    </div>
  );
}

/** Monitor name editor (inline) */
function MonitorNameEditor({ monitor, onRefreshParent }: { monitor: Monitor; onRefreshParent: () => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(monitor.alias);
  const [saving, setSaving] = useState(false);

  const displayName = monitor.alias || monitor.model || monitor.device_name || monitor.edid_id.slice(0, 16);

  const save = async () => {
    setSaving(true);
    try {
      await setMonitorName(monitor.edid_id, draft);
      setEditing(false);
      onRefreshParent();
    } finally {
      setSaving(false);
    }
  };

  if (!editing) {
    return (
      <span className="inline-flex items-center gap-2">
        <h3 className="text-sm font-medium text-neutral-200">{displayName}</h3>
        {monitor.alias && (
          <span className="text-xs text-neutral-500">{monitor.model}</span>
        )}
        <button
          onClick={() => {
            setDraft(monitor.alias);
            setEditing(true);
          }}
          className="brutalist-btn px-1.5 py-0.5 text-xs border border-neutral-600 bg-neutral-800 text-neutral-500 hover:bg-neutral-700 hover:text-neutral-200"
          title="Rename monitor"
        >
          ✎
        </button>
      </span>
    );
  }

  return (
    <span className="inline-flex items-center gap-2">
      <input
        type="text"
        value={draft}
        autoFocus
        maxLength={64}
        placeholder={monitor.model}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") void save();
          else if (e.key === "Escape") setEditing(false);
        }}
        disabled={saving}
        className="px-2 py-1 text-sm border-2 border-neutral-200 bg-neutral-800 text-neutral-200 placeholder-neutral-500 focus-visible:outline-2 focus-visible:outline-lime-400 focus-visible:outline-offset-2 disabled:opacity-50"
      />
      <button
        onClick={() => void save()}
        disabled={saving}
        className="brutalist-btn px-2 py-1 text-xs font-medium border-2 border-lime-400 shadow-[2px_2px_0px_#a3e635] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-lime-400 text-black hover:bg-lime-300 disabled:opacity-50"
      >
        {saving ? "…" : "SAVE"}
      </button>
      <button
        onClick={() => setEditing(false)}
        disabled={saving}
        className="brutalist-btn px-2 py-1 text-xs font-medium border-2 border-neutral-200 bg-neutral-800 text-neutral-400 hover:text-neutral-200 disabled:opacity-50"
      >
        CANCEL
      </button>
    </span>
  );
}

/** Monitor reset button */
function MonitorResetButton({ edidId, onPinChange }: { edidId: string; onPinChange: () => void }) {
  const [resetting, setResetting] = useState(false);
  const [message, setMessage] = useState<{ text: string; ok: boolean } | null>(null);

  const handleReset = async () => {
    setResetting(true);
    setMessage(null);
    try {
      try {
        await unpinMonitor(edidId);
      } catch {
        // ignore
      }
      try {
        const result = await resetMonitor(edidId);
        if (result.error) {
          setMessage({ text: `Reset failed: ${result.error}`, ok: false });
          setTimeout(() => setMessage(null), 6000);
        } else {
          setMessage({ text: "Reset to default", ok: true });
          setTimeout(() => setMessage(null), 3000);
        }
      } catch (err) {
        setMessage({ text: `Reset failed: ${String(err)}`, ok: false });
        setTimeout(() => setMessage(null), 6000);
      }
    } finally {
      onPinChange();
      setResetting(false);
    }
  };

  return (
    <span className="inline-flex items-center gap-2">
      {message && (
        <span className={`text-xs ${message.ok ? "text-emerald-400" : "text-red-400"}`}>
          {message.text}
        </span>
      )}
      <button
        onClick={handleReset}
        disabled={resetting}
        className={`brutalist-btn px-2 py-1 text-xs font-medium border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 ${
          resetting
            ? "bg-neutral-700 text-neutral-500 cursor-wait"
            : "bg-neutral-800 text-neutral-400 hover:bg-neutral-700 hover:text-neutral-200"
        }`}
        title="Reset this monitor to default colours (identity gamma)"
      >
        {resetting ? "RESETTING…" : "RESET"}
      </button>
    </span>
  );
}

export default function MonitorList({ monitors, presets, loading, onEdit, onRefresh, onCreateNew, pins, onPinChange }: Props) {
  const carouselRef = useRef<HTMLDivElement>(null);

  // ── Derived data (runs every render, cheap — needed by hooks below) ──
  const monitorMap = new Map<string, Monitor>();
  for (const m of monitors) monitorMap.set(m.edid_id, m);
  for (const p of presets) {
    if (!monitorMap.has(p.edid_id)) {
      monitorMap.set(p.edid_id, {
        edid_id: p.edid_id,
        model: "Unknown Monitor",
        serial: "",
        connected: false,
        device_name: "",
        alias: "",
      });
    }
  }

  const grouped = new Map<string, Preset[]>();
  for (const p of presets) {
    if (!grouped.has(p.edid_id)) grouped.set(p.edid_id, []);
    grouped.get(p.edid_id)!.push(p);
  }

  const sortedMonitors = Array.from(monitorMap.values()).sort((a, b) => {
    if (a.connected !== b.connected) return a.connected ? -1 : 1;
    return a.model.localeCompare(b.model);
  });

  const visibleMonitors = sortedMonitors.filter(
    (m) => m.connected || (grouped.get(m.edid_id)?.length ?? 0) > 0,
  );

  // ── Hooks (unconditional — must be before any early return) ──────────
  const [selectedId, setSelectedId] = useState<string>(() => {
    const firstConnected = visibleMonitors.find((m) => m.connected);
    return firstConnected?.edid_id ?? visibleMonitors[0]?.edid_id ?? "";
  });

  // Sync selected when monitors change and current selection no longer valid
  useEffect(() => {
    if (!selectedId || !visibleMonitors.some((m) => m.edid_id === selectedId)) {
      const firstConnected = visibleMonitors.find((m) => m.connected);
      setSelectedId(firstConnected?.edid_id ?? visibleMonitors[0]?.edid_id ?? "");
    }
  }, [monitors, presets]); // eslint-disable-line react-hooks/exhaustive-deps

  const scrollCarousel = useCallback((dir: "left" | "right") => {
    if (!carouselRef.current) return;
    const amount = dir === "left" ? -300 : 300;
    carouselRef.current.scrollBy({ left: amount, behavior: "smooth" });
  }, []);

  const selectedMonitor = monitorMap.get(selectedId) ?? visibleMonitors[0] ?? null;
  const selectedPresets = selectedId ? grouped.get(selectedId) ?? [] : [];
  const pinnedId = selectedId ? pins[selectedId] : undefined;

  // ── Early return when loading ────────────────────────────────────────
  if (loading) {
    return (
      <section className="flex-1 p-6 space-y-4">
        <div className="h-6 bg-neutral-800 w-20 mb-6 animate-pulse" />
        <SkeletonRow />
        <SkeletonRow />
      </section>
    );
  }

  // Full empty state
  if (visibleMonitors.length === 0 && presets.length === 0) {
    return (
      <section className="flex-1 p-6 space-y-6">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-medium text-neutral-400 uppercase tracking-widest">Library</h2>
          <button
            onClick={() => onCreateNew("")}
            className="brutalist-btn px-4 py-2 text-sm font-medium border-2 border-lime-400 shadow-[2px_2px_0px_#a3e635] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-lime-400 text-black hover:bg-lime-300"
          >
            + CREATE
          </button>
        </div>
        <EmptyState onCreateNew={onCreateNew} monitors={monitors} />
      </section>
    );
  }

  return (
    <section className="flex-1 p-6 space-y-5">
      {/* ── Monitor selector pills ──────────────────────────────────────── */}
      <div className="flex items-center gap-2 overflow-x-auto carousel-scroll pb-1">
        {visibleMonitors.map((m) => {
          const count = grouped.get(m.edid_id)?.length ?? 0;
          const isSelected = m.edid_id === selectedId;
          return (
            <button
              key={m.edid_id}
              onClick={() => setSelectedId(m.edid_id)}
              className={`brutalist-btn shrink-0 flex items-center gap-2 px-3 py-1.5 text-xs font-medium border-2 active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 ${
                isSelected
                  ? "border-lime-400 bg-lime-400 text-black shadow-[2px_2px_0px_#a3e635]"
                  : "border-neutral-200 bg-neutral-900 text-neutral-400 shadow-[2px_2px_0px_#e5e7eb] hover:bg-neutral-800"
              }`}
            >
              <span className={`w-2 h-2 ${m.connected ? "bg-emerald-400" : "bg-neutral-600"}`} />
              <span className="truncate max-w-[120px]">{m.alias || m.model || m.device_name}</span>
              <span className="font-mono text-[10px] opacity-70">{count}</span>
            </button>
          );
        })}
      </div>

      {/* ── Selected monitor header block ───────────────────────────────── */}
      {selectedMonitor && (
        <div className="border-2 border-neutral-200 bg-neutral-900 p-4">
          <div className="flex items-center justify-between flex-wrap gap-2">
            <div className="flex items-center gap-3 flex-wrap">
              <MonitorNameEditor monitor={selectedMonitor} onRefreshParent={onRefresh} />
              {selectedMonitor.serial && (
                <span className="text-xs text-neutral-500 font-mono">{selectedMonitor.serial}</span>
              )}
              <MonitorResetButton edidId={selectedMonitor.edid_id} onPinChange={onPinChange} />
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => onCreateNew(selectedMonitor.edid_id)}
                className="brutalist-btn px-3 py-1 text-xs font-medium border-2 border-lime-400 shadow-[2px_2px_0px_#a3e635] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-lime-400 text-black hover:bg-lime-300"
                title="Create a preset for this monitor"
              >
                + CREATE
              </button>
              {selectedMonitor.connected ? (
                <span className="inline-flex items-center gap-1.5 text-[10px] font-medium text-emerald-400 border-2 border-emerald-400 px-2 py-0.5 uppercase tracking-widest">
                  <span className="w-1.5 h-1.5 bg-emerald-400" />
                  CONNECTED
                </span>
              ) : (
                <span className="inline-flex items-center gap-1.5 text-[10px] font-medium text-neutral-500 border-2 border-neutral-600 px-2 py-0.5 uppercase tracking-widest">
                  <span className="w-1.5 h-1.5 bg-neutral-600" />
                  OFFLINE
                </span>
              )}
              {pinnedId && (
                <span className="text-[10px] font-medium text-lime-400 border-2 border-lime-400 px-2 py-0.5 uppercase tracking-widest">
                  ENFORCED
                </span>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ── Preset carousel ─────────────────────────────────────────────── */}
      {selectedPresets.length === 0 ? (
        <div className="flex flex-col items-center justify-center flex-1 min-h-64 py-12 text-center space-y-3 border-2 border-dashed border-neutral-700 bg-neutral-900/50">
          <p className="text-xs text-neutral-500">No presets for this monitor</p>
          <button
            onClick={() => onCreateNew(selectedId)}
            className="brutalist-btn px-4 py-2 text-sm font-medium border-2 border-lime-400 shadow-[2px_2px_0px_#a3e635] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-lime-400 text-black hover:bg-lime-300"
          >
            CREATE PRESET
          </button>
        </div>
      ) : (
        <div className="relative">
          {/* Scroll arrows */}
          <button
            onClick={() => scrollCarousel("left")}
            className="brutalist-btn absolute left-0 top-1/2 -translate-y-1/2 -translate-x-3 z-10 w-8 h-8 flex items-center justify-center border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-neutral-900 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
            aria-label="Previous presets"
          >
            ‹
          </button>
          <button
            onClick={() => scrollCarousel("right")}
            className="brutalist-btn absolute right-0 top-1/2 -translate-y-1/2 translate-x-3 z-10 w-8 h-8 flex items-center justify-center border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-neutral-900 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
            aria-label="Next presets"
          >
            ›
          </button>

          {/* Carousel track */}
          <div
            ref={carouselRef}
            className="flex gap-4 overflow-x-auto snap-x snap-mandatory carousel-scroll px-4 py-1"
          >
            {selectedPresets.map((preset) => (
              <div key={preset.id} className="snap-start shrink-0">
                <PresetCard
                  preset={preset}
                  monitor={selectedMonitor!}
                  onEdit={onEdit}
                  onRefreshParent={onRefresh}
                  isPinned={pins[preset.edid_id] === preset.id}
                  onPinChange={onPinChange}
                />
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}