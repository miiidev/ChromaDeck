import { useState, useEffect, type FormEvent, type CSSProperties } from "react";

/** Fill-track style for range inputs: `--fill` percent + `--fill-color`. */
function trackFill(value: number, min: number, max: number, color = "#111"): CSSProperties {
  const pct = Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100));
  return { "--fill": `${pct}%`, "--fill-color": color } as CSSProperties;
}
import type { Monitor, Preset, PresetInput } from "../lib/types";
import { createPreset, updatePreset, importIcc } from "../lib/tauri";
import { vibranceSupported, captureNvcp } from "../lib/tauri";
import { validatePresetForm, type ValidationErrors } from "../lib/validation";
import { open } from "@tauri-apps/plugin-dialog";

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
    <div className="fixed inset-0 z-50 flex overflow-y-auto bg-ink/70 p-4">
      {/* Paper bg, 2px ink border, 6px shadow — Bauhaus modal */}
      <div className="m-auto w-full max-w-lg border-2 border-ink bg-surface shadow-modal motion-reduce:shadow-[3px_3px_0px_var(--shadow-clr)]">
        {/* Header: primary block bar + title */}
        <div className="flex items-center justify-between px-6 py-4 border-b-2 border-ink">
          <h2 className="min-w-0 flex-1 truncate text-base font-semibold text-ink font-heading">
            {isEditing ? `EDIT: ${editPreset?.name}` : "CREATE PRESET"}
          </h2>
          <button
            onClick={onClose}
            className="bauhaus-btn p-1.5 border-2 border-ink bg-surface text-secondary hover:bg-surface-hover"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="px-6 py-5 space-y-5">
          {/* Name */}
          <div>
            <label className="block text-[10px] font-medium text-secondary uppercase tracking-widest mb-1.5">
              Name
            </label>
            <input
              type="text"
              value={form.name}
              onChange={(e) => updateField("name", e.target.value)}
              placeholder="My color preset"
              className={`w-full px-3 py-2 text-sm border-2 bg-surface text-ink placeholder-muted focus-visible:outline-2 focus-visible:outline-blue focus-visible:outline-offset-2 ${
                errors.name ? "border-red" : "border-ink"
              }`}
            />
            {errors.name && <p className="mt-1 text-xs text-red">{errors.name}</p>}
          </div>

          {/* NVCP capture source */}
          <div>
            <label className="block text-[10px] font-medium text-secondary uppercase tracking-widest mb-1.5">
              NVCP Capture Source <span className="text-muted font-normal normal-case">(optional, import only)</span>
            </label>
            <div className="flex items-center gap-2">
              <select
                value={captureMonitorId}
                onChange={(e) => setCaptureMonitorId(e.target.value)}
                className="flex-1 px-3 py-2 text-sm border-2 border-ink bg-surface text-ink focus-visible:outline-2 focus-visible:outline-blue focus-visible:outline-offset-2"
              >
                <option value="">— SELECT SOURCE —</option>
                {connectedMonitors.map((m) => (
                  <option key={m.edid_id} value={m.edid_id}>
                    {m.alias || m.model || m.device_name} {m.serial ? `(${m.serial})` : ""}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={handleCaptureNvcp}
                disabled={!captureMonitorId || nvcpImporting}
                className="bauhaus-btn shrink-0 px-3 py-2 text-xs font-medium border-2 border-ink bg-surface text-secondary hover:bg-surface-hover shadow-btn disabled:opacity-50"
              >
                {nvcpImporting ? "IMPORTING…" : "IMPORT NVCP"}
              </button>
            </div>
            {errors.nvcp && <p className="mt-1 text-xs text-red">{errors.nvcp}</p>}
            {connectedMonitors.length === 0 && (
              <p className="mt-1 text-xs text-yellow">No connected monitors detected.</p>
            )}
          </div>

          {/* ICC file picker */}
          <div>
            <label className="block text-[10px] font-medium text-secondary uppercase tracking-widest mb-1.5">
              ICC Profile <span className="text-muted font-normal normal-case">(optional)</span>
            </label>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleBrowseIcc}
                disabled={iccImporting}
                className="bauhaus-btn px-3 py-2 text-xs font-medium border-2 border-ink bg-surface text-secondary hover:bg-surface-hover shadow-btn disabled:opacity-50"
              >
                {iccImporting ? "IMPORTING…" : "BROWSE…"}
              </button>
              {iccStatus ? (
                <span className="text-xs text-blue truncate">{iccStatus.filename}</span>
              ) : (
                <span className="text-xs text-muted">No ICC profile selected</span>
              )}
            </div>
            {errors.icc_path && <p className="mt-1 text-xs text-red">{errors.icc_path}</p>}
          </div>

          {/* Gamma slider — blue square thumb (data-thumb="square") */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-secondary uppercase tracking-widest">Gamma</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={0.3} max={2.8} step={0.05} value={form.gamma}
                onChange={(e) => updateField("gamma", parseFloat(e.target.value))}
                className="min-w-0 flex-1" data-thumb="square"
                style={trackFill(form.gamma, 0.3, 2.8, "#0066B3")}
              />
              <span className="w-12 shrink-0 text-right text-xs text-secondary font-mono">{form.gamma.toFixed(2)}</span>
            </div>
            <div className="flex justify-between text-xs text-muted mt-0.5">
              <span className="font-mono">0.3</span>
              <span className="font-mono">1.0 (neutral)</span>
              <span className="font-mono">2.8</span>
            </div>
            {errors.gamma && <p className="mt-1 text-xs text-red">{errors.gamma}</p>}
          </div>

          {/* Brightness slider — yellow circle thumb */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-secondary uppercase tracking-widest">Brightness</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={0} max={100} step={1} value={form.brightness}
                onChange={(e) => updateField("brightness", parseFloat(e.target.value))}
                className="min-w-0 flex-1" data-thumb="circle"
                style={trackFill(form.brightness, 0, 100, "#FFCC00")}
              />
              <span className="w-12 shrink-0 text-right text-xs text-secondary font-mono">{form.brightness.toFixed(0)}</span>
            </div>
            <div className="flex justify-between text-xs text-muted mt-0.5">
              <span className="font-mono">0</span>
              <span className="font-mono">50</span>
              <span className="font-mono">100</span>
            </div>
            {errors.brightness && <p className="mt-1 text-xs text-red">{errors.brightness}</p>}
          </div>

          {/* Contrast slider — red triangle thumb */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-secondary uppercase tracking-widest">Contrast</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={0} max={100} step={1} value={form.contrast}
                onChange={(e) => updateField("contrast", parseFloat(e.target.value))}
                className="min-w-0 flex-1" data-thumb="triangle"
                style={trackFill(form.contrast, 0, 100, "#E30613")}
              />
              <span className="w-12 shrink-0 text-right text-xs text-secondary font-mono">{form.contrast.toFixed(0)}</span>
            </div>
            <div className="flex justify-between text-xs text-muted mt-0.5">
              <span className="font-mono">0</span>
              <span className="font-mono">50</span>
              <span className="font-mono">100</span>
            </div>
            {errors.contrast && <p className="mt-1 text-xs text-red">{errors.contrast}</p>}
          </div>

          {/* RGB gains — square thumbs with per-channel color */}
          <div>
            <label className="block text-[10px] font-medium text-secondary uppercase tracking-widest mb-2">
              RGB Gains <span className="text-muted font-normal normal-case">(1.0 = neutral)</span>
            </label>
            <div className="grid grid-cols-3 gap-3">
              {(["R", "G", "B"] as const).map((channel, idx) => {
                const chColor = ["#E30613", "#4ade80", "#0066B3"][idx];
                return (
                  <div key={channel}>
                    <div className="mb-1">
                      <span className="text-xs font-mono text-ink">{channel}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <input
                        type="range" min={0} max={2} step={0.05} value={form.rgb_gains[idx]}
                        onChange={(e) => {
                          const newGains = [...form.rgb_gains] as [number, number, number];
                          newGains[idx] = parseFloat(e.target.value);
                          updateField("rgb_gains", newGains);
                        }}
                        className="min-w-0 flex-1" data-thumb="square"
                        style={trackFill(form.rgb_gains[idx], 0, 2, chColor)}
                      />
                      <span className="w-10 shrink-0 text-right text-xs text-muted font-mono">{form.rgb_gains[idx].toFixed(1)}</span>
                    </div>
                  </div>
                );
              })}
            </div>
            {errors.rgb_gains && <p className="mt-1 text-xs text-red">{errors.rgb_gains}</p>}
          </div>

          {/* Vibrance slider — ink default (square) */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-secondary uppercase tracking-widest">Digital Vibrance</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={0} max={100} step={1} value={form.vibrance}
                disabled={nvSupported === false}
                onChange={(e) => updateField("vibrance", parseFloat(e.target.value))}
                className="min-w-0 flex-1"
                style={trackFill(form.vibrance, 0, 100)}
              />
              <span className="w-12 shrink-0 text-right text-xs text-secondary font-mono">{form.vibrance.toFixed(0)}</span>
            </div>
            <div className="flex justify-between text-xs text-muted mt-0.5">
              <span className="font-mono">0</span>
              <span className="font-mono">50</span>
              <span className="font-mono">100</span>
            </div>
            {errors.vibrance && <p className="mt-1 text-xs text-red">{errors.vibrance}</p>}
          </div>

          {/* Hue slider — ink default (square) */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-secondary uppercase tracking-widest">Hue</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={0} max={359} step={1} value={form.hue_deg}
                disabled={nvSupported === false}
                onChange={(e) => updateField("hue_deg", parseFloat(e.target.value))}
                className="min-w-0 flex-1"
                style={trackFill(form.hue_deg, 0, 359)}
              />
              <span className="w-12 shrink-0 text-right text-xs text-secondary font-mono">{form.hue_deg.toFixed(0)}°</span>
            </div>
            <div className="flex justify-between text-xs text-muted mt-0.5">
              <span className="font-mono">0</span>
              <span className="font-mono">359</span>
            </div>
            {errors.hue_deg && <p className="mt-1 text-xs text-red">{errors.hue_deg}</p>}
            {nvSupported === false && (
              <p className="mt-1 text-xs text-yellow">Digital vibrance/hue need an NVIDIA-driven display.</p>
            )}
          </div>

          {/* Precedence footnote */}
          <p className="mt-2 text-[10px] text-muted leading-relaxed">
            <span className="font-medium uppercase tracking-widest text-secondary">Precedence:</span>
            ICC profile first, then gamma + RGB gains overlaid.
          </p>

          {/* Actions */}
          <div className="flex flex-wrap items-center justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="bauhaus-btn px-4 py-2 text-sm font-medium border-2 border-ink bg-surface text-secondary hover:bg-surface-hover"
            >
              CANCEL
            </button>
            {/* CREATE/UPDATE = blue bg + white text (EDIT role) */}
            <button
              type="submit"
              disabled={saving}
              className={`bauhaus-btn px-6 py-2 text-sm font-medium border-2 border-blue bg-primary-blue shadow-btn active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 ${
                saving ? "opacity-50 cursor-wait" : ""
              }`}
              style={{ color: "white" }}
            >
              {saving ? "SAVING…" : isEditing ? "UPDATE" : "CREATE"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}