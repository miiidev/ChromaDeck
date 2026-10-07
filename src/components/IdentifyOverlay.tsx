import { useEffect, useState } from "react";
import { getIdentifyInfo } from "../lib/tauri";

interface Props {
  number: number;
  total?: number;
  monitorName?: string;
}

/**
 * Fullscreen‑centered overlay shown on an identify‑* webview window.
 * Displays the monitor number in a giant font, the monitor's display
 * name prominently underneath, and a "CHROMADECK · monitor N of M"
 * caption. Resolves name/total via the backend on mount (the window
 * label only carries the number); props act as initial values.
 *
 * Uses shadcn‑style token classes only (bg-background, text-foreground, etc.).
 */
export default function IdentifyOverlay({ number, total: totalProp, monitorName: nameProp }: Props) {
  const [name, setName] = useState<string | null>(nameProp ?? null);
  const [total, setTotal] = useState<number | null>(totalProp ?? null);

  useEffect(() => {
    if (nameProp !== undefined && totalProp !== undefined) return;
    let live = true;
    getIdentifyInfo(number)
      .then((info) => {
        if (!live) return;
        if (nameProp === undefined) setName(info.name);
        if (totalProp === undefined) setTotal(info.total);
      })
      .catch(() => {
        // e.g. browser dev preview without Tauri backend: number only
      });
    return () => {
      live = false;
    };
  }, [number, nameProp, totalProp]);

  const prefersReducedMotion =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)")?.matches;

  return (
    <div
      className="fixed inset-0 flex items-center justify-center bg-background/60"
      style={
        prefersReducedMotion
          ? { transition: "none" }
          : { transition: "opacity var(--motion-std) var(--ease-out)" }
      }
    >
      <div className="flex flex-col items-center gap-3 px-6">
        {/* Giant monitor number */}
        <span
          className="text-[128px] font-bold leading-none text-foreground select-none"
          aria-label={`Monitor ${number}`}
        >
          {number}
        </span>

        {/* Monitor display name — the point of the overlay */}
        {name && (
          <p className="max-w-[380px] break-words text-center text-xl font-semibold text-foreground">
            {name}
          </p>
        )}

        {/* Caption */}
        <p className="text-base font-mono text-muted-foreground tracking-widest uppercase">
          CHROMADECK · Monitor {number}
          {total !== null && total !== undefined && total > 1 && (
            <span className="text-muted-foreground/70">
              {" "}of {total}
            </span>
          )}
        </p>
      </div>
    </div>
  );
}
