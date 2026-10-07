import { useState, useEffect } from "react";
import type { Monitor, Preset, ApplyResult } from "../lib/types";
import { applyPreset, pinPreset, unpinMonitor } from "../lib/tauri";
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

export default function ApplyDialog({ preset, monitors, pins, initialEdid, onClose, onApplied }: Props) {
  const [selectedEdid, setSelectedEdid] = useState<string>("");
  const [pinToggle, setPinToggle] = useState(false);
  const [applying, setApplying] = useState(false);
  const [lastResult, setLastResult] = useState<ApplyResult | null>(null);

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
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            Apply: {preset.name}
          </DialogTitle>
          <DialogDescription className="mono uppercase tracking-widest">
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
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-widest mb-1 mono">
            Monitors
          </p>
          {monitors.length === 0 && (
            <p className="text-xs text-destructive">No monitors detected.</p>
          )}
          {monitors.map((m) => {
            const isSelected = m.edid_id === selectedEdid;
            const isPinnedHere = pins[m.edid_id] === preset.id;
            return (
              <label
                key={m.edid_id}
                className={`flex items-center gap-3 px-3 py-2.5 rounded-lg border border-border cursor-pointer select-none ${
                  isSelected
                    ? "bg-muted border-primary"
                    : m.connected
                      ? "bg-card"
                      : "bg-card/50 text-muted-foreground"
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
                  {isPinnedHere && (
                    <span className="inline-flex items-center gap-1">
                      <span className="status-dot status-dot-pinned" title="Pinned" />
                      <Badge variant="outline" className="text-accent border-accent">
                        PINNED
                      </Badge>
                    </span>
                  )}
                  {m.connected ? (
                    <span className="status-dot status-dot-connected" title="Connected" />
                  ) : (
                    <span className="text-xs text-muted-foreground uppercase tracking-widest mono">
                      OFFLINE
                    </span>
                  )}
                </span>
              </label>
            );
          })}
        </RadioGroup>

        {/* ── Pin toggle ────────────────────────────────────────── */}
        <div className="flex items-center gap-3">
          <Label className="inline-flex items-center gap-2 cursor-pointer">
            <Checkbox
              checked={pinToggle}
              onCheckedChange={setPinToggle}
            >
              <CheckboxIndicator />
            </Checkbox>
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-widest mono">
              PIN TO MONITOR
            </span>
          </Label>
          {isPinnedOnTarget && pinToggle && (
            <span className="text-xs text-primary transient-enter">Already pinned</span>
          )}
        </div>

        {/* ── Result feedback — persistent ──────────────────────── */}
        {lastResult && (
          <div aria-live="polite" className={lastResult.error ? "exit-fade" : "feedback-enter"}>
            {lastResult.error ? (
              <span className="text-xs text-destructive block validation-slide">Apply failed: {lastResult.error}</span>
            ) : (
              <span className="text-xs text-primary block mono feedback-enter">
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