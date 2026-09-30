import { useState, useEffect, type FormEvent } from "react";
import type { Monitor, Preset, PresetInput } from "../lib/types";
import { createPreset, updatePreset, importIcc } from "../lib/tauri";
import { vibranceSupported, captureNvcp } from "../lib/tauri";
import { validatePresetForm, type ValidationErrors } from "../lib/validation";
import { open } from "@tauri-apps/plugin-dialog";

interface Props {
  monitors: Monitor[];
  editPreset: Preset | null;
  defaultMonitorId?: string | null;
  onClose: () => void;
  onSaved: () => void;
}

const DEFAULT_INPUT: PresetInput = {
  name: "",
  edid_id: "",
  brightness: 50,
  contrast: 50,
  rgb_gains: [1.0, 1.0, 1.0],
  gamma: 1.0,
  vibrance: 50,
  hue_deg: 0,
};

export default function PresetEditor({ monitors, editPreset, defaultMonitorId, onClose, onSaved }: Props) {
  const isEditing = editPreset !== null;
  const [form, setForm] = useState<PresetInput>(DEFAULT_INPUT);
  const [errors, setErrors] = useState<ValidationErrors>({});
  const [saving, setSaving] = useState(false);
  const [iccStatus, setIccStatus] = useState<{ hash: string; filename: string } | null>(null);
  const [iccImporting, setIccImporting] = useState(false);
  const [nvcpImporting, setNvcpImporting] = useState(false);
  const [nvSupported, setNvSupported] = useState<boolean | null>(null);

  // Populate form when editing
  useEffect(() => {
    if (editPreset) {
      setForm({
        name: editPreset.name,
        edid_id: editPreset.edid_id,
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
      // Pre-select defaultMonitorId if provided, else first connected monitor
      const targetId = defaultMonitorId || "";
      const target = targetId
        ? monitors.find((m) => m.edid_id === targetId)
        : monitors.find((m) => m.connected);
      if (target) {
        setForm((prev) => ({ ...prev, edid_id: target.edid_id }));
      }
    }
    setErrors({});
    setIccImporting(false);
  }, [editPreset, monitors]); // eslint-disable-line react-hooks/exhaustive-deps

  // Probe NVAPI vibrance/hue support when monitor selection changes
  useEffect(() => {
    let cancelled = false;
    setNvSupported(null);
    if (!form.edid_id) {
      setNvSupported(false);
      return;
    }
    vibranceSupported(form.edid_id)
      .then((ok) => { if (!cancelled) setNvSupported(ok); })
      .catch(() => { if (!cancelled) setNvSupported(false); });
    return () => { cancelled = true; };
  }, [form.edid_id]);

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
    if (!form.edid_id) return;
    setNvcpImporting(true);
    setErrors((prev) => {
      const { nvcp: _, ...rest } = prev;
      return rest;
    });
    try {
      const state = await captureNvcp(form.edid_id);
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
    <div className="fixed inset-0 z-50 flex overflow-y-auto bg-neutral-950/80 p-4">
      <div className="m-auto w-full max-w-lg border-2 border-neutral-200 bg-neutral-900 shadow-[6px_6px_0px_#a3e635] motion-reduce:shadow-[3px_3px_0px_#a3e635]">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b-2 border-neutral-200">
          <h2 className="min-w-0 flex-1 truncate text-base font-semibold text-neutral-200">
            {isEditing ? `EDIT: ${editPreset?.name}` : "CREATE PRESET"}
          </h2>
          <button
            onClick={onClose}
            className="brutalist-btn p-1.5 border-2 border-neutral-200 bg-neutral-800 text-neutral-500 hover:bg-neutral-700 hover:text-neutral-200"
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
            <label className="block text-[10px] font-medium text-neutral-500 uppercase tracking-widest mb-1.5">
              Name
            </label>
            <input
              type="text"
              value={form.name}
              onChange={(e) => updateField("name", e.target.value)}
              placeholder="My color preset"
              className={`w-full px-3 py-2 text-sm border-2 bg-neutral-800 text-neutral-200 placeholder-neutral-600 focus-visible:outline-2 focus-visible:outline-lime-400 focus-visible:outline-offset-2 ${
                errors.name ? "border-red-400" : "border-neutral-200"
              }`}
            />
            {errors.name && <p className="mt-1 text-xs text-red-400">{errors.name}</p>}
          </div>

          {/* Monitor select + NVCP capture */}
          <div>
            <label className="block text-[10px] font-medium text-neutral-500 uppercase tracking-widest mb-1.5">
              Monitor
            </label>
            <div className="flex items-center gap-2">
              <select
                value={form.edid_id}
                onChange={(e) => updateField("edid_id", e.target.value)}
                className={`flex-1 px-3 py-2 text-sm border-2 bg-neutral-800 text-neutral-200 focus-visible:outline-2 focus-visible:outline-lime-400 focus-visible:outline-offset-2 ${
                  errors.name && !form.edid_id ? "border-red-400" : "border-neutral-200"
                }`}
              >
                <option value="">— SELECT MONITOR —</option>
                {connectedMonitors.map((m) => (
                  <option key={m.edid_id} value={m.edid_id}>
                    {m.alias || m.model || m.device_name} {m.serial ? `(${m.serial})` : ""}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={handleCaptureNvcp}
                disabled={!form.edid_id || nvcpImporting}
                className="brutalist-btn shrink-0 px-3 py-2 text-xs font-medium border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-neutral-800 text-neutral-300 hover:bg-neutral-700 disabled:opacity-50"
              >
                {nvcpImporting ? "IMPORTING…" : "IMPORT NVCP"}
              </button>
            </div>
            {errors.nvcp && <p className="mt-1 text-xs text-red-400">{errors.nvcp}</p>}
            {connectedMonitors.length === 0 && (
              <p className="mt-1 text-xs text-amber-400">No connected monitors detected.</p>
            )}
          </div>

          {/* ICC file picker */}
          <div>
            <label className="block text-[10px] font-medium text-neutral-500 uppercase tracking-widest mb-1.5">
              ICC Profile <span className="text-neutral-600 font-normal normal-case">(optional)</span>
            </label>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleBrowseIcc}
                disabled={iccImporting}
                className="brutalist-btn px-3 py-2 text-xs font-medium border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-neutral-800 text-neutral-300 hover:bg-neutral-700 disabled:opacity-50"
              >
                {iccImporting ? "IMPORTING…" : "BROWSE…"}
              </button>
              {iccStatus ? (
                <span className="text-xs text-emerald-400 truncate">{iccStatus.filename}</span>
              ) : (
                <span className="text-xs text-neutral-600">No ICC profile selected</span>
              )}
            </div>
            {errors.icc_path && <p className="mt-1 text-xs text-red-400">{errors.icc_path}</p>}
          </div>

          {/* Gamma slider */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-neutral-500 uppercase tracking-widest">Gamma</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={1.0} max={3.0} step={0.05} value={form.gamma}
                onChange={(e) => updateField("gamma", parseFloat(e.target.value))}
                className="min-w-0 flex-1"
              />
              <span className="w-12 shrink-0 text-right text-xs text-neutral-300 font-mono">{form.gamma.toFixed(2)}</span>
            </div>
            <div className="flex justify-between text-xs text-neutral-600 mt-0.5">
              <span className="font-mono">1.0</span>
              <span className="font-mono">3.0</span>
            </div>
            {errors.gamma && <p className="mt-1 text-xs text-red-400">{errors.gamma}</p>}
          </div>

          {/* Brightness slider */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-neutral-500 uppercase tracking-widest">Brightness</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={0} max={100} step={1} value={form.brightness}
                onChange={(e) => updateField("brightness", parseFloat(e.target.value))}
                className="min-w-0 flex-1"
              />
              <span className="w-12 shrink-0 text-right text-xs text-neutral-300 font-mono">{form.brightness.toFixed(0)}</span>
            </div>
            <div className="flex justify-between text-xs text-neutral-600 mt-0.5">
              <span className="font-mono">0</span>
              <span className="font-mono">50</span>
              <span className="font-mono">100</span>
            </div>
            {errors.brightness && <p className="mt-1 text-xs text-red-400">{errors.brightness}</p>}
          </div>

          {/* Contrast slider */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-neutral-500 uppercase tracking-widest">Contrast</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={0} max={100} step={1} value={form.contrast}
                onChange={(e) => updateField("contrast", parseFloat(e.target.value))}
                className="min-w-0 flex-1"
              />
              <span className="w-12 shrink-0 text-right text-xs text-neutral-300 font-mono">{form.contrast.toFixed(0)}</span>
            </div>
            <div className="flex justify-between text-xs text-neutral-600 mt-0.5">
              <span className="font-mono">0</span>
              <span className="font-mono">50</span>
              <span className="font-mono">100</span>
            </div>
            {errors.contrast && <p className="mt-1 text-xs text-red-400">{errors.contrast}</p>}
          </div>

          {/* RGB gains */}
          <div>
            <label className="block text-[10px] font-medium text-neutral-500 uppercase tracking-widest mb-2">
              RGB Gains <span className="text-neutral-600 font-normal normal-case">(1.0 = neutral)</span>
            </label>
            <div className="grid grid-cols-3 gap-3">
              {(["R", "G", "B"] as const).map((channel, idx) => (
                <div key={channel}>
                  <div className="mb-1">
                    <span className="text-xs font-mono" style={{ color: channel === "R" ? "#f87171" : channel === "G" ? "#4ade80" : "#60a5fa" }}>
                      {channel}
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    <input
                      type="range" min={0} max={2} step={0.05} value={form.rgb_gains[idx]}
                      onChange={(e) => {
                        const newGains = [...form.rgb_gains] as [number, number, number];
                        newGains[idx] = parseFloat(e.target.value);
                        updateField("rgb_gains", newGains);
                      }}
                      className="min-w-0 flex-1"
                    />
                    <span className="w-10 shrink-0 text-right text-xs text-neutral-500 font-mono">{form.rgb_gains[idx].toFixed(1)}</span>
                  </div>
                </div>
              ))}
            </div>
            {errors.rgb_gains && <p className="mt-1 text-xs text-red-400">{errors.rgb_gains}</p>}
          </div>

          {/* Vibrance slider */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-neutral-500 uppercase tracking-widest">Digital Vibrance</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={0} max={100} step={1} value={form.vibrance}
                disabled={nvSupported === false}
                onChange={(e) => updateField("vibrance", parseFloat(e.target.value))}
                className="min-w-0 flex-1"
              />
              <span className="w-12 shrink-0 text-right text-xs text-neutral-300 font-mono">{form.vibrance.toFixed(0)}</span>
            </div>
            <div className="flex justify-between text-xs text-neutral-600 mt-0.5">
              <span className="font-mono">0</span>
              <span className="font-mono">50</span>
              <span className="font-mono">100</span>
            </div>
            {errors.vibrance && <p className="mt-1 text-xs text-red-400">{errors.vibrance}</p>}
          </div>

          {/* Hue slider */}
          <div>
            <div className="mb-1.5">
              <label className="text-[10px] font-medium text-neutral-500 uppercase tracking-widest">Hue</label>
            </div>
            <div className="flex items-center gap-3">
              <input
                type="range" min={0} max={359} step={1} value={form.hue_deg}
                disabled={nvSupported === false}
                onChange={(e) => updateField("hue_deg", parseFloat(e.target.value))}
                className="min-w-0 flex-1"
              />
              <span className="w-12 shrink-0 text-right text-xs text-neutral-300 font-mono">{form.hue_deg.toFixed(0)}°</span>
            </div>
            <div className="flex justify-between text-xs text-neutral-600 mt-0.5">
              <span className="font-mono">0</span>
              <span className="font-mono">359</span>
            </div>
            {errors.hue_deg && <p className="mt-1 text-xs text-red-400">{errors.hue_deg}</p>}
            {nvSupported === false && (
              <p className="mt-1 text-xs text-amber-400">Digital vibrance/hue need an NVIDIA-driven display.</p>
            )}
          </div>

          {/* Precedence hint */}
          <div className="border-2 border-lime-400 px-4 py-3 bg-neutral-800">
            <p className="text-xs text-lime-300 leading-relaxed">
              <strong>APPLY PRECEDENCE:</strong> ICC profile is applied first, then gamma and RGB
              gains are overlaid on top. This means the gamma curve and RGB multipliers will adjust
              the image <em>after</em> the ICC profile has been installed.
            </p>
          </div>

          {/* Actions */}
          <div className="flex flex-wrap items-center justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="brutalist-btn px-4 py-2 text-sm font-medium border-2 border-neutral-200 bg-neutral-800 text-neutral-400 hover:bg-neutral-700 hover:text-neutral-200"
            >
              CANCEL
            </button>
            <button
              type="submit"
              disabled={saving}
              className={`brutalist-btn px-6 py-2 text-sm font-medium border-2 border-lime-400 shadow-[2px_2px_0px_#a3e635] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 ${
                saving
                  ? "bg-lime-800 text-lime-300 cursor-wait"
                  : "bg-lime-400 text-black hover:bg-lime-300"
              }`}
            >
              {saving ? "SAVING…" : isEditing ? "UPDATE" : "CREATE"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}