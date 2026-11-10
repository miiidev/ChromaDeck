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

type DialogKind = "available" | "downloading" | "ready" | "up-to-date";

interface Props {
  open: boolean;
  onClose: () => void;
  onDownloadAndInstall: () => void;
  onRestart: () => void;
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
  onRestart,
  onRemindMeLater,
  update,
  kind,
  progress,
}: Props) {
  const handleDownloadAndInstall = useCallback(() => {
    onDownloadAndInstall();
  }, [onDownloadAndInstall]);

  const handleRestart = useCallback(() => {
    onRestart();
  }, [onRestart]);

  const version = update?.version ?? "";
  const notes = update?.notes ?? null;
  const date = update?.date ?? null;

  return (
    <Dialog open={open} onOpenChange={(isOpen) => {
      if (!isOpen) onClose();
    }}>
      <DialogContent>
        <DialogHeader>
          {kind === "up-to-date" ? (
            <DialogTitle>You&rsquo;re up to date</DialogTitle>
          ) : (
            <DialogTitle>Update available</DialogTitle>
          )}
          {date && (
            <DialogDescription>{date}</DialogDescription>
          )}
        </DialogHeader>
        <div className="flex flex-col gap-3">
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
          {kind === "downloading" ? (
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
                onClick={
                  kind === "ready" ? handleRestart : handleDownloadAndInstall
                }
              >
                {kind === "ready" ? "Restart now" : "Download & install"}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
