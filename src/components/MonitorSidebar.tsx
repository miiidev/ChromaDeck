import { useState, useRef, useEffect } from "react";
import type { Monitor, Preset } from "../lib/types";
import { identifyMonitors, resetMonitor, setMonitorName, unpinMonitor } from "../lib/tauri";
import { prefersReducedMotion } from "../lib/motion";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import WindowDots from "@/components/ui/window-dots";
import { MonitorCheck, ChevronLeft, ChevronRight } from "lucide-react";

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
        <span className={`text-xs font-medium truncate ${monitor.connected ? "text-foreground" : "text-muted-foreground"}`}>{displayName}</span>
        <Button variant="ghost" size="xs" onClick={() => { setDraft(monitor.alias); setEditing(true); }} title="Rename monitor" aria-label={`Rename monitor ${displayName}`}>
          ✎
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
        className="h-7 w-28 px-2 text-xs"
      />
      <Button variant="secondary" size="xs" onClick={() => void save()} disabled={saving}>
        {saving ? "…" : "Save"}
      </Button>
      <Button variant="ghost" size="xs" onClick={() => setEditing(false)} disabled={saving}>
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
        <span key={message.text} aria-live="polite" className={`transient-enter text-xs font-bold ${message.ok ? "text-ink" : "text-danger-ink"}`}>
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
        {resetting ? "Resetting…" : "Reset"}
      </Button>
    </span>
  );
}

export default function MonitorSidebar({ monitors, presets, pins, onRefresh, onPinChange, onApplyFor }: Props) {
  const [identifying, setIdentifying] = useState(false);
  const trackRef = useRef<HTMLDivElement>(null);
  const [canScroll, setCanScroll] = useState(false);

  // Show carousel arrows only when the track actually overflows.
  useEffect(() => {
    const track = trackRef.current;
    if (!track) {
      setCanScroll(false);
      return;
    }
    const update = () => setCanScroll(track.scrollWidth > track.clientWidth + 1);
    update();
    const ro = new ResizeObserver(update);
    ro.observe(track);
    return () => ro.disconnect();
  }, [monitors.length]);
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

  const scrollTrack = (dir: 1 | -1) => {
    const track = trackRef.current;
    if (!track) return;
    track.scrollBy({
      left: dir * track.clientWidth * 0.8,
      behavior: prefersReducedMotion() ? "auto" : "smooth",
    });
  };

  if (monitors.length === 0) return null;

  return (
    <aside className="w-full shrink-0 rounded-brutal border-2 border-ink bg-white text-ink shadow-brutal-sm shell-enter" style={{ ["--shell-delay" as string]: "60ms" }}>
      <div className="flex items-center gap-2 px-3 py-2 border-b-2 border-ink">
        <WindowDots />
        <span className="text-xs font-bold text-ink uppercase tracking-widest mono">
          Monitors ({monitors.length})
        </span>
        <span className="flex-1" />
        {canScroll && (
          <span className="hidden sm:inline-flex items-center gap-1">
            <Button
              variant="ghost"
              size="xs"
              onClick={() => scrollTrack(-1)}
              title="Scroll monitors left"
              aria-label="Scroll monitors left"
            >
              <ChevronLeft className="size-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="xs"
              onClick={() => scrollTrack(1)}
              title="Scroll monitors right"
              aria-label="Scroll monitors right"
            >
              <ChevronRight className="size-3.5" />
            </Button>
          </span>
        )}
        <Button
          variant="outline"
          size="sm"
          onClick={() => void handleIdentify()}
          disabled={identifying}
          title="Show monitor numbers on each display"
        >
          <MonitorCheck className="size-3.5" />
          {identifying ? "…" : "Identify"}
        </Button>
      </div>
      <div ref={trackRef} className="flex gap-2 overflow-x-auto carousel-scroll snap-x snap-mandatory p-2 sidebar-content-enter">
          {monitors.map((m) => {
            const pinnedId = pins[m.edid_id];
            const pinnedPreset = pinnedId ? presets.find((p) => p.id === pinnedId) : null;
            return (
              <div
                key={m.edid_id}
                className={`flex w-64 shrink-0 snap-start flex-col gap-1.5 rounded-lg border-2 p-2 monitor-row-enter ${
                  m.connected ? "border-ink bg-white" : "border-dashed border-ink bg-paper"
                }`}
                style={{ "--stagger-ms": `${Math.min(monitors.indexOf(m) * 60, 300)}ms` } as Record<string, string>}
              >
                <div className="flex items-center gap-2 text-foreground">
                  <span
                    className={`status-dot ${m.connected ? "status-dot-connected" : "status-dot-offline"}`}
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
                      <span className="status-dot status-dot-pinned" title={`Pinned: ${pinnedPreset.name}`} />
                      <Badge variant="pinned" title={`Pinned: ${pinnedPreset.name}`}>
                        PINNED: {pinnedPreset.name}
                      </Badge>
                    </span>
                  )}
                </div>
                <div className="flex flex-wrap items-center gap-1">
                  {pinnedPreset && m.connected && (
                    <Button variant="default" size="xs" onClick={() => onApplyFor(pinnedPreset, m.edid_id)} title={`Apply presets to ${m.alias || m.model}`}>
                      Apply…
                    </Button>
                  )}
                  {pinnedId && (
                    <Button variant="outline" size="xs" onClick={async () => { try { await unpinMonitor(m.edid_id); onPinChange(); } catch { /* silent */ } }} title="Unpin from this monitor">
                      Unpin
                    </Button>
                  )}
                  <MonitorResetButton edidId={m.edid_id} onPinChange={onPinChange} />
                </div>
              </div>
            );
          })}
        </div>
    </aside>
  );
}