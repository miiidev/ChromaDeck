"use client";

import { useCallback } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { UpdateInfo } from "@/lib/tauri";

type DialogKind = "available" | "downloading" | "installing" | "error" | "up-to-date";

interface Props {
  open: boolean;
  onClose: () => void;
  onDownloadAndInstall: () => void;
  onRetry: () => void;
  error: string | null;
  onRemindMeLater: () => void;
  update: UpdateInfo | null;
  kind: DialogKind;
  progress: number | null;
}

/** Dialog shown when an update check completes — whether or not an update exists. */
export default function UpdateDialog({
  open,
  onClose,
  onDownloadAndInstall,
  onRetry,
  error,
  onRemindMeLater,
  update,
  kind,
  progress,
}: Props) {
  const handleDownloadAndInstall = useCallback(() => {
    onDownloadAndInstall();
  }, [onDownloadAndInstall]);

  const busy = kind === "downloading" || kind === "installing";

  const version = update?.version ?? "";
  const notes = update?.notes ?? null;
  const date = update?.date ?? null;

  return (
    <Dialog open={open} onOpenChange={(isOpen) => {
      if (!isOpen && !busy) onClose();
    }}>
      <DialogContent showCloseButton={!busy}>
        <DialogHeader>
          {kind === "up-to-date" ? (
            <DialogTitle>You&rsquo;re up to date</DialogTitle>
          ) : (
            <DialogTitle>{kind === "error" ? "Update failed" : "Update available"}</DialogTitle>
          )}
          {date && (
            <DialogDescription>{date}</DialogDescription>
          )}
        </DialogHeader>
        <div className="flex flex-col gap-3">
          {kind === "error" && <p role="alert" className="text-sm text-foreground">{error}</p>}
          {kind === "installing" && <p role="status" className="text-sm text-muted-foreground">Installing update… The app will close when the installer starts.</p>}
          <div className="mono text-[11px] text-muted-foreground">
            Version {version}
          </div>
          {notes && kind !== "up-to-date" && (
            <div className="mono text-[10px] text-foreground leading-relaxed">
              {notes}
            </div>
          )}
          {kind === "downloading" && (
            <div className="flex flex-col gap-1.5">
              <div className="flex justify-between mono text-[10px] text-muted-foreground">
                <span>Downloading</span>
                <span>{progress != null ? `${progress}%` : "…"}</span>
              </div>
              <div className="w-full bg-subtle-fg rounded-full h-1.5 overflow-hidden">
                <div
                  className="bg-primary h-full"
                  style={{ width: progress != null ? `${progress}%` : "0%" }}
                />
              </div>
            </div>
          )}
        </div>
        <DialogFooter>
          {busy ? (
            <Button
              variant="outline"
              onClick={onClose}
              disabled
              className="cursor-not-allowed opacity-50"
            >
              Installing…
            </Button>
          ) : kind === "up-to-date" ? (
            <Button onClick={onRemindMeLater}>
              OK
            </Button>
          ) : (
            <>
              <Button variant="outline" onClick={onRemindMeLater}>
                Remind me later
              </Button>
              <Button
                onClick={kind === "error" ? onRetry : handleDownloadAndInstall}
              >
                {kind === "error" ? "Check again" : "Download & install"}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
