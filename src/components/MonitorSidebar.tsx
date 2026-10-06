import { useState } from "react";
import type { Monitor, Preset } from "../lib/types";
import { resetMonitor, setMonitorName, unpinMonitor } from "../lib/tauri";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

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
        <span className="text-xs font-medium text-foreground truncate">{displayName}</span>
        <Button variant="ghost" size="xs" onClick={() => { setDraft(monitor.alias); setEditing(true); }} title="Rename monitor" aria-label={`Rename monitor ${displayName}`}>
          ✎
        </Button>
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
        className="h-7 w-28 rounded-lg border border-input bg-transparent px-2 py-1 text-xs placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:opacity-50"
      />
      <Button variant="secondary" size="xs" onClick={() => void save()} disabled={saving}>
        {saving ? "…" : "SAVE"}
      </Button>
      <Button variant="ghost" size="xs" onClick={() => setEditing(false)} disabled={saving}>
        CANCEL
      </Button>
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
        <span aria-live="polite" className={`text-xs ${message.ok ? "text-primary" : "text-destructive"}`}>
          {message.text}
        </span>
      )}
      <Button
        variant="outline"
        size="xs"
        onClick={handleReset}
        disabled={resetting}
        title="Reset this monitor to default colours (identity gamma)"
      >
        {resetting ? "RESETTING…" : "RESET"}
      </Button>
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
    <aside className={`${collapsed ? "w-full md:w-12" : "w-full md:w-72"} shrink-0 rounded-lg bg-card text-card-foreground ring-1 ring-foreground/10 self-start`}>
      <div className="flex items-center justify-between px-3 py-2 border-b border-border">
        {!collapsed && (
          <span className="text-xs font-medium text-muted-foreground uppercase tracking-widest mono">
            Monitors ({monitors.length})
          </span>
        )}
        <Button
          variant="ghost"
          size="xs"
          onClick={toggleCollapsed}
          title={collapsed ? "Expand monitors" : "Collapse monitors"}
          aria-label={collapsed ? "Expand monitor sidebar" : "Collapse monitor sidebar"}
          aria-expanded={!collapsed}
        >
          {collapsed ? "»" : "«"}
        </Button>
      </div>
      {!collapsed && (
        <div className="space-y-2 p-2 max-h-96 overflow-y-auto">
          {monitors.map((m) => {
            const pinnedId = pins[m.edid_id];
            const pinnedPreset = pinnedId ? presets.find((p) => p.id === pinnedId) : null;
            return (
              <div
                key={m.edid_id}
                className="flex flex-col gap-1.5 rounded-lg border border-border bg-muted p-2"
              >
                <div className="flex items-center gap-2 text-foreground">
                  <span
                    className={`status-dot ${m.connected ? "status-dot-connected" : "status-dot-warning"}`}
                    title={m.connected ? "Connected" : "Offline"}
                  />
                  <MonitorNameEditor monitor={m} onRefreshParent={onRefresh} />
                </div>
                {m.serial && (
                  <span className="text-xs text-muted-foreground mono" title={m.serial}>{m.serial}</span>
                )}
                <div className="flex flex-wrap items-center gap-1">
                  {!m.connected && (
                    <Badge variant="outline">
                      OFFLINE
                    </Badge>
                  )}
                  {pinnedPreset && (
                    <Badge variant="outline" className="text-accent border-accent" title={`Pinned: ${pinnedPreset.name}`}>
                      PINNED: {pinnedPreset.name}
                    </Badge>
                  )}
                </div>
                <div className="flex flex-wrap items-center gap-1">
                  {pinnedPreset && m.connected && (
                    <Button variant="default" size="xs" onClick={() => onApplyFor(pinnedPreset, m.edid_id)} title={`Apply presets to ${m.alias || m.model}`}>
                      APPLY…
                    </Button>
                  )}
                  {pinnedId && (
                    <Button variant="outline" size="xs" onClick={async () => { try { await unpinMonitor(m.edid_id); onPinChange(); } catch { /* silent */ } }} title="Unpin from this monitor">
                      UNPIN
                    </Button>
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
              className={`status-dot ${m.connected ? "status-dot-connected" : "status-dot-warning"}`}
              title={`${m.alias || m.model || m.device_name}${pins[m.edid_id] ? " (pinned)" : ""}`}
            />
          ))}
        </div>
      )}
    </aside>
  );
}