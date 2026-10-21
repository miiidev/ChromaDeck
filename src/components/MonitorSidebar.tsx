import { useState } from "react";
import type { Monitor, Preset } from "../lib/types";
import { identifyMonitors, resetMonitor, setMonitorName, unpinMonitor, probeMonitor } from "../lib/tauri";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import WindowDots from "@/components/ui/window-dots";
import { MonitorCheck, Pencil, Stethoscope } from "lucide-react";
import { cn } from "cn";

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
        <span className={`text-sm font-medium truncate ${monitor.connected ? "text-foreground" : "text-muted-foreground"}`}>{displayName}</span>
        <Button variant="ghost" size="icon-xs" onClick={() => { setDraft(monitor.alias); setEditing(true); }} title="Rename monitor" aria-label={`Rename monitor ${displayName}`}>
          <Pencil className="size-3.5" />
        </Button>
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5">
      <Input
        type="text"
        value={draft}
        autoFocus
        maxLength={64}
        placeholder={monitor.model}
        aria-label="Monitor name"
        onChange={(e) => setDraft((e.target as HTMLInputElement).value)}
        onKeyDown={(e) => { if (e.key === "Enter") void save(); else if (e.key === "Escape") setEditing(false); }}
        disabled={saving}
        className="h-8 w-32 px-2 text-sm"
      />
      <Button variant="secondary" size="icon-xs" onClick={() => void save()} disabled={saving}>
        {saving ? "…" : "Save"}
      </Button>
      <Button variant="ghost" size="icon-xs" onClick={() => setEditing(false)} disabled={saving}>
        Cancel
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
        const result = await resetMonitor(edidId);
        if (result.error) {
          // Keep the pin: a failed restore (e.g. Intel with no captured LUT)
          // must not silently drop the user's enforcement intent.
          setMessage({ text: `Reset failed: ${result.error}`, ok: false });
          setTimeout(() => setMessage(null), 6000);
        } else {
          // Unpin only after the restore actually succeeded.
          try {
            await unpinMonitor(edidId);
          } catch {
            // best-effort: the reset itself already succeeded
          }
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
        <span key={message.text} aria-live="polite" className={`transient-enter text-xs font-bold ${message.ok ? "text-foreground" : "text-danger-ink"}`}>
          {message.text}
        </span>
      )}
      <Button
        variant="outline"
        size="icon-xs"
        onClick={handleReset}
        disabled={resetting}
        title="Reset this monitor to default colours (identity gamma)"
      >
        {resetting ? "Resetting…" : "Reset"}
      </Button>
    </span>
  );
}

/** Monitor hardware probe button: read live state, change nothing */
function MonitorProbeButton({ edidId }: { edidId: string }) {
  const [probing, setProbing] = useState(false);
  const [report, setReport] = useState<string | null>(null);

  const handleProbe = async () => {
    setProbing(true);
    setReport(null);
    try {
      const p = await probeMonitor(edidId);
      const vib =
        p.vibrance_supported && p.vibrance !== null && p.vibrance !== undefined
          ? `vib ${p.vibrance} hue ${p.hue ?? "?"}`
          : "vib n/a";
      // Intel's colour lives in the pixtx LUT, not the OS ramp.
      const rampLabel = p.backend_id === "intel" ? "lut" : "ramp";
      const ramp =
        p.ramp_peak !== null && p.ramp_peak !== undefined
          ? `${rampLabel} ${p.ramp_peak.toFixed(2)}${p.ramp_identity ? "" : " !"}`
          : `${rampLabel} n/a`;
      setReport(`${p.backend_id ?? p.engine} · ${vib} · ${ramp}`);
      setTimeout(() => setReport(null), 8000);
    } catch (err) {
      setReport(`Probe failed: ${String(err)}`);
      setTimeout(() => setReport(null), 6000);
    } finally {
      setProbing(false);
    }
  };

  return (
    <span className="inline-flex items-center gap-2">
      {report && (
        <span key={report} aria-live="polite" className="text-xs mono text-muted-foreground">{report}</span>
      )}
      <Button variant="ghost" size="icon-xs" onClick={() => void handleProbe()} disabled={probing} title="Read live hardware state for this monitor (changes nothing)">
        {probing ? "…" : <Stethoscope className="size-3.5" />}
      </Button>
    </span>
  );
}

