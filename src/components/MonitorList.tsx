import { useState } from "react";
import type { Monitor, Preset } from "../lib/types";
import { resetMonitor, setMonitorName, unpinMonitor } from "../lib/tauri";
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
}

/** Loading skeleton rows */
function SkeletonRow() {
  return (
    <div className="animate-pulse rounded-lg border border-neutral-800 bg-neutral-900/50 p-4 space-y-3">
      <div className="h-4 bg-neutral-800 rounded w-1/3" />
      <div className="h-3 bg-neutral-800 rounded w-1/2" />
    </div>
  );
}

/** Empty state when no monitors or presets exist */
function EmptyState({ onCreateNew }: { onCreateNew: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center space-y-4">
      <div className="w-16 h-16 rounded-full bg-neutral-900 border border-neutral-800 flex items-center justify-center">
        <svg className="w-8 h-8 text-neutral-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 12V5.25" />
        </svg>
      </div>
      <h3 className="text-sm font-medium text-neutral-400">No presets yet</h3>
      <p className="text-xs text-neutral-600 max-w-xs">
        Create your first colour preset to apply an ICC profile and gamma adjustment to a monitor.
      </p>
      <button
        onClick={onCreateNew}
        className="px-4 py-2 text-sm font-medium rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white transition-colors"
      >
        Create Preset
      </button>
    </div>
  );
}

/** Inline monitor rename (pencil toggle in the group header) */
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
          <span className="text-xs text-neutral-600">{monitor.model}</span>
        )}
        <button
          onClick={() => {
            setDraft(monitor.alias);
            setEditing(true);
          }}
          className="px-1 py-0.5 text-xs rounded text-neutral-600 hover:text-neutral-200 hover:bg-neutral-700 transition-colors"
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
        className="px-2 py-1 text-sm rounded-md border border-neutral-700 bg-neutral-800 text-neutral-200 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 disabled:opacity-50"
      />
      <button
        onClick={() => void save()}
        disabled={saving}
        className="px-2 py-1 text-xs font-medium rounded-md bg-indigo-600 hover:bg-indigo-500 text-white transition-colors disabled:opacity-50"
      >
        {saving ? "…" : "Save"}
      </button>
      <button
        onClick={() => setEditing(false)}
        disabled={saving}
        className="px-2 py-1 text-xs font-medium rounded-md text-neutral-500 hover:text-neutral-300"
      >
        Cancel
      </button>
    </span>
  );
}

/** Per-monitor reset-to-default button with inline result feedback */
function MonitorResetButton({ edidId, onPinChange }: { edidId: string; onPinChange: () => void }) {  const [resetting, setResetting] = useState(false);
  const [message, setMessage] = useState<{ text: string; ok: boolean } | null>(null);

  const handleReset = async () => {
    setResetting(true);
    setMessage(null);
    try {
      try {
        await unpinMonitor(edidId); // no-op when unpinned; Reset always disarms
      } catch {
        // ignore — reset proceeds regardless
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
        className={`px-2 py-1 text-xs font-medium rounded-md transition-colors ${
          resetting
            ? "text-neutral-500 cursor-wait"
            : "text-neutral-500 hover:text-neutral-200 hover:bg-neutral-700"
        }`}
        title="Reset this monitor to default colours (identity gamma)"
      >
        {resetting ? "Resetting…" : "Reset"}
      </button>
    </span>
  );
}

export default function MonitorList({ monitors, presets, loading, onEdit, onRefresh, onCreateNew, pins, onPinChange }: Props) {
  if (loading) {
    return (
      <section className="flex-1 p-6 space-y-4">
        <div className="h-6 bg-neutral-800 rounded w-20 mb-6 animate-pulse" />
        <SkeletonRow />
        <SkeletonRow />
      </section>
    );
  }

  // Build a complete monitor map from monitors list + any edid_ids referenced by presets
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

  // Group presets by edid_id
  const grouped = new Map<string, Preset[]>();
  for (const p of presets) {
    if (!grouped.has(p.edid_id)) grouped.set(p.edid_id, []);
    grouped.get(p.edid_id)!.push(p);
  }

  // Sort monitors: connected first, then by model name
  const sortedMonitors = Array.from(monitorMap.values()).sort((a, b) => {
    if (a.connected !== b.connected) return a.connected ? -1 : 1;
    return a.model.localeCompare(b.model);
  });

  // Filter to only show monitors that either are connected or have presets
  const visibleMonitors = sortedMonitors.filter(
    (m) => m.connected || (grouped.get(m.edid_id)?.length ?? 0) > 0,
  );

  if (visibleMonitors.length === 0 && presets.length === 0) {
    return (
      <section className="flex-1 p-6 space-y-6">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-medium text-neutral-300">Library</h2>
          <button
            onClick={onCreateNew}
            className="px-4 py-2 text-sm font-medium rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white transition-colors"
          >
            Create Preset
          </button>
        </div>
        <EmptyState onCreateNew={onCreateNew} />
      </section>
    );
  }

  return (
    <section className="flex-1 p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-medium text-neutral-300">Library</h2>
        <div className="flex items-center gap-3">
          <button
            onClick={onRefresh}
            className="px-3 py-1.5 text-xs font-medium rounded-lg bg-neutral-800 hover:bg-neutral-700 text-neutral-300 transition-colors"
            title="Refresh monitors and presets"
          >
            Refresh
          </button>
          <button
            onClick={onCreateNew}
            className="px-4 py-1.5 text-sm font-medium rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white transition-colors"
          >
            + Create
          </button>
        </div>
      </div>

      {visibleMonitors.map((monitor) => {
        const monitorPresets = grouped.get(monitor.edid_id) || [];
        return (
          <div
            key={monitor.edid_id}
            className="rounded-lg border border-neutral-800 bg-neutral-900/50 p-4"
          >
            {/* Monitor header */}
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-3">
                <MonitorNameEditor monitor={monitor} onRefreshParent={onRefresh} />
                {monitor.serial && (
                  <span className="text-xs text-neutral-600 font-mono">{monitor.serial}</span>
                )}
              </div>
              {monitor.connected ? (
                <div className="flex items-center gap-2">
                  <MonitorResetButton edidId={monitor.edid_id} onPinChange={onPinChange} />
                  <span className="inline-flex items-center gap-1.5 text-xs text-emerald-400">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
                    Connected
                  </span>
                </div>
              ) : (
                <span className="inline-flex items-center gap-1.5 text-xs text-neutral-500">
                  <span className="w-1.5 h-1.5 rounded-full bg-neutral-600" />
                  Offline — kept
                </span>
              )}
            </div>

            {/* Presets for this monitor */}
            {monitorPresets.length === 0 ? (
              <p className="text-xs text-neutral-500 italic">
                No presets yet. Click Create to add one.
              </p>
            ) : (
              <div className="space-y-2">
                {monitorPresets.map((preset) => (
                  <PresetCard
                    key={preset.id}
                    preset={preset}
                    monitor={monitor}
                    onEdit={onEdit}
                    onRefreshParent={onRefresh}
                    isPinned={pins[monitor.edid_id] === preset.id}
                    onPinChange={onPinChange}
                  />
                ))}
              </div>
            )}
          </div>
        );
      })}
    </section>
  );
}