import { useState, useEffect, useRef, type FormEvent } from "react";
import type { Monitor, Preset, PresetInput } from "../lib/types";
import { createPreset, updatePreset, importIcc } from "../lib/tauri";
import { validatePresetForm, type ValidationErrors } from "../lib/validation";

interface Props {
  monitors: Monitor[];
  editPreset: Preset | null; // null = create new, non-null = editing
  onClose: () => void;
  onSaved: () => void;
}

const DEFAULT_INPUT: PresetInput = {
  name: "",
  edid_id: "",
  brightness: 1.0,
  contrast: 0.5,
  rgb_gains: [1.0, 1.0, 1.0],
  gamma: 2.2,
};

export default function PresetEditor({ monitors, editPreset, onClose, onSaved }: Props) {
  const isEditing = editPreset !== null;
  const [form, setForm] = useState<PresetInput>(DEFAULT_INPUT);
  const [errors, setErrors] = useState<ValidationErrors>({});
  const [saving, setSaving] = useState(false);
  const [iccStatus, setIccStatus] = useState<{ hash: string; filename: string } | null>(null);
  const [iccImporting, setIccImporting] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

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
      });
      if (editPreset.icc_hash) {
        setIccStatus({ hash: editPreset.icc_hash, filename: editPreset.icc_filename });
      }
    } else {
      setForm(DEFAULT_INPUT);
      setIccStatus(null);
      // Pre-select first connected monitor
      const firstConnected = monitors.find((m) => m.connected);
      if (firstConnected) {
        setForm((prev) => ({ ...prev, edid_id: firstConnected.edid_id }));
      }
    }
    setErrors({});
    setIccImporting(false);
  }, [editPreset, monitors]);

  const handleFilePick = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    // In a Tauri webview, File.path gives the real filesystem path
    const filePath = (file as File & { path?: string }).path;
    if (!filePath) {
      setErrors((prev) => ({ ...prev, icc_path: "Cannot access file path in browser preview. Run with `npm run tauri dev`." }));
      return;
    }

    setIccImporting(true);
    try {
      const hash = await importIcc(filePath);
      setIccStatus({ hash, filename: file.name });
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
    // Clear error on change
    if (errors[key as keyof ValidationErrors]) {
      setErrors((prev) => {
        const { [key as keyof ValidationErrors]: _, ...rest } = prev;
        return rest;
      });
    }
  };

  const connectedMonitors = monitors.filter((m) => m.connected);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="w-full max-w-lg mx-4 rounded-xl border border-neutral-700 bg-neutral-900 shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-neutral-800">
          <h2 className="text-base font-semibold text-neutral-200">
            {isEditing ? `Edit: ${editPreset?.name}` : "Create Preset"}
          </h2>
          <button
            onClick={onClose}
            className="p-1 rounded-md text-neutral-500 hover:text-neutral-300 hover:bg-neutral-800 transition-colors"
          >
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="px-6 py-5 space-y-5">
          {/* Name */}
          <div>
            <label className="block text-xs font-medium text-neutral-400 mb-1.5">Name</label>
            <input
              type="text"
              value={form.name}
              onChange={(e) => updateField("name", e.target.value)}
              placeholder="My color preset"
              className={`w-full px-3 py-2 text-sm rounded-lg border bg-neutral-800 text-neutral-200 placeholder-neutral-600 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 transition-colors ${
                errors.name ? "border-red-500/50" : "border-neutral-700"
              }`}
            />
            {errors.name && <p className="mt-1 text-xs text-red-400">{errors.name}</p>}
          </div>

          {/* Monitor select */}
          <div>
            <label className="block text-xs font-medium text-neutral-400 mb-1.5">Monitor</label>
            <select
              value={form.edid_id}
              onChange={(e) => updateField("edid_id", e.target.value)}
              className={`w-full px-3 py-2 text-sm rounded-lg border bg-neutral-800 text-neutral-200 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 transition-colors ${
                errors.name && !form.edid_id ? "border-red-500/50" : "border-neutral-700"
              }`}
            >
              <option value="">— Select monitor —</option>
              {connectedMonitors.map((m) => (
                <option key={m.edid_id} value={m.edid_id}>
                  {m.model || m.device_name} {m.serial ? `(${m.serial})` : ""}
                </option>
              ))}
            </select>
            {connectedMonitors.length === 0 && (
              <p className="mt-1 text-xs text-amber-400">No connected monitors detected.</p>
            )}
          </div>

          {/* ICC file picker */}
          <div>
            <label className="block text-xs font-medium text-neutral-400 mb-1.5">
              ICC Profile <span className="text-neutral-600 font-normal">(optional)</span>
            </label>
            <div className="flex items-center gap-2">
              <input
                ref={fileInputRef}
                type="file"
                accept=".icc,.icm"
                onChange={handleFilePick}
                className="hidden"
              />
              <button
                type="button"
                onClick={() => fileInputRef.current?.click()}
                disabled={iccImporting}
                className="px-3 py-2 text-xs font-medium rounded-lg bg-neutral-800 hover:bg-neutral-700 text-neutral-300 border border-neutral-700 transition-colors disabled:opacity-50"
              >
                {iccImporting ? "Importing…" : "Browse…"}
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
            <div className="flex items-center justify-between mb-1.5">
              <label className="text-xs font-medium text-neutral-400">Gamma</label>
              <span className="text-xs text-neutral-500 font-mono">{form.gamma.toFixed(2)}</span>
            </div>
            <input
              type="range"
              min={1.0}
              max={3.0}
              step={0.05}
              value={form.gamma}
              onChange={(e) => updateField("gamma", parseFloat(e.target.value))}
              className="w-full h-1.5 rounded-lg appearance-none cursor-pointer bg-neutral-700 accent-indigo-500"
            />
            <div className="flex justify-between text-xs text-neutral-600 mt-0.5">
              <span>1.0</span>
              <span>3.0</span>
            </div>
            {errors.gamma && <p className="mt-1 text-xs text-red-400">{errors.gamma}</p>}
          </div>

          {/* Brightness slider */}
          <div>
            <div className="flex items-center justify-between mb-1.5">
              <label className="text-xs font-medium text-neutral-400">Brightness</label>
              <span className="text-xs text-neutral-500 font-mono">{form.brightness.toFixed(2)}</span>
            </div>
            <input
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={form.brightness}
              onChange={(e) => updateField("brightness", parseFloat(e.target.value))}
              className="w-full h-1.5 rounded-lg appearance-none cursor-pointer bg-neutral-700 accent-indigo-500"
            />
            <div className="flex justify-between text-xs text-neutral-600 mt-0.5">
              <span>0</span>
              <span>1</span>
            </div>
            {errors.brightness && <p className="mt-1 text-xs text-red-400">{errors.brightness}</p>}
          </div>

          {/* Contrast slider */}
          <div>
            <div className="flex items-center justify-between mb-1.5">
              <label className="text-xs font-medium text-neutral-400">Contrast</label>
              <span className="text-xs text-neutral-500 font-mono">{form.contrast.toFixed(2)}</span>
            </div>
            <input
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={form.contrast}
              onChange={(e) => updateField("contrast", parseFloat(e.target.value))}
              className="w-full h-1.5 rounded-lg appearance-none cursor-pointer bg-neutral-700 accent-indigo-500"
            />
            <div className="flex justify-between text-xs text-neutral-600 mt-0.5">
              <span>0</span>
              <span>1</span>
            </div>
            {errors.contrast && <p className="mt-1 text-xs text-red-400">{errors.contrast}</p>}
          </div>

          {/* RGB gains */}
          <div>
            <label className="block text-xs font-medium text-neutral-400 mb-2">RGB Gains</label>
            <div className="grid grid-cols-3 gap-3">
              {(["R", "G", "B"] as const).map((channel, idx) => (
                <div key={channel}>
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-medium" style={{ color: channel === "R" ? "#f87171" : channel === "G" ? "#4ade80" : "#60a5fa" }}>
                      {channel}
                    </span>
                    <span className="text-xs text-neutral-500 font-mono">{form.rgb_gains[idx].toFixed(1)}</span>
                  </div>
                  <input
                    type="range"
                    min={0}
                    max={2}
                    step={0.05}
                    value={form.rgb_gains[idx]}
                    onChange={(e) => {
                      const newGains = [...form.rgb_gains] as [number, number, number];
                      newGains[idx] = parseFloat(e.target.value);
                      updateField("rgb_gains", newGains);
                    }}
                    className="w-full h-1.5 rounded-lg appearance-none cursor-pointer bg-neutral-700 accent-indigo-500"
                  />
                </div>
              ))}
            </div>
            {errors.rgb_gains && <p className="mt-1 text-xs text-red-400">{errors.rgb_gains}</p>}
          </div>

          {/* Precedence hint */}
          <div className="rounded-lg bg-indigo-950/30 border border-indigo-900/40 px-4 py-3">
            <p className="text-xs text-indigo-300 leading-relaxed">
              <strong>Apply precedence:</strong> ICC profile is applied first, then gamma and RGB
              gains are overlaid on top. This means the gamma curve and RGB multipliers will adjust
              the image <em>after</em> the ICC profile has been installed.
            </p>
          </div>

          {/* Actions */}
          <div className="flex items-center justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm font-medium rounded-lg text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800 transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={saving}
              className={`px-6 py-2 text-sm font-medium rounded-lg transition-colors ${
                saving
                  ? "bg-indigo-800 text-indigo-300 cursor-wait"
                  : "bg-indigo-600 hover:bg-indigo-500 text-white"
              }`}
            >
              {saving ? "Saving…" : isEditing ? "Update" : "Create"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}