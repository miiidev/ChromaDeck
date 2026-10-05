import { useState } from "react";
import type { Monitor, Preset } from "../lib/types";
import { resetMonitor, setMonitorName, unpinMonitor } from "../lib/tauri";

interface Props {
  monitors: Monitor[];
  presets: Preset[];
  pins: Record<string, string>;
  onRefresh: () => void;
  onPinChange: () => void;
  onApplyFor: (preset: Preset, edidId: string) => void;
}

/** Compact inline monitor rename */
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
      <span className="inline-flex items-center gap-1.5 min-w-0">
        <span className="text-xs font-medium text-ink truncate">{displayName}</span>
        <button
          onClick={() => { setDraft(monitor.alias); setEditing(true); }}
          className="bauhaus-btn px-1.5 py-0.5 text-xs border border-ink bg-surface text-secondary hover:bg-surface-hover"
          title="Rename monitor"
          aria-label={`Rename monitor ${displayName}`}
        >
          ✎
        </button>
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5">
      <input
        type="text"
        value={draft}
        autoFocus
        maxLength={64}
        placeholder={monitor.model}
        aria-label="Monitor name"
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") void save(); else if (e.key === "Escape") setEditing(false); }}
        disabled={saving}
        className="px-2 py-1 text-xs border-2 border-ink bg-surface text-ink placeholder-muted focus-visible:outline-2 focus-visible:outline-blue focus-visible:outline-offset-2 disabled:opacity-50 w-28"
      />
      <button
        onClick={() => void save()}
        disabled={saving}
        className="bauhaus-btn px-2 py-0.5 text-xs font-medium border-2 border-blue bg-primary-blue disabled:opacity-50"
        style={{ color: "white" }}
      >
        {saving ? "…" : "SAVE"}
      </button>
      <button
        onClick={() => setEditing(false)}
        disabled={saving}
        className="bauhaus-btn px-2 py-0.5 text-xs font-medium border-2 border-ink bg-surface text-secondary hover:text-ink disabled:opacity-50"
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
        <span aria-live="polite" className={`text-xs ${message.ok ? "text-blue" : "text-red"}`}>
          {message.text}
        </span>
      )}
      <button
        onClick={handleReset}
        disabled={resetting}
        className={`bauhaus-btn px-2 py-0.5 text-[11px] font-medium border border-ink bg-surface text-secondary hover:bg-surface-hover ${
          resetting ? "cursor-wait opacity-70" : ""
        }`}
        title="Reset this monitor to default colours (identity gamma)"
      >
        {resetting ? "RESETTING…" : "RESET"}
      </button>
    </span>
  );
}

export default function MonitorSidebar({ monitors, presets, pins, onRefresh, onPinChange, onApplyFor }: Props) {
  const [collapsed, setCollapsed] = useState<boolean>(() => {
    try {
      return localStorage.getItem("chromadeck.monitors-collapsed") === "1";
    } catch {
      return false;
    }
  });
  const toggleCollapsed = () => {
    setCollapsed((prev) => {
      try {
        localStorage.setItem("chromadeck.monitors-collapsed", prev ? "0" : "1");
      } catch {
        // ignore storage failures
      }
      return !prev;
    });
  };

  if (monitors.length === 0) return null;

  return (
    <aside className={`${collapsed ? "w-full md:w-12" : "w-full md:w-72"} shrink-0 border-2 border-ink bg-surface self-start`}>
      <div className="flex items-center justify-between px-3 py-2 border-b-2 border-ink">
        {!collapsed && (
          <span className="text-[11px] font-medium text-secondary uppercase tracking-widest">
            Monitors ({monitors.length})
          </span>
        )}
        <button
          onClick={toggleCollapsed}
          className="bauhaus-btn px-2 py-0.5 text-xs border border-ink bg-surface text-secondary hover:bg-surface-hover"
          title={collapsed ? "Expand monitors" : "Collapse monitors"}
          aria-label={collapsed ? "Expand monitor sidebar" : "Collapse monitor sidebar"}
          aria-expanded={!collapsed}
        >
          {collapsed ? "»" : "«"}
        </button>
      </div>
      {!collapsed && (
        <div className="space-y-2 p-2 max-h-96 overflow-y-auto">
          {monitors.map((m) => {
            const pinnedId = pins[m.edid_id];
            const pinnedPreset = pinnedId ? presets.find((p) => p.id === pinnedId) : null;
            return (
              <div
                key={m.edid_id}
                className="flex flex-col gap-1.5 border border-ink bg-surface p-2"
              >
                <div className="flex items-center gap-2">
                  {/* Status dot: ● connected (blue), ■ pinned (red), ▲ warning (yellow) */}
                  <span
                    className={`status-dot ${m.connected ? "status-dot-connected" : "status-dot-warning"}`}
                    title={m.connected ? "Connected" : "Offline"}
                  />
                  <MonitorNameEditor monitor={m} onRefreshParent={onRefresh} />
                </div>
                {m.serial && (
                  <span className="text-[10px] text-muted font-mono" title={m.serial}>{m.serial}</span>
                )}
                <div className="flex flex-wrap items-center gap-1">
                  {!m.connected && (
                    <span className="text-[10px] font-medium text-secondary border border-ink px-1.5 py-0.5 uppercase tracking-widest leading-none">
                      OFFLINE
                    </span>
                  )}
                  {pinnedPreset && (
                    <span className="text-[10px] font-medium text-blue border border-blue px-1.5 py-0.5 uppercase tracking-widest leading-none truncate max-w-full" title={`Pinned: ${pinnedPreset.name}`}>
                      PINNED: {pinnedPreset.name}
                    </span>
                  )}
                </div>
                <div className="flex flex-wrap items-center gap-1">
                  {pinnedPreset && m.connected && (
                    <button
                      onClick={() => onApplyFor(pinnedPreset, m.edid_id)}
                      className="bauhaus-btn px-2 py-0.5 text-[11px] font-medium border border-red bg-primary-red"
                      style={{ color: "white" }}
                      title={`Apply presets to ${m.alias || m.model}`}
                    >
                      APPLY…
                    </button>
                  )}
                  {pinnedId && (
                    <button
                      onClick={async () => {
                        try { await unpinMonitor(m.edid_id); onPinChange(); } catch { /* silent */ }
                      }}
                      className="bauhaus-btn px-2 py-0.5 text-[11px] font-medium border border-ink bg-surface text-secondary hover:bg-surface-hover"
                      title="Unpin from this monitor"
                    >
                      UNPIN
                    </button>
                  )}
                  <MonitorResetButton edidId={m.edid_id} onPinChange={onPinChange} />
                </div>
              </div>
            );
          })}
        </div>
      )}
      {collapsed && (
        <div className="flex md:flex-col flex-row items-center gap-2 p-2 flex-wrap">
          {monitors.map((m) => (
            <span
              key={m.edid_id}
              className={`status-dot ${m.connected ? "status-dot-connected" : "status-dot-warning"} ${pins[m.edid_id] ? "outline outline-1 outline-blue outline-offset-1" : ""}`}
              title={`${m.alias || m.model || m.device_name}${pins[m.edid_id] ? " (pinned)" : ""}`}
            />
          ))}
        </div>
      )}
    </aside>
  );
}