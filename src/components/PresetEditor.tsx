import { useState, useEffect, type FormEvent } from "react";

import type { Monitor, Preset, PresetInput } from "../lib/types";
import { createPreset, updatePreset, importIcc, captureNvcp } from "../lib/tauri";
import { validatePresetForm, type ValidationErrors } from "../lib/validation";
import { DEFAULT_PRESET_INPUT, presetToFormValues, resetSliderValues } from "../lib/presetForm";
import { open } from "@tauri-apps/plugin-dialog";
import PreviewStrip from "@/components/PreviewStrip";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogFooter, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";

interface Props {
  monitors: Monitor[];
  editPreset: Preset | null;
  onClose: () => void;
  onSaved: () => void;
}

export default function PresetEditor({ monitors, editPreset, onClose, onSaved }: Props) {
  const isEditing = editPreset !== null;
  const [form, setForm] = useState<PresetInput>(DEFAULT_PRESET_INPUT);
  const [errors, setErrors] = useState<ValidationErrors>({});
  const [saving, setSaving] = useState(false);
  const [iccStatus, setIccStatus] = useState<{ hash: string; filename: string } | null>(null);
  const [iccImporting, setIccImporting] = useState(false);
  const [captureMonitorId, setCaptureMonitorId] = useState<string>(() => {
    return monitors.find((m) => m.connected)?.edid_id ?? "";
  });
  const [nvcpImporting, setNvcpImporting] = useState(false);
  // Presets are global. The editor deliberately shows no per-display
  // capability gating: binding it to the import-only capture dropdown misled
  // users into thinking that control chose the apply target. Capability is
  // reported at apply time (ApplyDialog), where the actual target is known.
  const [tagFocused, setTagFocused] = useState(false);
  const [tagDraft, setTagDraft] = useState("");

  // Follow stored tag while not editing the hex field.
  useEffect(() => {
    if (!tagFocused) setTagDraft(form.color_tag ?? "");
  }, [form.color_tag, tagFocused]);

  // Populate form when editing
  useEffect(() => {
    if (editPreset) {
      setForm(presetToFormValues(editPreset));
      if (editPreset.icc_hash) {
        setIccStatus({ hash: editPreset.icc_hash, filename: editPreset.icc_filename });
      }
    } else {
      setForm(DEFAULT_PRESET_INPUT);
      setIccStatus(null);
    }
    setErrors({});
    setIccImporting(false);
  }, [editPreset]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleBrowseIcc = async () => {
    setIccImporting(true);
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "ICC profiles", extensions: ["icc", "icm"] }],
      });
      if (!selected) {
        setIccImporting(false);
        return;
      }
      const hash = await importIcc(selected);
      const parts = selected.replace(/\\/g, "/").split("/");
      setIccStatus({ hash, filename: parts[parts.length - 1] });
      setErrors((prev) => {
        const { icc_path: _, ...rest } = prev;
        return rest;
      });
    } catch (err) {
      setErrors((prev) => ({ ...prev, icc_path: `ICC import failed: ${err}` }));
    } finally {
      setIccImporting(false);
    }
  };

  const handleCaptureNvcp = async () => {
    if (!captureMonitorId) return;
    setNvcpImporting(true);
    setErrors((prev) => {
      const { nvcp: _, ...rest } = prev;
      return rest;
    });
    try {
      const state = await captureNvcp(captureMonitorId);
      setForm((prev) => ({
        ...prev,
        brightness: state.brightness,
        contrast: state.contrast,
        gamma: state.gamma,
        vibrance: state.vibrance,
        hue_deg: state.hue_deg,
      }));
    } catch (err) {
      setErrors((prev) => ({ ...prev, nvcp: `NVCP capture failed: ${err}` }));
    } finally {
      setNvcpImporting(false);
    }
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const errs: ValidationErrors = {};
    if (!validatePresetForm(form, errs)) {
      setErrors(errs);
      return;
    }
    setSaving(true);
    try {
      if (isEditing && editPreset) {
        await updatePreset(editPreset.id, form);
      } else {
        await createPreset(form);
      }
      onSaved();
    } catch (err) {
      setErrors({ name: `Save failed: ${err}` });
    } finally {
      setSaving(false);
    }
  };

  const updateField = <K extends keyof PresetInput>(key: K, value: PresetInput[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }));
    if (errors[key as keyof ValidationErrors]) {
      setErrors((prev) => {
        const { [key as keyof ValidationErrors]: _, ...rest } = prev;
        return rest;
      });
    }
  };

  const connectedMonitors = monitors.filter((m) => m.connected);

  // Restore every color setting to its neutral default. Name, ICC
  // selection, tag color, and capture source are left untouched.
  const handleResetSliders = () => {
    setForm(resetSliderValues);
    setErrors((prev) => {
      const { brightness: _, contrast: __, rgb_gains: ___, gamma: ____, vibrance: _____, saturation: ______, hue_deg: _______, ...rest } = prev;
      return rest;
    });
  };

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="sm:max-w-[682px] max-h-[calc(100dvh-3rem)] overflow-y-auto overflow-x-clip">
        <DialogHeader>
          <DialogTitle>
            {isEditing ? `EDIT: ${editPreset?.name}` : "CREATE PRESET"}
          </DialogTitle>
        </DialogHeader>

        {/* Form — two columns on sm+: preview/basics left, sliders right */}
        <form onSubmit={handleSubmit} className="grid gap-5 sm:grid-cols-[240px_1fr]">
          <div className="space-y-3 min-w-0">
          {/* Live simulated preview */}
          <PreviewStrip
            preset={{
              brightness: form.brightness,
              contrast: form.contrast,
              gamma: form.gamma,
              rgb_gains: form.rgb_gains,
              vibrance: form.vibrance,
              saturation: form.saturation,
              hue_deg: form.hue_deg,
            }}
            height={96}
            showTag
          />
          {iccStatus && (
            <p className="text-[10px] text-muted-foreground mono -mt-3">
              SIM excludes ICC profile
            </p>
          )}

          {/* Name */}
          <div>
            <Label htmlFor="preset-name" className="text-[10px] uppercase tracking-[0.12em] mb-1.5 mono">
              Name
            </Label>
            <Input
              id="preset-name"
              type="text"
              value={form.name}
              onChange={(e) => updateField("name", (e.target as HTMLInputElement).value)}
              placeholder="My color preset"
              aria-invalid={errors.name ? ("true" as const) : undefined}
            />
            {errors.name && <p className="mt-1 text-xs text-danger-ink validation-slide">{errors.name}</p>}
          </div>

          {/* NVCP capture source */}
          <div>
            <div className="mb-1.5 flex items-baseline gap-2">
              <Label className="shrink-0 text-[10px] uppercase tracking-[0.12em] mono">
                NVCP Capture Source
              </Label>
              <span className="truncate text-[9px] normal-case text-muted-foreground">
                (optional, import only)
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Select value={captureMonitorId} onValueChange={(v: string | null) => setCaptureMonitorId(v ?? "")}>
                <SelectTrigger className="min-w-0 flex-1">
                  {/* Explicit label: Base UI falls back to the raw edid_id
                      when the selected monitor has no mounted item
                      (e.g. disconnected mid-session), so resolve the
                      display name from the FULL monitor list here. */}
                  <SelectValue>
                    {(v: string | null) => {
                      if (!v) return "SELECT SOURCE";
                      const m = monitors.find((mon) => mon.edid_id === v);
                      return m ? (m.alias || m.model || m.device_name) : v;
                    }}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="">SELECT SOURCE</SelectItem>
                  {connectedMonitors.map((m) => (
                    <SelectItem key={m.edid_id} value={m.edid_id}>
                      {m.alias || m.model || m.device_name} {m.serial ? `(${m.serial})` : ""}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Button
                variant="outline"
                size="sm"
                className="shrink-0"
                onClick={handleCaptureNvcp}
                disabled={!captureMonitorId || nvcpImporting}
              >
                {nvcpImporting ? "Importing…" : "Import NVCP"}
              </Button>
            </div>
            {errors.nvcp && <p className="mt-1 text-xs text-danger-ink validation-slide">{errors.nvcp}</p>}
            {connectedMonitors.length === 0 && (
              <p className="mt-1 text-xs text-danger-ink">No connected monitors detected.</p>
            )}
          </div>

          {/* ICC file picker */}
          <div>
            <div className="mb-1.5 flex items-baseline gap-2">
              <Label className="shrink-0 text-[10px] uppercase tracking-[0.12em] mono">
                ICC Profile
              </Label>
              <span className="truncate text-[9px] normal-case text-muted-foreground">
                (optional)
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm" className="shrink-0" onClick={handleBrowseIcc} disabled={iccImporting}>
                {iccImporting ? "Importing…" : "Browse…"}
              </Button>
              {iccStatus ? (
                <span className="min-w-0 flex-1 text-[11px] text-ink font-bold truncate mono icc-pop" title={iccStatus.filename}>{iccStatus.filename}</span>
              ) : (
                <span className="min-w-0 flex-1 text-[11px] text-muted-foreground mono truncate">No ICC profile selected</span>
              )}
            </div>
            {errors.icc_path && <p className="mt-1 text-xs text-danger-ink validation-slide">{errors.icc_path}</p>}
          </div>

          {/* Tag color — fully custom: native picker + hex field */}
          <div>
            <div className="mb-1.5 flex items-baseline gap-2">
              <Label className="shrink-0 text-[10px] uppercase tracking-[0.12em] mono">
                Tag Color
              </Label>
              <span className="truncate text-[9px] normal-case text-muted-foreground">
                (optional, tints card)
              </span>
            </div>
            <div className="flex items-center gap-2">
              <input
                type="color"
                value={/^#[0-9a-fA-F]{6}$/.test(form.color_tag ?? "") ? form.color_tag as string : "#22d3ee"}
                onChange={(e) => updateField("color_tag", (e.target as HTMLInputElement).value)}
                className="h-7 w-9 shrink-0 cursor-pointer rounded-md border border-border bg-transparent p-0.5"
                aria-label="Pick tag color"
                title="Pick tag color"
              />
              <input
                type="text"
                value={tagDraft}
                spellCheck={false}
                maxLength={7}
                placeholder="#rrggbb"
                aria-label="Tag color hex value, type to edit"
                title="Hex color, e.g. #ff5533 (Enter commits, Escape reverts)"
                onChange={(e) => setTagDraft((e.target as HTMLInputElement).value)}
                onFocus={() => setTagFocused(true)}
                onBlur={() => {
                  setTagFocused(false);
                  const v = tagDraft.trim();
                  if (/^#[0-9a-fA-F]{6}$/.test(v)) updateField("color_tag", v);
                  else setTagDraft(form.color_tag ?? "");
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                  else if (e.key === "Escape") {
                    setTagDraft(form.color_tag ?? "");
                    (e.target as HTMLInputElement).blur();
                  }
                }}
                className="min-w-0 flex-1 bg-transparent p-0 text-xs mono tabular-nums outline-none border-b border-transparent text-muted-foreground hover:text-foreground focus:text-foreground focus:border-ink"
              />
              <Button
                variant="ghost"
                size="xs"
                className="shrink-0"
                onClick={() => {
                  updateField("color_tag", "");
                  setTagDraft("");
                }}
                title="Remove tag color"
                aria-label="Remove tag color"
              >
                None
              </Button>
              <span
                aria-hidden="true"
                title="Card tint preview"
                className="h-5 w-8 shrink-0 rounded-md border border-border"
                style={
                  /^#[0-9a-fA-F]{6}$/.test(form.color_tag ?? "")
                    ? ({ backgroundColor: `color-mix(in srgb, var(--color-card), ${form.color_tag} 25%)` } as Record<string, string>)
                    : undefined
                }
              />
            </div>
            {errors.color_tag && <p className="mt-1 text-xs text-danger-ink validation-slide">{errors.color_tag}</p>}
          </div>
          </div>

          {/* Right column — sliders — full-width stack (Figma: one field per
              row, value in the label row, markers under the track). A 2-col
              mini-grid squeezed the tracks to ~180px, so the mid marker
              collided with the thumb. */}
          <div className="flex flex-col gap-4 min-w-0">
          {/* Gamma | Brightness — side by side */}
          <SliderField
            label="Gamma"
            min={0.3} max={2.8} step={0.05}
            value={form.gamma}
            display={form.gamma.toFixed(2)}
            onChange={(v) => updateField("gamma", v)}
            markers={[{ value: 0.3, label: "0.3" }, { value: 2.8, label: "2.8" }]}
            error={errors.gamma}
          />

          <SliderField
            label="Brightness"
            min={0} max={100} step={1}
            value={form.brightness}
            display={form.brightness.toFixed(0)}
            onChange={(v) => updateField("brightness", v)}
            markers={[{ value: 0, label: "0" }, { value: 50, label: "50" }, { value: 100, label: "100" }]}
            error={errors.brightness}
          />

          {/* Contrast | Vibrance — side by side */}
          <SliderField
            label="Contrast"
            min={0} max={100} step={1}
            value={form.contrast}
            display={form.contrast.toFixed(0)}
            onChange={(v) => updateField("contrast", v)}
            markers={[{ value: 0, label: "0" }, { value: 50, label: "50" }, { value: 100, label: "100" }]}
            error={errors.contrast}
          />

          <SliderField
            label="Digital Vibrance"
            min={0} max={100} step={1}
            value={form.vibrance}
            display={form.vibrance.toFixed(0)}
            onChange={(v) => updateField("vibrance", v)}
            markers={[{ value: 0, label: "0" }, { value: 50, label: "50" }, { value: 100, label: "100" }]}
            error={errors.vibrance}
          />

          {/* RGB Gains — full width */}
          <div className="sm:col-span-2">
            <div className="mb-2 flex items-baseline gap-2">
              <Label className="shrink-0 text-[10px] uppercase tracking-[0.12em] mono">
                RGB Gains
              </Label>
              <span className="truncate text-[9px] normal-case text-muted-foreground">
                (1.0 = neutral)
              </span>
            </div>
            <div className="grid grid-cols-3 gap-3">
              {(["R", "G", "B"] as const).map((channel, idx) => {
                return (
                  <div key={channel}>
                    <div className="mb-1">
                      <span className="text-xs mono text-foreground">{channel}</span>
                    </div>
                    <Slider
                      min={0} max={2} step={0.05}
                      value={[form.rgb_gains[idx]]}
                      onValueChange={([v]) => {
                        const newGains = [...form.rgb_gains] as [number, number, number];
                        newGains[idx] = v;
                        updateField("rgb_gains", newGains);
                      }}
                      className="w-full"
                    />
                    {/* Value on its own row, right-aligned: the channel
                        column is too narrow for a side-by-side readout. */}
                    <div className="mt-1 flex justify-end">
                      <EditableNumber
                        label={`${channel} gain`}
                        min={0} max={2} step={0.05}
                        display={form.rgb_gains[idx].toFixed(1)}
                        onCommit={(v) => {
                          const newGains = [...form.rgb_gains] as [number, number, number];
                          newGains[idx] = v;
                          updateField("rgb_gains", newGains);
                        }}
                        className="w-14 shrink-0"
                      />
                    </div>
                  </div>
                );
              })}
            </div>
            {errors.rgb_gains && <p className="mt-1 text-xs text-danger-ink">{errors.rgb_gains}</p>}
          </div>

          {/* Saturation | Hue — side by side */}
          <SliderField
            label="Saturation"
            min={0} max={100} step={1}
            value={form.saturation}
            display={form.saturation.toFixed(0)}
            onChange={(v) => updateField("saturation", v)}
            markers={[{ value: 0, label: "0" }, { value: 50, label: "50" }, { value: 100, label: "100" }]}
            error={errors.saturation}
          />

          <SliderField
            label="Hue"
            min={0} max={359} step={1}
            value={form.hue_deg}
            display={`${form.hue_deg.toFixed(0)}°`}
            onChange={(v) => updateField("hue_deg", v)}
            markers={[{ value: 0, label: "0" }, { value: 359, label: "359" }]}
            error={errors.hue_deg}
          />

          {/* Precedence footnote */}
          <p className="sm:col-span-2 text-[10px] leading-relaxed text-muted-foreground mono">
            <span className="font-medium uppercase tracking-[0.12em] text-muted-foreground">Precedence:</span>
            ICC profile first, then gamma + RGB gains overlaid.
          </p>
          </div>

          {/* Actions — full width */}
          <DialogFooter className="sm:col-span-2">
            <Button variant="outline" size="sm" type="button" onClick={handleResetSliders} title="Restore all color settings to their neutral defaults">
              Reset
            </Button>
            <Button variant="outline" size="sm" onClick={onClose}>
              Cancel
            </Button>
            <Button variant="secondary" size="sm" type="submit" disabled={saving} className="save-crossfade">
              {saving ? "Saving…" : isEditing ? "Update" : "Create"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Editable numeric readout beside a slider.
 * Shows the formatted display string; typing + Enter/blur commits a parsed
 * value clamped to [min,max] and snapped to step (Escape reverts).
 * Invalid input reverts silently — range errors surface at save time. */
function EditableNumber({
  label, min, max, step, display, onCommit, disabled, className,
}: {
  label: string;
  min: number; max: number; step: number;
  display: string;
  onCommit: (v: number) => void;
  disabled?: boolean;
  className?: string;
}) {
  const [draft, setDraft] = useState(display);
  const [focused, setFocused] = useState(false);
  // Follow slider drags while not editing.
  useEffect(() => {
    if (!focused) setDraft(display);
  }, [display, focused]);

  const commit = () => {
    const n = parseFloat(draft);
    if (!Number.isFinite(n)) {
      setDraft(display);
      return;
    }
    const clamped = Math.min(max, Math.max(min, n));
    const snapped = Math.round((clamped - min) / step) * step + min;
    onCommit(Number(snapped.toFixed(6)));
  };

  return (
    <input
      type="text"
      inputMode="decimal"
      value={draft}
      disabled={disabled}
      aria-label={`${label} value, type a number to edit`}
      title={`${label}: ${display} (click to edit, range ${min} to ${max})`}
      onChange={(e) => setDraft((e.target as HTMLInputElement).value)}
      onFocus={() => setFocused(true)}
      onBlur={() => {
        setFocused(false);
        commit();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        else if (e.key === "Escape") {
          setDraft(display);
          (e.target as HTMLInputElement).blur();
        }
      }}
      className={`bg-transparent p-0 text-right text-xs mono tabular-nums outline-none border-b border-transparent text-muted-foreground hover:text-foreground focus:text-foreground focus:border-ink disabled:pointer-events-none disabled:opacity-50 ${className ?? ""}`}
    />
  );
}

/** Marker label pinned to its true track position. */
export interface SliderMarker {
  value: number;
  label: string;
}

/** Reusable slider + label + value + markers */
function SliderField({
  label, min, max, step, value, display, onChange, markers, error,
}: {
  label: string;
  min: number; max: number; step: number;
  value: number;
  display: string;
  onChange: (v: number) => void;
  markers: SliderMarker[];
  error?: string;
}) {
  return (
    <div>
      {/* Label row carries the value on the right, per the Figma editor:
          the label, the current value, then a full-width track below. */}
      <div className="mb-1.5 flex items-end justify-between gap-3">
        <Label className="text-[10px] uppercase tracking-[0.12em] mono">
          {label}
        </Label>
        <EditableNumber
          label={label}
          min={min} max={max} step={step}
          display={display}
          onCommit={onChange}
          className="w-14 shrink-0"
        />
      </div>
      <Slider
        min={min}
        max={max}
        step={step}
        value={[value]}
        onValueChange={([v]) => onChange(v)}
        className="w-full"
      />
      {/* Markers sit exactly under their track positions: each label is
          centered on the thumb-center position for its value (10px
          half-thumb insets for the size-4 thumb). End markers are
          edge-pinned (first left-aligned, last right-aligned) so short
          labels like "0.3" never hang past the row edge and get clipped by
          the dialog's overflow-x-clip. */}
      <div className="relative mt-2.5 h-4 mono text-[11px] tabular-nums text-muted-foreground">
        {markers.map((m, i) => {
          const frac = Math.min(1, Math.max(0, (m.value - min) / (max - min)));
          const align = i === 0 ? "translateX(0)" : i === markers.length - 1 ? "translateX(-100%)" : "translateX(-50%)";
          return (
            <span
              key={m.label}
              className="absolute whitespace-nowrap"
              style={{ left: `calc(8px + ${frac} * (100% - 16px))`, transform: align }}
            >
              {m.label}
            </span>
          );
        })}
      </div>
      {error && <p className="mt-1 text-xs text-danger-ink validation-slide">{error}</p>}
    </div>
  );
}