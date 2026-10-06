import { useState, useEffect, type FormEvent } from "react";

import type { Monitor, Preset, PresetInput } from "../lib/types";
import { createPreset, updatePreset, importIcc } from "../lib/tauri";
import { vibranceSupported, captureNvcp } from "../lib/tauri";
import { validatePresetForm, type ValidationErrors } from "../lib/validation";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";

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
    <div className="fixed inset-0 isolate z-50 flex overflow-y-auto bg-black/10 p-4">
      {/* Popover-toned panel */}
      <div className="m-auto w-full max-w-lg rounded-xl bg-popover text-popover-foreground ring-1 ring-foreground/10">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-border">
          <h2 className="min-w-0 flex-1 truncate text-base font-medium text-foreground">
            {isEditing ? `EDIT: ${editPreset?.name}` : "CREATE PRESET"}
          </h2>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onClose}
            aria-label="Close editor"
          />
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="px-6 py-5 space-y-5">
          {/* Name */}
          <div>
            <label className="flex items-center gap-2 text-xs leading-none font-medium text-muted-foreground uppercase tracking-widest mb-1.5 mono">
              Name
            </label>
            <input
              type="text"
              value={form.name}
              onChange={(e) => updateField("name", e.target.value)}
              placeholder="My color preset"
              className={`h-8 w-full rounded-lg border ${errors.name ? "border-destructive" : "border-border"} bg-transparent px-2.5 py-1 text-sm placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50`}
            />
            {errors.name && <p className="mt-1 text-xs text-destructive">{errors.name}</p>}
          </div>

          {/* NVCP capture source */}
          <div>
            <label className="flex items-center gap-2 text-xs leading-none font-medium text-muted-foreground uppercase tracking-widest mb-1.5 mono">
              NVCP Capture Source <span className="text-muted-foreground font-normal normal-case">(optional, import only)</span>
            </label>
            <div className="flex items-center gap-2">
              <select
                value={captureMonitorId}
                onChange={(e) => setCaptureMonitorId(e.target.value)}
                className="flex w-fit items-center justify-between gap-1.5 h-8 rounded-lg border border-border bg-transparent py-2 pr-2 pl-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
              >
                <option value="">— SELECT SOURCE —</option>
                {connectedMonitors.map((m) => (
                  <option key={m.edid_id} value={m.edid_id}>
                    {m.alias || m.model || m.device_name} {m.serial ? `(${m.serial})` : ""}
                  </option>
                ))}
              </select>
              <Button
                variant="outline"
                size="sm"
                onClick={handleCaptureNvcp}
                disabled={!captureMonitorId || nvcpImporting}
              >
                {nvcpImporting ? "IMPORTING…" : "IMPORT NVCP"}
              </Button>
            </div>
            {errors.nvcp && <p className="mt-1 text-xs text-destructive">{errors.nvcp}</p>}
            {connectedMonitors.length === 0 && (
              <p className="mt-1 text-xs text-destructive">No connected monitors detected.</p>
            )}
          </div>

          {/* ICC file picker */}
          <div>
            <label className="flex items-center gap-2 text-xs leading-none font-medium text-muted-foreground uppercase tracking-widest mb-1.5 mono">
              ICC Profile <span className="text-muted-foreground font-normal normal-case">(optional)</span>
            </label>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm" onClick={handleBrowseIcc} disabled={iccImporting}>
                {iccImporting ? "IMPORTING…" : "BROWSE…"}
              </Button>
              {iccStatus ? (
                <span className="text-xs text-primary truncate mono">{iccStatus.filename}</span>
              ) : (
                <span className="text-xs text-muted-foreground mono">No ICC profile selected</span>
              )}
            </div>
            {errors.icc_path && <p className="mt-1 text-xs text-destructive">{errors.icc_path}</p>}
          </div>

          {/* Gamma slider */}
          <SliderField
            label="Gamma"
            min={0.3} max={2.8} step={0.05}
            value={form.gamma}
            display={form.gamma.toFixed(2)}
            onChange={(v) => updateField("gamma", v)}
            markers={["0.3", "1.0 (neutral)", "2.8"]}
            error={errors.gamma}
          />

          {/* Brightness slider */}
          <SliderField
            label="Brightness"
            min={0} max={100} step={1}
            value={form.brightness}
            display={form.brightness.toFixed(0)}
            onChange={(v) => updateField("brightness", v)}
            markers={["0", "50", "100"]}
            error={errors.brightness}
          />

          {/* Contrast slider */}
          <SliderField
            label="Contrast"
            min={0} max={100} step={1}
            value={form.contrast}
            display={form.contrast.toFixed(0)}
            onChange={(v) => updateField("contrast", v)}
            markers={["0", "50", "100"]}
            error={errors.contrast}
          />

          {/* RGB gains — three per-channel sliders */}
          <div>
            <label className="flex items-center gap-2 text-xs leading-none font-medium text-muted-foreground uppercase tracking-widest mb-2 mono">
              RGB Gains <span className="text-muted-foreground font-normal normal-case">(1.0 = neutral)</span>
            </label>
            <div className="grid grid-cols-3 gap-3">
              {(["R", "G", "B"] as const).map((channel, idx) => {
                return (
                  <div key={channel}>
                    <div className="mb-1">
                      <span className="text-xs mono text-foreground">{channel}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <input
                        type="range" min={0} max={2} step={0.05} value={form.rgb_gains[idx]}
                        onChange={(e) => {
                          const newGains = [...form.rgb_gains] as [number, number, number];
                          newGains[idx] = parseFloat(e.target.value);
                          updateField("rgb_gains", newGains);
                        }}
                        className="flex-1 h-6 appearance-none cursor-pointer bg-muted rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-primary"
                      />
                      <span className="w-10 shrink-0 text-right text-xs text-muted-foreground mono">{form.rgb_gains[idx].toFixed(1)}</span>
                    </div>
                  </div>
                );
              })}
            </div>
            {errors.rgb_gains && <p className="mt-1 text-xs text-destructive">{errors.rgb_gains}</p>}
          </div>

          {/* Vibrance slider */}
          <SliderField
            label="Digital Vibrance"
            min={0} max={100} step={1}
            value={form.vibrance}
            display={form.vibrance.toFixed(0)}
            onChange={(v) => updateField("vibrance", v)}
            markers={["0", "50", "100"]}
            error={errors.vibrance}
            disabled={nvSupported === false}
          />

          {/* Hue slider */}
          <SliderField
            label="Hue"
            min={0} max={359} step={1}
            value={form.hue_deg}
            display={`${form.hue_deg.toFixed(0)}°`}
            onChange={(v) => updateField("hue_deg", v)}
            markers={["0", "359"]}
            error={errors.hue_deg}
            disabled={nvSupported === false}
          />
          {nvSupported === false && (
            <p className="mt-1 text-xs text-destructive">Digital vibrance/hue need an NVIDIA-driven display.</p>
          )}

          {/* Precedence footnote */}
          <p className="mt-2 text-xs text-muted-foreground leading-relaxed mono">
            <span className="font-medium uppercase tracking-widest text-muted-foreground">Precedence:</span>
            ICC profile first, then gamma + RGB gains overlaid.
          </p>

          {/* Actions */}
          <div className="flex flex-wrap items-center justify-end gap-3 pt-2">
            <Button variant="outline" size="sm" onClick={onClose}>
              CANCEL
            </Button>
            <Button variant="secondary" size="sm" type="submit" disabled={saving}>
              {saving ? "SAVING…" : isEditing ? "UPDATE" : "CREATE"}
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
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
  markers: string[];
  error?: string;
  disabled?: boolean;
}) {
  return (
    <div>
      <div className="mb-1.5">
        <label className="flex items-center gap-2 text-xs leading-none font-medium text-muted-foreground uppercase tracking-widest mono">
          {label}
        </label>
      </div>
      <div className="flex items-center gap-3">
        <input
          type="range" min={min} max={max} step={step} value={value}
          disabled={disabled}
          onChange={(e) => onChange(parseFloat(e.target.value))}
          className="flex-1 h-6 appearance-none cursor-pointer bg-muted rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:opacity-50"
        />
        <span className="w-12 shrink-0 text-right text-xs text-muted-foreground mono">{display}</span>
      </div>
      <div className="flex justify-between text-xs text-muted-foreground mt-0.5 mono">
        {markers.map((m) => <span key={m}>{m}</span>)}
      </div>
      {error && <p className="mt-1 text-xs text-destructive">{error}</p>}
    </div>
  );
}