export default function MonitorSidebar({ monitors, presets, pins, onRefresh, onPinChange, onApplyFor }: {
  monitors: Monitor[];
  presets: Preset[];
  pins: Record<string, string>;
  onRefresh: () => void;
  onPinChange: () => void;
  onApplyFor: (preset: Preset, edidId: string) => void;
}) {
  const [identifying, setIdentifying] = useState(false);

  if (monitors.length === 0) return null;

  const handleIdentify = async () => {
    setIdentifying(true);
    try {
      await identifyMonitors();
    } catch {
      // silent
    } finally {
      setIdentifying(false);
    }
  };

  return (
    <aside className="w-full shrink-0 rounded-card border border-border bg-surface text-foreground shell-enter" style={{ ["--shell-delay" as string]: "60ms" }}>
      <div className="flex items-center gap-2 px-3 py-2 border-b border-border">
        <WindowDots />
        <span className="text-xs font-bold text-foreground uppercase tracking-[0.12em] mono">
          Monitors ({monitors.length})
        </span>
        <span className="flex-1" />
        <Button
          variant="outline"
          size="sm"
          onClick={() => void handleIdentify()}
          disabled={identifying}
          title="Show monitor numbers on each display"
        >
          <MonitorCheck className="size-3.5 stroke-[1.75]" />
          {identifying ? "…" : "Identify"}
        </Button>
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 p-3">
        {monitors.map((m, index) => {
          const pinnedId = pins[m.edid_id];
          const pinnedPreset = pinnedId ? presets.find((p) => p.id === pinnedId) : null;
          const isPinned = !!pinnedPreset;
          return (
            <div
              key={m.edid_id}
              className={cn(
                "flex flex-col gap-2 rounded-brutal p-3 border border-border bg-surface transition-colors",
                isPinned ? "border-primary" : "",
                m.connected ? "" : "border-dashed bg-surface/50"
              )}
              style={{ "--stagger-ms": `${Math.min(index * 60, 180)}ms` } as Record<string, string>}
            >
              <div className="flex items-center gap-2 text-foreground">
                <span
                  className={cn(
                    "size-2.5 rounded-full flex-shrink-0",
                    m.connected ? "bg-primary" : "bg-muted-foreground"
                  )}
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
                  <span className="inline-flex items-center gap-1">
                    <span className="size-2 rounded-md bg-primary" title={`Pinned: ${pinnedPreset.name}`} />
                    <Badge variant="pinned" title={`Pinned: ${pinnedPreset.name}`}>
                      PINNED: {pinnedPreset.name}
                    </Badge>
                  </span>
                )}
              </div>
              <div className="flex flex-wrap items-center gap-1">
                {pinnedPreset && m.connected && (
                  <Button variant="default" size="icon-xs" onClick={() => onApplyFor(pinnedPreset, m.edid_id)} title={`Apply ${pinnedPreset.name} to ${m.alias || m.model}`}>
                    Apply
                  </Button>
                )}
                {pinnedId && (
                  <Button variant="outline" size="icon-xs" onClick={async () => { try { await unpinMonitor(m.edid_id); onPinChange(); } catch { /* silent */ } }} title="Unpin from this monitor">
                    Unpin
                  </Button>
                )}
                <MonitorResetButton edidId={m.edid_id} onPinChange={onPinChange} />
                <MonitorProbeButton edidId={m.edid_id} />
              </div>
            </div>
          );
        })}
      </div>
    </aside>
  );
}