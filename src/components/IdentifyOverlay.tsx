import { useEffect, useState } from "react";
import { getIdentifyInfo } from "../lib/tauri";

interface Props {
  number: number;
  total?: number;
  monitorName?: string;
  /** Raw identity chain line from the backend; rendered under the name. */
  chain?: string;
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
export default function IdentifyOverlay({
  number,
  total: totalProp,
  monitorName: nameProp,
  chain: chainProp,
}: Props) {
  const [name, setName] = useState<string | null>(nameProp ?? null);
  const [total, setTotal] = useState<number | null>(totalProp ?? null);
  const [chain, setChain] = useState<string | null>(chainProp ?? null);

  useEffect(() => {
    if (nameProp !== undefined && totalProp !== undefined && chainProp !== undefined) return;
    let live = true;
    getIdentifyInfo(number)
      .then((info) => {
        if (!live) return;
        if (nameProp === undefined) setName(info.name);
        if (totalProp === undefined) setTotal(info.total);
        if (chainProp === undefined) setChain(info.chain ?? null);
      })
      .catch(() => {
        // e.g. browser dev preview without Tauri backend: number only
      });
    return () => {
      live = false;
    };
  }, [number, nameProp, totalProp, chainProp]);

  const prefersReducedMotion =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)")?.matches;

  return (
    <div
      className="pointer-events-none fixed inset-0 flex select-none items-center justify-center"
      style={
        prefersReducedMotion
          ? { transition: "none" }
          : { transition: "opacity var(--motion-std) var(--ease-out)" }
      }
    >
      {/* Card is sized in vmin so it reads the same on 1080p → 4K/ultrawide. */}
      <div className="flex flex-col items-center gap-[2vmin] rounded-[8px] bg-background/70 px-[6vmin] py-[4vmin] backdrop-blur-sm">
        {/* Giant monitor number */}
        <span
          className="font-display text-[clamp(96px,22vmin,360px)] font-extrabold leading-none text-primary select-none"
          aria-label={`Monitor ${number}`}
        >
          {number}
        </span>

        {/* Monitor display name — the point of the overlay */}
        {name && (
          <p className="max-w-[min(80vw,60ch)] break-words text-center text-[clamp(20px,3.2vmin,48px)] font-semibold text-foreground">
            {name}
          </p>
        )}

        {/* Caption */}
        <p className="text-[clamp(12px,1.6vmin,22px)] font-mono text-foreground/80 tracking-widest uppercase">
          CHROMADECK · Monitor {number}
          {total !== null && total !== undefined && total > 1 && (
            <span className="text-foreground/60">
              {" "}of {total}
            </span>
          )}
        </p>

        {/* Raw identity chain — diagnostics evidence, machine-parsable. */}
        {chain && (
          <p className="max-w-[min(80vw,80ch)] break-all text-center font-mono text-[clamp(10px,1.3vmin,16px)] text-foreground/60">
            <span className="uppercase tracking-widest text-foreground/70">
              Monitor identity chain
            </span>
            <br />
            {chain.includes("EDID=none") ? "EDID not available" : chain}
          </p>
        )}
      </div>
    </div>
  );
}
