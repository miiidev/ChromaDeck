import { type ReactNode } from "react";

/** Neutral-dimming stat tile: label + mono value, dimmed when default. */
interface StatTileProps {
  label: string;
  value: string;
  neutral: boolean;
}

export default function StatTile({ label, value, neutral }: StatTileProps): ReactNode {
  // Long values (e.g. gamma "2.80") shrink instead of overflowing the tile
  // at narrow grid columns. Grid items also need min-w-0 to shrink at all.
  const valueSize =
    value.length > 4 ? "text-[22px]" : value.length > 3 ? "text-[26px]" : "text-[30px]";
  return (
    <div className="flex flex-col items-stretch gap-1 min-w-0 overflow-hidden rounded-[10px] bg-[#151a26] px-3 py-[9px]">
      <span className="text-[10px] font-mono font-medium tracking-[0.12em] uppercase text-[#8b92a6] truncate">
        {label}
      </span>
      <span
        className={`font-mono font-bold ${valueSize}/[1] tabular-nums whitespace-nowrap`}
        style={{ color: neutral ? "#6b7285" : "#f2f5fa" } as Record<string, string>}
      >
        {value}
      </span>
    </div>
  );
}