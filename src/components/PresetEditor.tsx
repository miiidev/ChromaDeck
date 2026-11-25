import { useState, useEffect, useRef, type FormEvent } from "react";

import type { Monitor, Preset, PresetInput } from "../lib/types";
import {
  captureNvcp,
  createPreset,
  getDisplayCapabilities,
  importIcc,
  previewPreset,
  restorePreview,
  updatePreset,
} from "../lib/tauri";
import { validatePresetForm, type ValidationErrors } from "../lib/validation";
import {
  DEFAULT_PRESET_INPUT,
  applyCapturedState,
  clampSliderValue,
  presetToFormValues,
  resetSliderValues,
  withClearedIcc,
  withSelectedIcc,
} from "../lib/presetForm";
import { NEUTRAL } from "../lib/presetCard";
import { addProgram } from "../lib/programMapping";
import { joinCategory, splitCategory } from "../lib/presetCard";
import { applyPreviewPixel, type RgbColor } from "../lib/preview";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Checkbox, CheckboxIndicator } from "@/components/ui/checkbox";
import { Combobox, ComboboxContent, ComboboxInput, ComboboxItem } from "@/components/ui/combobox";
import { Dialog, DialogContent, DialogHeader, DialogFooter, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { PresetTag } from "@/components/ui/preset-tag";
import { Check, ChevronDown, Download, Eye, RotateCcw, Upload, X } from "lucide-react";
import { cn } from "cn";

interface Props {
  presets: Preset[];
  monitors?: Monitor[];
  editPreset: Preset | null;
  onClose: () => void;
  /** Fired after the close animation finishes; the parent unmounts here. */
  onClosed?: () => void;
  onSaved: () => void;
}

/** Live colour-bar preview source swatches (sRGB, 0..1). */
const REFERENCE_SWATCHES: { name: string; rgb: [number, number, number] }[] = [
  { name: "Black", rgb: [0, 0, 0] },
  { name: "Shadow", rgb: [0.18, 0.18, 0.18] },
  { name: "Mid", rgb: [0.5, 0.5, 0.5] },
  { name: "Highlight", rgb: [0.85, 0.85, 0.85] },
  { name: "Red", rgb: [1, 0, 0] },
  { name: "Green", rgb: [0, 1, 0] },
  { name: "Blue", rgb: [0, 0, 1] },
  { name: "Cyan", rgb: [0, 1, 1] },
  { name: "Magenta", rgb: [1, 0, 1] },
  { name: "Yellow", rgb: [1, 1, 0] },
];

const IDENTITY_PREVIEW = {
  brightness: 50,
  contrast: 50,
  gamma: 1,
  rgb_gains: [1, 1, 1] as [number, number, number],
  vibrance: 50,
  saturation: 50,
  hue_deg: 0,
};

/** Curated tag palette + None. Hexes are tuned for the dark surface. */
const TAG_OPTIONS = [
  { label: "None", value: "" },
  { label: "Mint", value: "#5eead4" },
  { label: "Teal", value: "#2dd4bf" },
  { label: "Cyan", value: "#22d3ee" },
  { label: "Sky", value: "#38bdf8" },
  { label: "Blue", value: "#60a5fa" },
  { label: "Indigo", value: "#818cf8" },
  { label: "Violet", value: "#a78bfa" },
  { label: "Magenta", value: "#e879f9" },
  { label: "Pink", value: "#f472b6" },
  { label: "Rose", value: "#fb7185" },
  { label: "Red", value: "#f87171" },
  { label: "Orange", value: "#fb923c" },
  { label: "Amber", value: "#fbbf24" },
  { label: "Lime", value: "#a3e635" },
  { label: "Green", value: "#4ade80" },
  { label: "Slate", value: "#94a3b8" },
] as const;

/** Marker shapes offered in the editor (stored as tag_shape). */
const TAG_SHAPE_OPTIONS = [
  { label: "Pill", value: "pill" },
  { label: "Round", value: "round" },
  { label: "Square", value: "square" },
  { label: "Bar", value: "bar" },
  { label: "Diamond", value: "diamond" },
  { label: "Hexagon", value: "hexagon" },
  { label: "Triangle", value: "triangle" },
  { label: "Ring", value: "ring" },
] as const;

function toHex(c: RgbColor): string {
  const channel = (v: number) =>
    Math.round(Math.max(0, Math.min(1, v)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${channel(c.r)}${channel(c.g)}${channel(c.b)}`;
}

function LiveColorCard({
  label,
  params,
  highlight,
}: {
  label: string;
  params: typeof IDENTITY_PREVIEW;
  highlight?: boolean;
}) {
  return (
    <div
      className={cn(
        "rounded-[8px] border bg-surface p-4",
        highlight ? "border-primary" : "border-border",
      )}
    >
      <div className="mb-3 flex items-center justify-between">
        <span className="font-body text-[12px] uppercase tracking-[0.4px] text-muted-foreground">
          {label}
        </span>
        <span className="mono text-[9px] text-muted-foreground">
          {REFERENCE_SWATCHES.length} SWATCHES
        </span>
      </div>
      <div className="grid grid-cols-10 gap-1">
        {REFERENCE_SWATCHES.map((swatch) => {
          const [r, g, b] = swatch.rgb;
          const c = applyPreviewPixel({ r, g, b }, params);
          return (
            <div
              key={swatch.name}
              title={swatch.name}
              className="h-[40px] rounded-[8px] border border-border"
              style={{ backgroundColor: toHex(c) }}
            />
          );
        })}
      </div>
    </div>
  );
}

export default function PresetEditor({ presets, monitors = [], editPreset, onClose, onClosed, onSaved }: Props) {
  const isEditing = editPreset !== null;
  // The dialog owns its open state so Base UI can run the data-closed exit
  // animation before the parent unmounts it (see onOpenChangeComplete below).
  const [dialogOpen, setDialogOpen] = useState(true);
  const [form, setForm] = useState<PresetInput>(DEFAULT_PRESET_INPUT);
  const [category, setCategory] = useState("");
  const [categoryError, setCategoryError] = useState<string | null>(null);
  const [errors, setErrors] = useState<ValidationErrors>({});
  const [saving, setSaving] = useState(false);
  const [iccStatus, setIccStatus] = useState<{ hash: string; filename: string } | null>(null);
  const [iccImporting, setIccImporting] = useState(false);
  const [programInput, setProgramInput] = useState("");
  const [programError, setProgramError] = useState<string | null>(null);

  // Hardware target for live preview & driver capture
  const connectedMonitors = monitors.filter((m) => m.connected);
  const [targetEdid, setTargetEdid] = useState<string>(
    () => connectedMonitors[0]?.edid_id ?? "",
  );
  const [targetCap, setTargetCap] = useState<{
    status: "idle" | "loading" | "ready" | "error";
    backendId?: string;
    backendLabel?: string;
  }>({ status: "idle" });
  const [capturing, setCapturing] = useState(false);
  const [captureNote, setCaptureNote] = useState<{ text: string; isError: boolean } | null>(null);
  const [livePreview, setLivePreview] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const previewedEdidRef = useRef<string | null>(null);
  const previewTimerRef = useRef<number | null>(null);

  // Keep targetEdid pointing at a connected display when monitor list changes
  useEffect(() => {
    if (connectedMonitors.length === 0) {
      if (targetEdid !== "") setTargetEdid("");
      return;
    }
    if (!connectedMonitors.some((m) => m.edid_id === targetEdid)) {
      setTargetEdid(connectedMonitors[0].edid_id);
    }
  }, [connectedMonitors, targetEdid]);

  // Query backend capability for the selected calibration monitor
  useEffect(() => {
    if (!targetEdid) {
      setTargetCap({ status: "idle" });
      return;
    }
    let cancelled = false;
    setTargetCap({ status: "loading" });
    getDisplayCapabilities(targetEdid)
      .then((caps) => {
        if (!cancelled) {
          setTargetCap({
            status: "ready",
            backendId: caps.backend_id,
            backendLabel: caps.backend_label,
          });
        }
      })
      .catch(() => {
        if (!cancelled) setTargetCap({ status: "error" });
      });
    return () => {
      cancelled = true;
    };
  }, [targetEdid]);

  // Restore any previewed monitor cleanly on toggle-off, target switch, or unmount
  const endLivePreview = async () => {
    if (previewTimerRef.current !== null) {
      window.clearTimeout(previewTimerRef.current);
      previewTimerRef.current = null;
    }
    const edid = previewedEdidRef.current;
    previewedEdidRef.current = null;
    if (edid) {
      try {
        await restorePreview(edid);
      } catch {
        // best-effort hardware restore
      }
    }
  };

  useEffect(
    () => () => {
      if (previewTimerRef.current !== null) {
        window.clearTimeout(previewTimerRef.current);
        previewTimerRef.current = null;
      }
      const edid = previewedEdidRef.current;
      previewedEdidRef.current = null;
      if (edid) {
        void restorePreview(edid).catch(() => {});
      }
    },
    [],
  );

  // Throttled live preview write whenever colour parameters change while active
  useEffect(() => {
    if (!livePreview || !targetEdid) {
      if (previewedEdidRef.current) {
        void endLivePreview();
      }
      setPreviewError(null);
      return;
    }
    // Switching preview monitor restores the previous monitor first
    if (previewedEdidRef.current && previewedEdidRef.current !== targetEdid) {
      const previous = previewedEdidRef.current;
      previewedEdidRef.current = null;
      void restorePreview(previous).catch(() => {});
    }
    if (previewTimerRef.current !== null) {
      window.clearTimeout(previewTimerRef.current);
    }
    previewTimerRef.current = window.setTimeout(() => {
      previewTimerRef.current = null;
      previewedEdidRef.current = targetEdid;
      void previewPreset(form, targetEdid)
        .then((res) => {
          setPreviewError(res.error ?? null);
        })
        .catch((err) => {
          setPreviewError(String(err));
        });
    }, 60);
    // Only re-trigger when preview target/toggle or colour parameters change
  }, [
    livePreview,
    targetEdid,
    form.gamma,
    form.brightness,
    form.contrast,
    form.vibrance,
    form.saturation,
    form.hue_deg,
    form.rgb_gains[0],
    form.rgb_gains[1],
    form.rgb_gains[2],
  ]);

  // Escape and backdrop dismissal must respect the same in-flight guard as the
  // disabled Save button; the save-success path passes `force` to close itself.
  const requestClose = (force = false) => {
    if (saving && !force) return;
    if (!force) {
      void endLivePreview();
    }
    setDialogOpen(false);
    onClose();
  };

  // Populate form when editing
  useEffect(() => {
    if (editPreset) {
      const values = presetToFormValues(editPreset);
      const split = splitCategory(values.name);
      setForm({ ...values, name: split.title });
      setCategory(split.category ?? "");
      if (editPreset.icc_hash) {
        setIccStatus({ hash: editPreset.icc_hash, filename: editPreset.icc_filename });
      }
    } else {
      setForm(DEFAULT_PRESET_INPUT);
      setCategory("");
      setIccStatus(null);
    }
    setErrors({});
    setCategoryError(null);
    setIccImporting(false);
    setProgramInput("");
    setProgramError(null);
    setCaptureNote(null);
    setPreviewError(null);
  }, [editPreset]);

  const canCapture =
    Boolean(targetEdid) &&
    targetCap.status === "ready" &&
    (targetCap.backendId === "nvapi" || targetCap.backendId === "preview");

  const captureDisabledReason = !targetEdid
    ? "Connect a monitor to capture live settings"
    : targetCap.status === "loading"
      ? "Detecting display backend…"
      : "Live driver capture requires an NVIDIA display";

  const handleCaptureMonitor = async () => {
    if (!targetEdid || !canCapture || capturing) return;
    setCapturing(true);
    setCaptureNote(null);
    try {
      const captured = await captureNvcp(targetEdid);
      setForm((prev) => applyCapturedState(prev, captured));
      setCaptureNote({
        text: "Captured live NVCP state (RGB gains reset to 1.00).",
        isError: false,
      });
    } catch (err) {
      setCaptureNote({
        text: `Capture failed: ${String(err)}`,
        isError: true,
      });
    } finally {
      setCapturing(false);
    }
  };

  const handleClearIcc = () => {
    setIccStatus(null);
    setForm((current) => withClearedIcc(current));
    setErrors((prev) => {
      const { icc_path: _, ...rest } = prev;
      return rest;
    });
  };

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
      setForm((current) => withSelectedIcc(current, selected));
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

  const handleAddProgram = () => {
    const result = addProgram(
      form.attached_programs,
      programInput,
      presets,
      editPreset?.id,
    );
    if (result.error) {
      setProgramError(result.error);
      return;
    }
    updateField("attached_programs", result.list);
    setProgramInput("");
    setProgramError(null);
  };

  const handleBrowseProgram = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [{ name: "Programs", extensions: ["exe"] }],
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      let list = form.attached_programs;
      let error: string | undefined;
      for (const path of paths) {
        const result = addProgram(list, path, presets, editPreset?.id);
        list = result.list;
        if (result.error) {
          error = result.error;
          break;
        }
      }
      updateField("attached_programs", list);
      setProgramError(error ?? null);
    } catch (err) {
      setProgramError(`Could not read selected programs: ${err}`);
    }
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const categoryInvalid = category.includes(" - ");
    setCategoryError(categoryInvalid ? 'Category cannot contain " - "' : null);
    const errs: ValidationErrors = {};
    if (!validatePresetForm(form, errs) || categoryInvalid) {
      setErrors(errs);
      return;
    }
    const payload: PresetInput = {
      ...form,
      name: joinCategory(category, form.name),
    };
    setSaving(true);
    try {
      if (isEditing && editPreset) {
        await updatePreset(editPreset.id, payload);
      } else {
        await createPreset(payload);
      }
      // Restore the previewed display after persisting so an edited active
      // preset re-applies its saved values and an unrelated display returns
      // cleanly to its prior state.
      await endLivePreview();
      onSaved();
      requestClose(true);
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

  const categories = Array.from(
    new Set(
      presets
        .map((p) => splitCategory(p.name).category)
        .filter((c): c is string => Boolean(c)),
    ),
  ).sort();
  const currentTag = form.color_tag ?? "";
  const tagLabel = TAG_OPTIONS.find((o) => o.value === currentTag)?.label ?? "Custom";
  const currentShape = form.tag_shape ?? "pill";
  const previewParams = {
    brightness: form.brightness,
    contrast: form.contrast,
    gamma: form.gamma,
    rgb_gains: form.rgb_gains,
    vibrance: form.vibrance,
    saturation: form.saturation,
    hue_deg: form.hue_deg,
  };

  return (
    <Dialog
      open={dialogOpen}
      onOpenChange={(next) => {
        if (!next) requestClose();
      }}
      onOpenChangeComplete={(next) => {
        if (!next) onClosed?.();
      }}
    >
      <DialogContent className="max-h-[calc(100dvh-3rem)] sm:max-w-[708px]">
        <DialogHeader>
          <span
            className="mono truncate text-[12px] uppercase tracking-[0.12em] text-primary"
            title={isEditing ? `Preset editor / ${editPreset?.name}` : "Preset editor / New"}
          >
            Preset editor / {isEditing ? editPreset?.name : "New"}
          </span>
          <DialogTitle className="font-display text-[32px] leading-none tracking-[-0.96px] text-foreground">
            Fine-tune your perspective.
          </DialogTitle>
          <DialogDescription className="font-mono text-[12px] leading-snug text-muted-foreground">
            Adjust colour without touching your display's defaults.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="flex flex-col gap-6">
          {/* ── Identity ─────────────────────────────────────────── */}
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-[1fr_200px]">
            <div className="flex min-w-0 flex-col gap-2">
              <Label
                htmlFor="preset-name"
                className="mono text-[12px] uppercase tracking-[0.12em] text-muted-foreground"
              >
                Preset name
              </Label>
              <Input
                id="preset-name"
                type="text"
                value={form.name}
                onChange={(e) => updateField("name", (e.target as HTMLInputElement).value)}
                placeholder="Warm"
                aria-invalid={errors.name ? ("true" as const) : undefined}
                className="h-11 rounded-[8px] px-3 py-2.5"
              />
              {errors.name && <p className="mono text-[10px] text-destructive-text validation-slide">{errors.name}</p>}
            </div>

            <div className="flex min-w-0 flex-col gap-2">
              <Label
                htmlFor="preset-category"
                className="mono text-[12px] uppercase tracking-[0.12em] text-muted-foreground"
              >
                Category
              </Label>
              <Combobox
                items={categories}
                value={category === "" ? null : category}
                onValueChange={(value) => {
                  setCategory(value ?? "");
                  setCategoryError(null);
                }}
                inputValue={category}
                onInputValueChange={(value) => {
                  setCategory(value);
                  if (categoryError) setCategoryError(null);
                }}
              >
                <div className="relative">
                  <ComboboxInput
                    id="preset-category"
                    placeholder="No category"
                    aria-invalid={categoryError ? ("true" as const) : undefined}
                  />
                  {category ? (
                    <button
                      type="button"
                      aria-label="Clear category"
                      title="Clear category"
                      onClick={() => {
                        setCategory("");
                        setCategoryError(null);
                      }}
                      className="absolute top-1/2 right-1 inline-flex size-8 -translate-y-1/2 items-center justify-center rounded-[8px] text-muted-foreground transition-colors hover:text-foreground focus-visible:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    >
                      <X className="size-3.5" />
                    </button>
                  ) : (
                    <ChevronDown className="pointer-events-none absolute top-1/2 right-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
                  )}
                </div>
                <ComboboxContent>
                  {(item: string) => (
                    <ComboboxItem key={item} value={item}>
                      {item}
                    </ComboboxItem>
                  )}
                </ComboboxContent>
              </Combobox>
              {categoryError && (
                <p className="mono text-[10px] text-destructive-text validation-slide">{categoryError}</p>
              )}
            </div>
          </div>

          {/* ── Color tag ────────────────────────────────────────── */}
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between gap-3">
              <span className="font-body text-[12px] uppercase tracking-[0.4px] text-muted-foreground">
                Tag colour
              </span>
              <span className="mono text-[10px] text-muted-foreground">{tagLabel}</span>
            </div>
            <div className="flex flex-wrap items-center gap-2" role="group" aria-label="Tag colour">
              {TAG_OPTIONS.map((option) => {
                const selected = currentTag === option.value;
                return (
                  <button
                    key={option.label}
                    type="button"
                    title={option.label}
                    aria-label={`Tag colour ${option.label}`}
                    aria-pressed={selected}
                    onClick={() => updateField("color_tag", option.value)}
                    className={cn(
                      "size-7 rounded-full border transition-[border-color]",
                      selected
                        ? "scale-110 border-primary ring-2 ring-primary ring-offset-2 ring-offset-surface transition-[scale,border-color] duration-[var(--motion-fast)]"
                        : "border-border hover:border-border-hover",
                      !option.value && "border-dashed bg-surface-hover",
                    )}
                    style={option.value ? { backgroundColor: option.value } : undefined}
                  />
                );
              })}
            </div>

            <div className="flex flex-wrap items-center gap-3">
              <span className="font-body text-[12px] uppercase tracking-[0.4px] text-muted-foreground">
                Tag shape
              </span>
              <div className="flex flex-wrap items-center gap-2" role="group" aria-label="Tag shape">
                {TAG_SHAPE_OPTIONS.map((option) => {
                  const selected = currentShape === option.value;
                  return (
                    <button
                      key={option.value}
                      type="button"
                      title={option.label}
                      aria-label={`Tag shape ${option.label}`}
                      aria-pressed={selected}
                      onClick={() => updateField("tag_shape", option.value)}
                      className={cn(
                        "inline-flex size-9 items-center justify-center rounded-[8px] border transition-[scale,border-color] duration-[var(--motion-fast)]",
                        selected
                          ? "scale-110 border-primary bg-primary-subtle"
                          : "border-border hover:border-border-hover",
                      )}
                    >
                      {/* Colour change remounts the selected preview so its
                          existing pop-in replays; unselected tags stay still. */}
                      <PresetTag
                        key={`${currentTag}-${option.value}`}
                        color={currentTag || undefined}
                        shape={option.value}
                        className={selected ? "pop-in" : undefined}
                      />
                    </button>
                  );
                })}
              </div>
            </div>
          </div>

          {/* ── Programs (auto-apply) ────────────────────────────── */}
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between gap-3">
              <span className="font-body text-[12px] uppercase tracking-[0.4px] text-muted-foreground">
                Programs
              </span>
              <span className="mono text-[10px] text-muted-foreground">
                {form.attached_programs.length} mapped
              </span>
            </div>
            <p className="font-body text-[12px] text-muted-foreground">
              Applies this preset to the monitor showing the program when it comes forward.
              Auto-apply must be switched on in the footer.
            </p>
            {form.attached_programs.length > 0 && (
              <div className="flex flex-wrap gap-2">
                {form.attached_programs.map((program) => (
                  <span
                    key={program}
                    data-slot="program-chip"
                    className="mono inline-flex items-center gap-1 rounded-[6px] border border-border bg-surface-hover py-1 pr-1 pl-2 text-[10px] text-foreground"
                  >
                    {program}
                    <button
                      type="button"
                      aria-label={`Remove ${program}`}
                      title={`Remove ${program}`}
                      onClick={() =>
                        updateField(
                          "attached_programs",
                          form.attached_programs.filter((p) => p !== program),
                        )
                      }
                      className="inline-flex size-4 items-center justify-center rounded-[4px] text-muted-foreground transition-colors hover:text-foreground focus-visible:text-foreground focus-visible:outline-none"
                    >
                      <X className="size-3" />
                    </button>
                  </span>
                ))}
              </div>
            )}
            <div className="flex items-center gap-2">
              <Input
                value={programInput}
                onChange={(e) => {
                  setProgramInput((e.target as HTMLInputElement).value);
                  if (programError) setProgramError(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    handleAddProgram();
                  }
                }}
                placeholder="chrome.exe"
                aria-label="Program executable"
                className="flex-1"
              />
              <button
                type="button"
                onClick={handleAddProgram}
                className="shrink-0 rounded-[8px] border border-border px-2.5 py-1.5 font-mono text-[11px] uppercase tracking-[0.1em] text-muted-foreground transition-colors hover:border-border-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
              >
                Add
              </button>
              <button
                type="button"
                onClick={() => void handleBrowseProgram()}
                className="shrink-0 rounded-[8px] border border-border px-2.5 py-1.5 font-mono text-[11px] uppercase tracking-[0.1em] text-muted-foreground transition-colors hover:border-border-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
              >
                Browse…
              </button>
            </div>
            {programError && (
              <p className="mono text-[10px] text-destructive-text validation-slide">{programError}</p>
            )}
          </div>

          {/* ── Live colour bar preview ──────────────────────────── */}
          <div className="flex flex-col gap-3">
            <span className="mono text-[12px] uppercase tracking-[0.12em] text-muted-foreground">
              Live colour bar preview
            </span>
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <LiveColorCard label="Original" params={IDENTITY_PREVIEW} />
              <LiveColorCard label="Adjusted" params={previewParams} highlight />
            </div>

            {/* Hardware calibration bar: live monitor preview & NVCP capture */}
            {connectedMonitors.length > 0 && (
              <div
                data-slot="editor-hardware-bar"
                className="flex flex-col gap-2 rounded-[8px] border border-border bg-surface p-3"
              >
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div className="flex min-w-0 flex-1 items-center gap-2">
                    <Select
                      value={targetEdid}
                      onValueChange={(value) => {
                        if (value) {
                          setTargetEdid(value);
                          setCaptureNote(null);
                        }
                      }}
                    >
                      <SelectTrigger
                        size="sm"
                        aria-label="Target monitor for preview or capture"
                        className="max-w-[240px] min-w-[160px] font-mono text-[11px]"
                      >
                        <SelectValue>
                          {(value: string | null) => {
                            const mon = connectedMonitors.find((m) => m.edid_id === value);
                            return mon ? (mon.alias || mon.model || mon.device_name) : "Select monitor";
                          }}
                        </SelectValue>
                      </SelectTrigger>
                      <SelectContent>
                        {connectedMonitors.map((m) => (
                          <SelectItem key={m.edid_id} value={m.edid_id}>
                            {m.alias || m.model || m.device_name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    {targetCap.status === "ready" && targetCap.backendLabel && (
                      <span className="mono hidden text-[10px] text-muted-foreground sm:inline">
                        {targetCap.backendLabel}
                      </span>
                    )}
                  </div>

                  <div className="flex shrink-0 items-center gap-2.5">
                    <label
                      className={cn(
                        "inline-flex cursor-pointer items-center gap-1.5 rounded-[8px] border px-2.5 py-1 select-none transition-colors",
                        livePreview
                          ? "border-primary bg-primary-subtle text-primary"
                          : "border-border text-muted-foreground hover:border-border-hover hover:text-foreground",
                      )}
                      title="Preview slider changes live on the selected monitor (reverts on Cancel)"
                    >
                      <Checkbox
                        checked={livePreview}
                        onCheckedChange={(checked) => setLivePreview(Boolean(checked))}
                        aria-label="Live preview on selected monitor"
                      >
                        <CheckboxIndicator />
                      </Checkbox>
                      <Eye className="size-3.5" strokeWidth={1.5} />
                      <span className="font-mono text-[10px] uppercase tracking-[0.1em]">
                        {livePreview ? "Live" : "Preview"}
                      </span>
                    </label>

                    <button
                      type="button"
                      onClick={() => void handleCaptureMonitor()}
                      disabled={!canCapture || capturing}
                      title={canCapture ? "Import live brightness, contrast, gamma, vibrance and hue from this NVIDIA monitor" : captureDisabledReason}
                      className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] border border-border px-2.5 py-1.5 font-mono text-[10px] uppercase tracking-[0.1em] text-muted-foreground transition-colors enabled:hover:border-border-hover enabled:hover:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-45"
                    >
                      <Download className="size-3" strokeWidth={1.5} />
                      {capturing ? "Capturing…" : "Capture"}
                    </button>
                  </div>
                </div>
                {previewError && (
                  <p className="mono text-[10px] text-destructive-text" role="alert">
                    Preview error: {previewError}
                  </p>
                )}
                {captureNote && (
                  <p
                    className={cn(
                      "mono text-[10px]",
                      captureNote.isError ? "text-destructive-text" : "text-muted-foreground",
                    )}
                    aria-live="polite"
                  >
                    {captureNote.text}
                  </p>
                )}
              </div>
            )}
          </div>

          <div className="h-px w-full bg-border" aria-hidden="true" />

          {/* ── Parameters: header + 2 columns ──────────────────── */}
          <div className="flex flex-col gap-5">
            <div className="flex items-center justify-between gap-3">
              <div className="flex flex-col gap-0.5">
                <span className="mono text-[12px] uppercase tracking-[0.12em] text-muted-foreground">
                  Colour parameters
                </span>
                <span className="mono text-[10px] text-muted-foreground">
                  Click any number to type · Double-click a slider to reset to neutral
                </span>
              </div>
              <button
                type="button"
                disabled={
                  form.gamma === NEUTRAL.gamma &&
                  form.brightness === NEUTRAL.brightness &&
                  form.contrast === NEUTRAL.contrast &&
                  form.vibrance === NEUTRAL.vibrance &&
                  form.saturation === NEUTRAL.saturation &&
                  form.hue_deg === 0 &&
                  form.rgb_gains[0] === 1 &&
                  form.rgb_gains[1] === 1 &&
                  form.rgb_gains[2] === 1
                }
                onClick={() => setForm((prev) => resetSliderValues(prev))}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-[8px] border border-border px-2.5 py-1.5 font-mono text-[10px] uppercase tracking-[0.1em] text-muted-foreground transition-colors enabled:hover:border-border-hover enabled:hover:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring disabled:opacity-40"
              >
                <RotateCcw className="size-3" strokeWidth={1.5} />
                Reset sliders
              </button>
            </div>

            <div className="grid grid-cols-1 gap-x-6 gap-y-5 sm:grid-cols-2">
              <SliderField
                label="Gamma"
                min={0.3} max={2.8} step={0.05} neutral={NEUTRAL.gamma}
                value={form.gamma}
                display={form.gamma.toFixed(2)}
                onChange={(v) => updateField("gamma", v)}
                error={errors.gamma}
              />
              <SliderField
                label="Brightness"
                min={0} max={100} step={1} neutral={NEUTRAL.brightness}
                value={form.brightness}
                display={form.brightness.toFixed(0)}
                onChange={(v) => updateField("brightness", v)}
                error={errors.brightness}
              />
              <SliderField
                label="Contrast"
                min={0} max={100} step={1} neutral={NEUTRAL.contrast}
                value={form.contrast}
                display={form.contrast.toFixed(0)}
                onChange={(v) => updateField("contrast", v)}
                error={errors.contrast}
              />
              <SliderField
                label="Vibrance"
                min={0} max={100} step={1} neutral={NEUTRAL.vibrance}
                value={form.vibrance}
                display={form.vibrance.toFixed(0)}
                onChange={(v) => updateField("vibrance", v)}
                error={errors.vibrance}
              />
              <SliderField
                label="Saturation"
                min={0} max={100} step={1} neutral={NEUTRAL.saturation}
                value={form.saturation}
                display={form.saturation.toFixed(0)}
                onChange={(v) => updateField("saturation", v)}
                error={errors.saturation}
              />
              <SliderField
                label="Hue"
                min={0} max={359} step={1} neutral={0}
                value={form.hue_deg}
                display={`${form.hue_deg.toFixed(0)}°`}
                onChange={(v) => updateField("hue_deg", v)}
                error={errors.hue_deg}
              />
            </div>

            {/* ── RGB Gains (3-channel multipliers) ──────────────── */}
            <div className="flex flex-col gap-3 rounded-[8px] border border-border bg-surface p-4">
              <div className="flex items-center justify-between gap-2">
                <span className="font-body text-[12px] uppercase tracking-[0.4px] text-muted-foreground">
                  RGB Gains
                </span>
                <span className="mono text-[10px] text-muted-foreground">
                  Neutral 1.00 · per-channel multiplier
                </span>
              </div>
              <div className="grid grid-cols-1 gap-x-5 gap-y-4 sm:grid-cols-3">
                {(["Red", "Green", "Blue"] as const).map((channel, idx) => (
                  <SliderField
                    key={channel}
                    label={channel}
                    min={0.5}
                    max={1.5}
                    inputMin={0}
                    inputMax={10}
                    step={0.01}
                    neutral={1.0}
                    value={form.rgb_gains[idx]}
                    display={form.rgb_gains[idx].toFixed(2)}
                    onChange={(v) => {
                      const next: [number, number, number] = [
                        form.rgb_gains[0],
                        form.rgb_gains[1],
                        form.rgb_gains[2],
                      ];
                      next[idx] = v;
                      updateField("rgb_gains", next);
                    }}
                  />
                ))}
              </div>
              {errors.rgb_gains && (
                <p className="mono text-[10px] text-destructive-text validation-slide">{errors.rgb_gains}</p>
              )}
            </div>
          </div>

          {/* ── ICC profile upload ───────────────────────────────── */}
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-4 rounded-[8px] border border-dashed border-border p-4">
              <span className="flex size-9 shrink-0 items-center justify-center rounded-[8px] border border-border text-muted-foreground">
                <Upload className="size-4" strokeWidth={1.5} />
              </span>
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="mono text-[12px] text-foreground">ICC profile</span>
                <span className="mono truncate text-[12px] text-muted-foreground" title={iccStatus?.filename}>
                  Optional · .icc or .icm · {iccStatus ? iccStatus.filename : "No file selected"}
                </span>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {iccStatus && (
                  <button
                    type="button"
                    onClick={handleClearIcc}
                    disabled={iccImporting}
                    aria-label="Remove ICC profile"
                    title="Detach ICC profile from this preset"
                    className="inline-flex shrink-0 items-center gap-1 rounded-[8px] border border-border px-2.5 py-1.5 font-mono text-[11px] uppercase tracking-[0.1em] text-muted-foreground transition-colors hover:border-border-hover hover:text-destructive-text focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring disabled:opacity-50"
                  >
                    <X className="size-3" strokeWidth={1.5} />
                    Clear
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => void handleBrowseIcc()}
                  disabled={iccImporting}
                  className="shrink-0 rounded-[8px] border border-border px-2.5 py-1.5 font-mono text-[11px] uppercase tracking-[0.1em] text-muted-foreground transition-colors hover:border-border-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring disabled:opacity-50"
                >
                  {iccImporting ? "Importing…" : "Choose file"}
                </button>
              </div>
            </div>
            {errors.icc_path && <p className="mono text-[10px] text-destructive-text validation-slide">{errors.icc_path}</p>}
          </div>

          {/* ── Actions ──────────────────────────────────────────── */}
          <DialogFooter>
            <Button variant="ghost" type="button" onClick={() => requestClose()}>
              Cancel
            </Button>
            <Button variant="default" type="submit" disabled={saving || iccImporting} trailingIcon={<Check className="size-4 text-primary-foreground" />}>
              {saving ? "Saving…" : "Save"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Flat slider row: label + click-to-type mono value + double-click neutral reset. */
function SliderField({
  label,
  min,
  max,
  inputMin,
  inputMax,
  step,
  neutral,
  value,
  display,
  onChange,
  error,
}: {
  label: string;
  min: number;
  max: number;
  inputMin?: number;
  inputMax?: number;
  step: number;
  neutral: number;
  value: number;
  display: string;
  onChange: (v: number) => void;
  error?: string;
}) {
  const [editingNumber, setEditingNumber] = useState(false);
  const [draft, setDraft] = useState("");
  const isNonNeutral = Math.abs(value - neutral) > 1e-6;
  const effectiveMin = inputMin ?? min;
  const effectiveMax = inputMax ?? max;
  const sliderValue = Math.min(max, Math.max(min, value));

  const commitDraft = () => {
    setEditingNumber(false);
    const parsed = Number(draft);
    if (!Number.isFinite(parsed)) return;
    onChange(clampSliderValue(parsed, effectiveMin, effectiveMax, step));
  };

  return (
    <div className="flex min-w-0 flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <Label className="font-body text-[12px] uppercase tracking-[0.4px] text-muted-foreground">
          {label}
        </Label>
        <div className="flex items-center gap-1.5">
          {isNonNeutral && !editingNumber && (
            <button
              type="button"
              onClick={() => onChange(neutral)}
              aria-label={`Reset ${label} to neutral (${neutral})`}
              title={`Reset ${label} to neutral`}
              className="inline-flex size-4 items-center justify-center rounded-[4px] text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <RotateCcw className="size-2.5" strokeWidth={1.75} />
            </button>
          )}
          {editingNumber ? (
            <input
              type="number"
              aria-label={`${label} value`}
              autoFocus
              step={step}
              min={effectiveMin}
              max={effectiveMax}
              value={draft}
              onFocus={(e) => e.currentTarget.select()}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={commitDraft}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  commitDraft();
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  e.stopPropagation();
                  setEditingNumber(false);
                }
              }}
              className="mono h-5 w-16 rounded-[4px] border border-primary bg-surface-hover px-1.5 text-right text-[12px] tabular-nums text-foreground focus:outline-none"
            />
          ) : (
            <button
              type="button"
              onClick={() => {
                setDraft(String(value));
                setEditingNumber(true);
              }}
              title="Click to type an exact value"
              aria-label={`Edit ${label} value (${display})`}
              className={cn(
                "mono rounded-[4px] px-1 text-[12px] tabular-nums transition-colors hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                isNonNeutral ? "text-primary" : "text-foreground",
              )}
            >
              {display}
            </button>
          )}
        </div>
      </div>
      <div
        onDoubleClick={() => onChange(neutral)}
        title={`Double-click to reset ${label} to neutral`}
      >
        <Slider
          min={min}
          max={max}
          step={step}
          value={[sliderValue]}
          onValueChange={([v]) => onChange(v)}
          className="w-full"
        />
      </div>
      {error && <p className="mono text-[10px] text-destructive-text validation-slide">{error}</p>}
    </div>
  );
}
