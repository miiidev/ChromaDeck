import { useEffect, useState } from "react";
import type { Monitor, Preset, ScheduleRule } from "../lib/types";
import { listSchedules, saveSchedules } from "../lib/tauri";
import {
  formatScheduleWindow,
  isOvernightWindow,
  validateScheduleRule,
} from "../lib/schedule";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Clock, Moon, Plus, Trash2 } from "lucide-react";

interface Props {
  open: boolean;
  monitors: Monitor[];
  presets: Preset[];
  onClose: () => void;
  onSaved: (rules: ScheduleRule[]) => void;
}

function monitorLabel(m: Monitor): string {
  const base = m.alias || m.model || m.device_name || m.edid_id.slice(0, 16);
  return m.connected ? base : `${base} (offline)`;
}

export default function ScheduleDialog({
  open,
  monitors,
  presets,
  onClose,
  onSaved,
}: Props) {
  const [rules, setRules] = useState<ScheduleRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    let active = true;
    setLoading(true);
    setError(null);
    void listSchedules()
      .then((loaded) => {
        if (active) setRules(loaded);
      })
      .catch((err) => {
        if (active) setError(`Failed to load schedules: ${String(err)}`);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [open]);

  const canAdd = monitors.length > 0 && presets.length > 0;

  const addRule = () => {
    if (!canAdd) return;
    const defaultMonitor =
      monitors.find((m) => m.connected)?.edid_id ?? monitors[0].edid_id;
    const defaultPreset = presets[0].id;
    setRules((prev) => [
      ...prev,
      {
        id: `draft-${Date.now()}-${prev.length}`,
        edid_id: defaultMonitor,
        preset_id: defaultPreset,
        start_time: "21:00",
        end_time: "07:00",
        enabled: true,
      },
    ]);
    setError(null);
  };

  const updateRule = (index: number, patch: Partial<ScheduleRule>) => {
    setRules((prev) =>
      prev.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)),
    );
    setError(null);
  };

  const removeRule = (index: number) => {
    setRules((prev) => prev.filter((_, i) => i !== index));
    setError(null);
  };

  const handleSave = async () => {
    for (let i = 0; i < rules.length; i++) {
      const msg = validateScheduleRule(rules[i]);
      if (msg) {
        setError(`Rule ${i + 1}: ${msg}`);
        return;
      }
    }
    setSaving(true);
    setError(null);
    try {
      const saved = await saveSchedules(rules);
      onSaved(saved);
      onClose();
    } catch (err) {
      setError(`Save failed: ${String(err)}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !saving) onClose();
      }}
    >
      <DialogContent
        data-slot="schedule-dialog"
        className="flex max-h-[82vh] w-[min(92vw,620px)] max-w-[620px] sm:max-w-[620px] flex-col gap-4 rounded-[8px] border border-border bg-surface-raised p-6"
      >
        <div className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <Clock className="size-4 text-primary" aria-hidden="true" />
            <DialogTitle className="font-display text-[17px] font-semibold tracking-[-0.02em] text-foreground">
              Display Schedules
            </DialogTitle>
          </div>
          <DialogDescription className="font-body text-[12px] text-muted-foreground">
            Automatically switch presets by local time of day. Overnight windows (e.g.{" "}
            <span className="font-mono text-foreground">21:00</span> to{" "}
            <span className="font-mono text-foreground">07:00</span>) wrap past midnight cleanly.
          </DialogDescription>
        </div>

        <div className="rounded-[8px] border border-border bg-surface px-3 py-2 font-mono text-[10px] text-muted-foreground">
          Precedence: Pinned monitor &gt; Active auto-apply &gt; Schedule &gt; Manual default
        </div>

        {error && (
          <div
            role="alert"
            className="rounded-[8px] border border-destructive/40 bg-destructive/10 px-3 py-2 font-mono text-[11px] text-destructive-text"
          >
            {error}
          </div>
        )}

        <div className="flex min-h-[120px] flex-1 flex-col gap-2.5 overflow-y-auto pr-1">
          {loading ? (
            <div className="flex flex-1 items-center justify-center py-8 font-mono text-[11px] text-muted-foreground">
              Loading schedules…
            </div>
          ) : rules.length === 0 ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-2 rounded-[8px] border border-dashed border-border py-8 text-center">
              <p className="font-mono text-[11px] text-muted-foreground">
                No time-of-day schedules configured
              </p>
              <p className="max-w-sm font-body text-[11px] text-muted-foreground/80">
                {canAdd
                  ? "Add a schedule rule to automatically apply a warmer or lower-brightness preset at night."
                  : "Connect a monitor and create at least one preset before adding a schedule."}
              </p>
            </div>
          ) : (
            rules.map((rule, index) => {
              const overnight = isOvernightWindow(rule.start_time, rule.end_time);
              const selectedMonitor = monitors.find((m) => m.edid_id === rule.edid_id);
              const selectedPreset = presets.find((p) => p.id === rule.preset_id);
              return (
                <div
                  key={rule.id || index}
                  data-slot="schedule-rule-row"
                  className="flex flex-col gap-2.5 rounded-[8px] border border-border bg-surface p-3"
                >
                  <div className="flex flex-wrap items-center gap-2">
                    <label
                      className="inline-flex cursor-pointer items-center"
                      title={rule.enabled ? "Disable rule" : "Enable rule"}
                    >
                      <input
                        type="checkbox"
                        checked={rule.enabled}
                        onChange={(e) =>
                          updateRule(index, { enabled: e.target.checked })
                        }
                        aria-label={`Enable schedule rule ${index + 1}`}
                        className="size-[14px] rounded-[4px] accent-[var(--color-primary)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      />
                    </label>

                    <div className="min-w-[160px] flex-1">
                      <Select
                        value={rule.edid_id}
                        onValueChange={(value) => {
                          if (typeof value === "string" && value) {
                            updateRule(index, { edid_id: value });
                          }
                        }}
                      >
                        <SelectTrigger
                          size="sm"
                          aria-label={`Monitor for rule ${index + 1}`}
                          className="w-full font-body text-[12px]"
                        >
                          <SelectValue>
                            {selectedMonitor
                              ? monitorLabel(selectedMonitor)
                              : rule.edid_id.slice(0, 16)}
                          </SelectValue>
                        </SelectTrigger>
                        <SelectContent>
                          {monitors.map((m) => (
                            <SelectItem key={m.edid_id} value={m.edid_id}>
                              {monitorLabel(m)}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>

                    <div className="min-w-[160px] flex-1">
                      <Select
                        value={rule.preset_id}
                        onValueChange={(value) => {
                          if (typeof value === "string" && value) {
                            updateRule(index, { preset_id: value });
                          }
                        }}
                      >
                        <SelectTrigger
                          size="sm"
                          aria-label={`Preset for rule ${index + 1}`}
                          className="w-full font-body text-[12px]"
                        >
                          <SelectValue>
                            {selectedPreset ? selectedPreset.name : "Select preset"}
                          </SelectValue>
                        </SelectTrigger>
                        <SelectContent>
                          {presets.map((p) => (
                            <SelectItem key={p.id} value={p.id}>
                              {p.name}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>

                    <button
                      type="button"
                      onClick={() => removeRule(index)}
                      aria-label={`Delete schedule rule ${index + 1}`}
                      title="Remove rule"
                      className="inline-flex size-7 shrink-0 items-center justify-center rounded-[8px] text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    >
                      <Trash2 className="size-3.5" />
                    </button>
                  </div>

                  <div className="flex flex-wrap items-center justify-between gap-2 pl-5">
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-[10px] uppercase tracking-[0.08em] text-muted-foreground">
                        From
                      </span>
                      <Input
                        type="text"
                        inputMode="numeric"
                        maxLength={5}
                        value={rule.start_time}
                        placeholder="21:00"
                        aria-label={`Start time for rule ${index + 1}`}
                        onChange={(e) =>
                          updateRule(index, {
                            start_time: (e.target as HTMLInputElement).value,
                          })
                        }
                        className="h-7 w-[72px] rounded-[6px] px-2 text-center font-mono text-[12px] tabular-nums"
                      />
                      <span className="font-mono text-[10px] uppercase tracking-[0.08em] text-muted-foreground">
                        To
                      </span>
                      <Input
                        type="text"
                        inputMode="numeric"
                        maxLength={5}
                        value={rule.end_time}
                        placeholder="07:00"
                        aria-label={`End time for rule ${index + 1}`}
                        onChange={(e) =>
                          updateRule(index, {
                            end_time: (e.target as HTMLInputElement).value,
                          })
                        }
                        className="h-7 w-[72px] rounded-[6px] px-2 text-center font-mono text-[12px] tabular-nums"
                      />
                    </div>

                    <div className="flex items-center gap-1.5 font-mono text-[10px] text-muted-foreground">
                      {overnight && (
                        <Moon
                          className="size-3 text-primary"
                          aria-hidden="true"
                        />
                      )}
                      <span>
                        {formatScheduleWindow(rule.start_time, rule.end_time)}
                      </span>
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>

        <DialogFooter className="flex items-center justify-between gap-2 border-t border-border pt-3 sm:justify-between">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            onClick={addRule}
            disabled={!canAdd || saving}
          >
            <Plus className="size-3.5" />
            Add schedule
          </Button>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={onClose}
              disabled={saving}
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="default"
              size="sm"
              onClick={() => void handleSave()}
              disabled={saving || loading}
            >
              {saving ? "Saving…" : "Save schedules"}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
