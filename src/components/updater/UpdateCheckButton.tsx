"use client";

import { useCallback } from "react";
import { RefreshCw } from "lucide-react";

interface Props {
  checking: boolean;
  onCheck: () => void;
}

/** Small footer button that triggers a manual update check. */
export default function UpdateCheckButton({ checking, onCheck }: Props) {
  const handleClick = useCallback(() => {
    onCheck();
  }, [onCheck]);

  return (
    <button
      data-slot="button"
      disabled={checking}
      onClick={handleClick}
      className="inline-flex items-center justify-center rounded-[8px] border border-border bg-transparent px-3 py-1.5 font-mono text-[10px] uppercase tracking-[0.12em] text-foreground transition-colors hover:border-border-hover hover:bg-surface-hover disabled:opacity-50 disabled:pointer-events-none"
      title={checking ? "Checking for updates…" : "Check for updates"}
    >
      {checking ? (
        <span className="flex items-center gap-1.5">
          <span className="animate-spin size-4 shrink-0 border-2 border-border border-t-foreground rounded-full" />
          <span className="btn-label">Checking…</span>
        </span>
      ) : (
        <span className="flex items-center gap-1.5">
          <RefreshCw size={10} strokeWidth={1.5} />
          <span className="btn-label">Check for updates</span>
        </span>
      )}
    </button>
  );
}
