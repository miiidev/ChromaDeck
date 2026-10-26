import { useState } from "react";
import type { Preset } from "../lib/types";
import { splitCategory } from "../lib/presetCard";
import { Trash2 } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { ArrowRightGlyph } from "@/components/ui/icons";

interface Props {
  preset: Preset;
  onClose: () => void;
  /** Fired after the close animation finishes; the parent unmounts here. */
  onClosed?: () => void;
  onConfirm: () => Promise<boolean> | boolean;
  /** Display-only context for the affected preset row. */
  pinnedTo?: string;
  inUse?: boolean;
  deleting?: boolean;
  /** Failure message from the last delete attempt; dialog stays open. */
  error?: string;
}

export default function DeletePresetDialog({
  preset,
  onClose,
  onClosed,
  onConfirm,
  pinnedTo,
  inUse,
  deleting,
  error,
}: Props) {
  // Own the open state so the exit animation can play before the parent
  // unmounts this dialog (see onOpenChangeComplete below).
  const [open, setOpen] = useState(true);
  const requestClose = (force = false) => {
    if (deleting && !force) return;
    setOpen(false);
    onClose();
  };
  const { category } = splitCategory(preset.name);
  const status = `${pinnedTo ? `Pinned to ${pinnedTo}` : "Not pinned"}${inUse ? " · Currently in use" : ""}`;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) requestClose();
      }}
      onOpenChangeComplete={(next) => {
        if (!next) onClosed?.();
      }}
    >
      <DialogContent className="sm:max-w-[440px]">
        <div className="flex size-12 items-center justify-center rounded-[8px] bg-destructive">
          <Trash2 className="size-5 text-white" strokeWidth={1.5} />
        </div>

        <div className="flex flex-col gap-2">
          <DialogTitle className="truncate font-display text-[32px] leading-none tracking-[-0.96px] text-foreground">
            Delete {preset.name}?
          </DialogTitle>
          <DialogDescription className="font-mono text-[12px] leading-snug text-muted-foreground">
            This preset will be permanently removed.
          </DialogDescription>
        </div>

        <div className="rounded-[12px] border border-border bg-surface p-4">
          <span className="inline-flex items-center rounded-full border border-border px-2 py-[3px] font-mono text-[9px] uppercase tracking-[0.1em] text-muted-foreground">
            {category ?? "Preset"}
          </span>
          <p className="mt-2 truncate font-mono text-[12px] text-foreground">{preset.name}</p>
          <p className="mt-1 truncate font-mono text-[10px] text-muted-foreground">{status}</p>
        </div>

        <p className="font-mono text-[10px] leading-relaxed text-muted-foreground">
          Your display keeps its current settings until you apply another preset.
        </p>

        {error && (
          <p role="alert" className="truncate font-mono text-[10px] text-destructive">
            {error}
          </p>
        )}

        <DialogFooter>
          <Button variant="ghost" type="button" onClick={() => requestClose()} disabled={deleting}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            type="button"
            onClick={() => {
              void (async () => {
                const deleted = await onConfirm();
                // Close through the exit animation only when the delete landed;
                // failures keep the dialog open with the error.
                if (deleted) requestClose(true);
              })();
            }}
            disabled={deleting}
            trailingIcon={<ArrowRightGlyph />}
          >
            {deleting ? "Deleting…" : "Delete"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
