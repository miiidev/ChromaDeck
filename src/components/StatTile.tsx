import { type ReactNode } from "react";

/** Elevated stat tile: label + mono value, dimmed when neutral. Double-Bezel inner. */
interface StatTileProps {
  label: string;
  value: string;
  neutral: boolean;
  size?: "16" | "20" | "24" | "32";
}

const valueSizes = {
  "16": "text-[18px]",
  "20": "text-[22px]",
  "24": "text-[26px]",
  "32": "text-[30px]",
} as const;

export default function StatTile({ label, value, neutral, size = "24" }: StatTileProps): ReactNode {
  const valueSize = valueSizes[size] ?? "text-[24px]";
  return (
    <div className="flex flex-col items-stretch gap-1 min-w-0 overflow-hidden rounded-brutal border border-border bg-inset px-3 py-2.5">
      <span className="text-[10px] font-mono font-bold tracking-[0.12em] uppercase text-muted-foreground truncate">
        {label}
      </span>
      <span
        className={`font-mono font-bold ${valueSize}/[1] tabular-nums whitespace-nowrap ${neutral ? "text-muted-foreground" : "text-foreground"} value-swap`}
      >
        {value}
      </span>
    </div>
  );
}