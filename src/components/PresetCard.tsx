import { useState, useEffect } from "react";
import type { Monitor, Preset } from "../lib/types";
import { deletePreset, createPreset, unpinMonitor } from "../lib/tauri";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Check, Copy, Pencil, PinOff, Trash2 } from "lucide-react";

interface Props {
  preset: Preset;
  monitors: Monitor[];
  pins: Record<string, string>;
  onEdit: (preset: Preset) => void;
  onRefreshParent: () => void;
  onPinChange: () => void;
  onApply: (preset: Preset) => void;
  appliedMap: Record<string, string>;
}

export default function PresetCard({ preset, monitors, pins, onEdit, onRefreshParent, onPinChange, onApply, appliedMap }: Props) {
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    if (!showDeleteModal) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShowDeleteModal(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [showDeleteModal]);

  const handleDuplicate = async () => {
    try {
      await createPreset({
        name: `${preset.name} (copy)`,
        brightness: preset.brightness,
        contrast: preset.contrast,
        rgb_gains: preset.rgb_gains,
        gamma: preset.gamma,
        vibrance: preset.vibrance,
        hue_deg: preset.hue_deg,
      });
      onRefreshParent();
    } catch {
      // silent
    }
  };

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await deletePreset(preset.id);
      setShowDeleteModal(false);
      onRefreshParent();
    } catch {
      setShowDeleteModal(false);
    } finally {
      setDeleting(false);
    }
  };

  // Find ALL monitors that have this preset pinned
  const pinnedEdidList = Object.entries(pins)
    .filter(([, pid]) => pid === preset.id)
    .map(([edid]) => edid);
  const isPinned = pinnedEdidList.length > 0;
  const isConnected = (edid: string) =>
    monitors.find((m) => m.edid_id === edid)?.connected ?? false;
  // In use = pinned to a connected monitor, or applied this session
  const isActive =
    pinnedEdidList.some(isConnected) ||
    Object.entries(appliedMap).some(
      ([edid, pid]) => pid === preset.id && isConnected(edid),
    );
  const pinnedMonitorNames = pinnedEdidList.map((edid) => {
    const m = monitors.find((m) => m.edid_id === edid);
    return m?.alias || m?.model || edid.slice(0, 12);
  });

  return (
    <div className={`flex flex-col rounded-xl bg-card text-card-foreground ring-1 ring-foreground/10 ${isActive || isPinned ? "ring-primary/30" : ""}`}>
      {/* Card body — stacked info */}
      <div className="px-4 py-3 space-y-2">
        {/* Name + badges row */}
        <div className="flex items-start justify-between gap-2">
          <span className="text-sm font-medium text-foreground truncate leading-tight">
            {preset.name}
          </span>
          <div className="flex items-center gap-1 shrink-0">
            {isPinned && (
              <Badge variant="outline" className="text-accent border-accent" title={`Pinned to ${pinnedMonitorNames.join(", ")}`}>
                PINNED
              </Badge>
            )}
            {preset.icc_hash && (
              <Badge variant="outline" title={preset.icc_filename}>
                ICC
              </Badge>
            )}
          </div>
        </div>

        {/* Parameter row — mono numerals */}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground mono">
          <span>γ{preset.gamma.toFixed(1)}</span>
          <span>B{preset.brightness.toFixed(0)}</span>
          <span>C{preset.contrast.toFixed(0)}</span>
          <span>RGB {preset.rgb_gains.map((v) => v.toFixed(1)).join("/")}</span>
          <span>V{preset.vibrance.toFixed(0)}</span>
          <span>H{preset.hue_deg.toFixed(0)}°</span>
        </div>

        {/* Pin target hint */}
        {pinnedMonitorNames.length > 0 && (
          <span className="text-xs text-muted-foreground">
            → {pinnedMonitorNames.join(", ")}
          </span>
        )}
      </div>

      {/* Action buttons row */}
      <div className="flex flex-wrap items-center gap-1 px-4 pb-3">
        {/* APPLY = primary */}
        <Button variant="default" size="icon-sm" onClick={() => onApply(preset)} title="Apply" aria-label={`Apply preset ${preset.name}`}>
          <Check />
        </Button>

        {isPinned && (
          <Button
            variant="outline"
            size="icon-sm"
            onClick={async () => {
              try {
                for (const edid of pinnedEdidList) {
                  await unpinMonitor(edid);
                }
                onPinChange();
              } catch {
                // silent
              }
            }}
            title="Unpin from all monitors"
            aria-label={`Unpin preset ${preset.name} from all monitors`}
          >
            <PinOff />
          </Button>
        )}

        {/* EDIT = secondary */}
        <Button variant="secondary" size="icon-sm" onClick={() => onEdit(preset)} title="Edit" aria-label={`Edit preset ${preset.name}`}>
          <Pencil />
        </Button>

        {/* DUP = outline */}
        <Button variant="outline" size="icon-sm" onClick={handleDuplicate} title="Duplicate preset" aria-label={`Duplicate preset ${preset.name}`}>
          <Copy />
        </Button>

        {/* DEL = destructive */}
        <Button variant="destructive" size="icon-sm" onClick={() => setShowDeleteModal(true)} title="Delete preset" aria-label={`Delete preset ${preset.name}`}>
          <Trash2 />
        </Button>
      </div>

      {/* Delete confirmation popup */}
      {showDeleteModal && (
        <div
          className="fixed inset-0 isolate z-50 flex items-center justify-center bg-black/10 p-4"
          onClick={() => { if (!deleting) setShowDeleteModal(false); }}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label={`Delete preset ${preset.name}`}
            className="w-full max-w-xs rounded-xl bg-popover text-popover-foreground ring-1 ring-foreground/10 p-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-base font-medium text-foreground">
              Delete preset?
            </h3>
            <p className="mt-2 text-sm text-muted-foreground leading-relaxed">
              Permanently delete <span className="font-medium text-foreground">"{preset.name}"</span>?
              This cannot be undone.
            </p>
            <div className="mt-4 flex items-center justify-end gap-2">
              <Button variant="outline" size="sm" onClick={() => setShowDeleteModal(false)} disabled={deleting}>
                CANCEL
              </Button>
              <Button variant="destructive" size="sm" onClick={() => void handleDelete()} disabled={deleting}>
                {deleting ? "DELETING…" : "DELETE"}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}