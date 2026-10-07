import { useState, useEffect, type FormEvent } from "react";

import type { Monitor, Preset, PresetInput } from "../lib/types";
import { createPreset, updatePreset, importIcc } from "../lib/tauri";
import { vibranceSupported, captureNvcp } from "../lib/tauri";
import { validatePresetForm, type ValidationErrors } from "../lib/validation";
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

const DEFAULT_INPUT: PresetInput = {
  name: "",
  brightness: 50,
  contrast: 50,
  rgb_gains: [1.0, 1.0, 1.0],
  gamma: 1.0,
  vibrance: 50,
  hue_deg: 0,
  color_tag: "",
};

export default function PresetEditor({ monitors, editPreset, onClose, onSaved }: Props) {
  const isEditing = editPreset !== null;
  const [form, setForm] = useState<PresetInput>(DEFAULT_INPUT);
  const [errors, setErrors] = useState<ValidationErrors>({});
  const [saving, setSaving] = useState(false);
  const [iccStatus, setIccStatus] = useState<{ hash: string; filename: string } | null>(null);
  const [iccImporting, setIccImporting] = useState(false);
  const [captureMonitorId, setCaptureMonitorId] = useState<string>(() => {
    return monitors.find((m) => m.connected)?.edid_id ?? "";
  });
  const [nvcpImporting, setNvcpImporting] = useState(false);
  const [nvSupported, setNvSupported] = useState<boolean | null>(null);
  const [tagFocused, setTagFocused] = useState(false);
  const [tagDraft, setTagDraft] = useState("");

  // Follow stored tag while not editing the hex field.
  useEffect(() => {
    if (!tagFocused) setTagDraft(form.color_tag ?? "");
  }, [form.color_tag, tagFocused]);

  // Populate form when editing
  useEffect(() => {
    if (editPreset) {
      setForm({
        name: editPreset.name,
        brightness: editPreset.brightness,
        contrast: editPreset.contrast,
        rgb_gains: editPreset.rgb_gains,
        gamma: editPreset.gamma,
        vibrance: editPreset.vibrance,
        hue_deg: editPreset.hue_deg,
        color_tag: editPreset.color_tag ?? "",
      });
      if (editPreset.icc_hash) {
        setIccStatus({ hash: editPreset.icc_hash, filename: editPreset.icc_filename });
      }
    } else {
      setForm(DEFAULT_INPUT);
      setIccStatus(null);
    }
    setErrors({});
    setIccImporting(false);
  }, [editPreset]); // eslint-disable-line react-hooks/exhaustive-deps

  // Probe NVAPI vibrance/hue support when capture monitor selection changes
  useEffect(() => {
    let cancelled = false;
    setNvSupported(null);
    if (!captureMonitorId) {
      return;
    }
    vibranceSupported(captureMonitorId)
      .then((ok) => { if (!cancelled) setNvSupported(ok); })
      .catch(() => { if (!cancelled) setNvSupported(false); });
    return () => { cancelled = true; };
  }, [captureMonitorId]);

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
            <Label htmlFor="preset-name" className="text-xs uppercase tracking-widest mb-1.5 mono">
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
            {errors.name && <p className="mt-1 text-xs text-destructive validation-slide">{errors.name}</p>}
          </div>

          {/* NVCP capture source */}
          <div>
            <Label className="text-xs uppercase tracking-widest mb-1.5 mono">
              NVCP Capture Source <span className="text-muted-foreground font-normal normal-case">(optional, import only)</span>
            </Label>
            <div className="flex items-center gap-2">
              <Select value={captureMonitorId} onValueChange={(v: string | null) => setCaptureMonitorId(v ?? "")}>
                <SelectTrigger className="min-w-0 flex-1">
                  {/* Explicit label: Base UI falls back to the raw edid_id
                      when the selected monitor has no mounted item
                      (e.g. disconnected mid-session), so resolve the
                      display name from the FULL monitor list here. */}
                  <SelectValue>
                    {(v: string | null) => {
                      if (!v) return "— SELECT SOURCE —";
                      const m = monitors.find((mon) => mon.edid_id === v);
                      return m ? (m.alias || m.model || m.device_name) : v;
                    }}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="">— SELECT SOURCE —</SelectItem>
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
            {errors.nvcp && <p className="mt-1 text-xs text-destructive validation-slide">{errors.nvcp}</p>}
            {connectedMonitors.length === 0 && (
              <p className="mt-1 text-xs text-destructive">No connected monitors detected.</p>
            )}
          </div>

          {/* ICC file picker */}
          <div>
            <Label className="text-xs uppercase tracking-widest mb-1.5 mono">
              ICC Profile <span className="text-muted-foreground font-normal normal-case">(optional)</span>
            </Label>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm" className="shrink-0" onClick={handleBrowseIcc} disabled={iccImporting}>
                {iccImporting ? "Importing…" : "Browse…"}
              </Button>
              {iccStatus ? (
                <span className="min-w-0 flex-1 text-xs text-primary truncate mono icc-pop" title={iccStatus.filename}>{iccStatus.filename}</span>
              ) : (
                <span className="min-w-0 flex-1 text-xs text-muted-foreground mono truncate">No ICC profile selected</span>
              )}
            </div>
            {errors.icc_path && <p className="mt-1 text-xs text-destructive validation-slide">{errors.icc_path}</p>}
          </div>

          {/* Tag color — fully custom: native picker + hex field */}
          <div>
            <Label className="text-xs uppercase tracking-widest mb-1.5 mono">
              Tag Color <span className="text-muted-foreground font-normal normal-case">(optional, tints the whole card)</span>
            </Label>
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
                className="min-w-0 flex-1 bg-transparent p-0 text-xs mono outline-none border-b border-transparent text-muted-foreground hover:text-foreground focus:text-foreground focus:border-primary"
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
            {errors.color_tag && <p className="mt-1 text-xs text-destructive validation-slide">{errors.color_tag}</p>}
          </div>
          </div>

          {/* Right column — sliders — 2-col mini-grid at sm+ */}
          <div className="flex flex-col gap-4 min-w-0 sm:grid sm:grid-cols-2 sm:gap-3">
          {/* Gamma | Brightness — side by side */}
          <SliderField
            label="Gamma"
            min={0.3} max={2.8} step={0.05}
            value={form.gamma}
            display={form.gamma.toFixed(2)}
            onChange={(v) => updateField("gamma", v)}
            markers={[{ value: 0.3, label: "0.3" }, { value: 1.0, label: "1.0 (neutral)" }, { value: 2.8, label: "2.8" }]}
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
            disabled={nvSupported === false}
          />

          {/* RGB Gains — full width */}
          <div className="sm:col-span-2">
            <Label className="text-xs uppercase tracking-widest mb-2 mono">
              RGB Gains <span className="text-muted-foreground font-normal normal-case">(1.0 = neutral)</span>
            </Label>
            <div className="grid grid-cols-3 gap-3">
              {(["R", "G", "B"] as const).map((channel, idx) => {
                return (
                  <div key={channel}>
                    <div className="mb-1">
                      <span className="text-xs mono text-foreground">{channel}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <Slider
                        min={0} max={2} step={0.05}
                        value={[form.rgb_gains[idx]]}
                        onValueChange={([v]) => {
                          const newGains = [...form.rgb_gains] as [number, number, number];
                          newGains[idx] = v;
                          updateField("rgb_gains", newGains);
                        }}
                        className="flex-1"
                      />
                      <EditableNumber
                        label={`${channel} gain`}
                        min={0} max={2} step={0.05}
                        display={form.rgb_gains[idx].toFixed(1)}
                        onCommit={(v) => {
                          const newGains = [...form.rgb_gains] as [number, number, number];
                          newGains[idx] = v;
                          updateField("rgb_gains", newGains);
                        }}
                        className="w-10 shrink-0"
                      />
                    </div>
                  </div>
                );
              })}
            </div>
            {errors.rgb_gains && <p className="mt-1 text-xs text-destructive">{errors.rgb_gains}</p>}
          </div>

          {/* Hue — full width */}
          <div className="sm:col-span-2">
          <SliderField
            label="Hue"
            min={0} max={359} step={1}
            value={form.hue_deg}
            display={`${form.hue_deg.toFixed(0)}°`}
            onChange={(v) => updateField("hue_deg", v)}
            markers={[{ value: 0, label: "0" }, { value: 359, label: "359" }]}
            error={errors.hue_deg}
            disabled={nvSupported === false}
          />
          {nvSupported === false && (
            <p className="text-xs text-destructive">Digital vibrance/hue need an NVIDIA-driven display.</p>
          )}
          </div>

          {/* Precedence footnote */}
          <p className="sm:col-span-2 text-xs text-muted-foreground leading-relaxed mono">
            <span className="font-medium uppercase tracking-widest text-muted-foreground">Precedence:</span>
            ICC profile first, then gamma + RGB gains overlaid.
          </p>
          </div>

          {/* Actions — full width */}
          <DialogFooter className="sm:col-span-2">
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
      title={`${label}: ${display} (click to edit, range ${min}–${max})`}
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
      className={`bg-transparent p-0 text-right text-xs mono outline-none border-b border-transparent text-muted-foreground hover:text-foreground focus:text-foreground focus:border-primary disabled:pointer-events-none disabled:opacity-50 ${className ?? ""}`}
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
  label, min, max, step, value, display, onChange, markers, error, disabled,
}: {
  label: string;
  min: number; max: number; step: number;
  value: number;
  display: string;
  onChange: (v: number) => void;
  markers: SliderMarker[];
  error?: string;
  disabled?: boolean;
}) {
  return (
    <div>
      <div className="mb-1.5">
        <Label className="text-xs uppercase tracking-widest mono">
          {label}
        </Label>
      </div>
      <div className="flex items-center gap-3">
        <Slider
          min={min}
          max={max}
          step={step}
          value={[value]}
          disabled={disabled}
          onValueChange={([v]) => onChange(v)}
          className="flex-1"
        />
        <EditableNumber
          label={label}
          min={min} max={max} step={step}
          display={display}
          onCommit={onChange}
          disabled={disabled}
          className="w-12 shrink-0"
        />
      </div>
      {/* Markers sit exactly under their track positions: the row is
          inset by the readout column (w-12 + gap-3 = 60px) so its center
          matches the track center, and each label is centered on the
          thumb-center position for its value (8px half-thumb insets). */}
      <div className="relative mt-0.5 h-4 mono text-xs text-muted-foreground" style={{ marginRight: "60px" }}>
        {markers.map((m) => {
          const frac = Math.min(1, Math.max(0, (m.value - min) / (max - min)));
          return (
            <span
              key={m.label}
              className="absolute whitespace-nowrap"
              style={{ left: `calc(8px + ${frac} * (100% - 16px))`, transform: "translateX(-50%)" }}
            >
              {m.label}
            </span>
          );
        })}
      </div>
      {error && <p className="mt-1 text-xs text-destructive validation-slide">{error}</p>}
    </div>
  );
}