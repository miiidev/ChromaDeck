import { useState, useEffect } from "react";
import type { Monitor, Preset, ApplyResult } from "../lib/types";
import { adjustmentLabel, summarizeApply } from "../lib/applyOutcomes";
import { connectedEdidSignature } from "../lib/connectedSignature";
import { applyPreset, pinPreset, unpinMonitor, getDisplayCapabilities } from "../lib/tauri";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox, CheckboxIndicator } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogHeader, DialogFooter, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupIndicator, RadioGroupItem } from "@/components/ui/radio-group";

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

type BackendState =
  | { status: "loading" }
  | { status: "ready"; label: string; unsupported: string[] }
  | { status: "error" };

export default function ApplyDialog({ preset, monitors, pins, initialEdid, onClose, onApplied }: Props) {
  const [selectedEdid, setSelectedEdid] = useState<string>("");
  const [pinToggle, setPinToggle] = useState(false);
  const [applying, setApplying] = useState(false);
  const [lastResult, setLastResult] = useState<ApplyResult | null>(null);
  const [backends, setBackends] = useState<Record<string, BackendState>>({});

  // Re-probe only when the connected set changes. The 5s monitor poll hands
  // us a new array on every tick (mergeMonitors), so depending on `monitors`
  // reset every badge to "loading" and re-ran every capability call. Keying
  // on a stable signature of connected EDIDs fixes the flicker.
  const connectedKey = connectedEdidSignature(monitors);

  // One capability call per connected display. Failures are contained per
  // row: the dialog still lists and applies to the display as before.
  useEffect(() => {
    let cancelled = false;
    const connected = monitors.filter((m) => m.connected);
    // Preserve already-resolved badges; only seed genuinely new ids.
    setBackends((prev) => {
      const next: Record<string, BackendState> = {};
      for (const m of connected) {
        next[m.edid_id] = prev[m.edid_id] ?? { status: "loading" };
      }
      return next;
    });
    for (const m of connected) {
      if (backends[m.edid_id]?.status === "ready") continue; // already resolved
      getDisplayCapabilities(m.edid_id)
        .then((caps) => {
          if (cancelled) return;
          const unsupported = caps.adjustments
            .filter((a) => a.status === "unsupported")
            .map((a) => adjustmentLabel(a.adjustment));
          setBackends((prev) => ({
            ...prev,
            [m.edid_id]: { status: "ready", label: caps.backend_label, unsupported },
          }));
        })
        .catch(() => {
          if (cancelled) return;
          setBackends((prev) => ({ ...prev, [m.edid_id]: { status: "error" } }));
        });
    }
    return () => { cancelled = true; };
    // `monitors` is intentionally not a dependency: the poll replaces the
    // array every tick without changing the connected set.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectedKey]);

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
  const summary = lastResult ? summarizeApply(lastResult) : null;
  const selectedBackend = backends[selectedEdid];

  const statusClass: Record<string, string> = {
    applied: "text-ink",
    skipped: "text-muted-foreground",
    failed: "text-danger-ink",
  };

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            Apply: {preset.name}
          </DialogTitle>
          <DialogDescription className="mono text-[11px] uppercase tracking-[0.12em]">
            Select target monitor
          </DialogDescription>
        </DialogHeader>

        {/* ── Monitor list ──────────────────────────────────────── */}
        <RadioGroup
          value={selectedEdid}
          onValueChange={(v) => {
            setSelectedEdid(v);
            setPinToggle(pins[v] === preset.id);
            setLastResult(null);
          }}
          className="space-y-2"
        >
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-[0.12em] mb-1 mono">
            Monitors
          </p>
          {monitors.length === 0 && (
            <p className="text-xs text-danger-ink">No monitors detected.</p>
          )}
          {monitors.map((m) => {
            const isSelected = m.edid_id === selectedEdid;
            const isPinnedHere = pins[m.edid_id] === preset.id;
            const backend = backends[m.edid_id];
            return (
                <label
                  key={m.edid_id}
                  className={`flex items-center gap-3 px-3 py-2.5 rounded-brutal cursor-pointer select-none ${
                    !m.connected
                      ? "border border-dashed border-[var(--color-border)] bg-transparent text-muted-foreground"
                      : isSelected
                        ? "border border-mint bg-card"
                        : "border border-[var(--color-border)] bg-card hover:bg-secondary"
                  }`}
                >
                <RadioGroupItem
                  value={m.edid_id}
                  disabled={!m.connected}
                  onClick={() => {
                    if (m.connected) {
                      setLastResult(null);
                    }
                  }}
                >
                  <RadioGroupIndicator />
                </RadioGroupItem>
                <span className="flex-1 min-w-0 truncate text-sm font-medium text-foreground">
                  {m.alias || m.model || m.device_name}
                </span>
                <span className="inline-flex items-center gap-1 shrink-0">
                  {m.connected && backend?.status === "ready" && (
                    <Badge
                      variant="outline"
                      className="h-4 px-1.5 text-[10px] mono tracking-wide"
                      title={`Colour backend: ${backend.label}`}
                    >
                      {backend.label}
                    </Badge>
                  )}
                  {m.connected && backend?.status === "loading" && (
                    <span
                      className="text-[10px] text-muted-foreground mono"
                      title="Detecting colour backend…"
                      aria-hidden="true"
                    >
                      …
                    </span>
                  )}
                  {m.connected && backend?.status === "error" && (
                    <span
                      className="text-[10px] text-muted-foreground mono"
                      title="Colour backend could not be detected"
                    >
                      n/a
                    </span>
                  )}
                  {isPinnedHere && (
                    <span className="inline-flex items-center gap-1">
                      <span className="status-dot status-dot-pinned" title="Pinned" />
                      <Badge variant="pinned">
                        PINNED
                      </Badge>
                    </span>
                  )}
                  {m.connected ? (
                    <span className="status-dot status-dot-connected" title="Connected" />
                  ) : (
                    <span className="text-xs text-muted-foreground uppercase tracking-[0.12em] mono">
                      OFFLINE
                    </span>
                  )}
                </span>
              </label>
            );
          })}
        </RadioGroup>

        {/* Capability for the chosen target lives here, where the target is
            actually known — the preset editor is global and stays ungated. */}
        {selectedBackend?.status === "ready" && selectedBackend.unsupported.length > 0 && (
          <p className="text-[10px] text-muted-foreground mono leading-snug">
            Not supported on this display: {selectedBackend.unsupported.join(", ")}
          </p>
        )}

        {/* ── Pin toggle ────────────────────────────────────────── */}
        <div className="flex items-center gap-3">
          <Label className="inline-flex items-center gap-2 cursor-pointer">
            <Checkbox
              checked={pinToggle}
              onCheckedChange={setPinToggle}
            >
              <CheckboxIndicator />
            </Checkbox>
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-[0.12em] mono">
              PIN TO MONITOR
            </span>
          </Label>
          {isPinnedOnTarget && pinToggle && (
            <span className="text-xs font-bold text-ink transient-enter">Already pinned</span>
          )}
        </div>

        {/* ── Result feedback — persistent ──────────────────────── */}
        {lastResult && summary && (
          <div aria-live="polite" className="feedback-enter">
            {lastResult.error && (
              <span className="text-xs text-danger-ink block validation-slide">
                Apply failed: {lastResult.error}
              </span>
            )}
            {!lastResult.error && (
              <span className="text-xs text-ink font-bold block mono feedback-enter">
                {summary.headline}
                {pinToggle ? " · PINNED" : ""}
              </span>
            )}
            {summary.rows.length > 0 && (
              <ul className="mt-1.5 space-y-0.5">
                {summary.rows.map((row) => (
                  <li
                    key={row.adjustment}
                    className="flex items-start gap-1.5 text-[10px] mono leading-tight"
                  >
                    <span className={`font-bold shrink-0 ${statusClass[row.status]}`}>
                      {row.status.toUpperCase()}
                    </span>
                    <span className="font-bold shrink-0">{row.label}</span>
                    {row.reason && (
                      <span className="text-muted-foreground">{row.reason}</span>
                    )}
                  </li>
                ))}
              </ul>
            )}
            {summary.verificationNote && (
              <span className="mt-1 block text-[10px] text-muted-foreground">
                {summary.verificationNote}
              </span>
            )}
          </div>
        )}

        {/* ── Actions ───────────────────────────────────────────── */}
        <DialogFooter>
          <Button variant="outline" size="sm" onClick={onClose} disabled={applying}>
            Cancel
          </Button>
          {lastResult && !lastResult.error && (
            <Button variant="outline" size="sm" onClick={onClose}>
              Done
            </Button>
          )}
          <Button variant="default" size="sm" onClick={handleApply} disabled={!selectedEdid || applying}>
            {applying ? "Applying…" : "Apply"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}