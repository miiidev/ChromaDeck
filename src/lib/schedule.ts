import type { ImportSummary, ScheduleRule } from "./types";

const HHMM_RE = /^([01]\d|2[0-3]):([0-5]\d)$/;

/** Parse 24h "HH:MM" into minutes since midnight, or null if invalid. */
export function parseHhmm(value: string): number | null {
  const m = HHMM_RE.exec(value.trim());
  if (!m) return null;
  return Number(m[1]) * 60 + Number(m[2]);
}

export function isValidHhmm(value: string): boolean {
  return parseHhmm(value) !== null;
}

/** True when a valid schedule wraps past midnight (e.g. 21:00 -> 07:00). */
export function isOvernightWindow(startTime: string, endTime: string): boolean {
  const s = parseHhmm(startTime);
  const e = parseHhmm(endTime);
  return s !== null && e !== null && s > e;
}

/** Client-side rule validation matching Rust `schedule::validate_rules`. */
export function validateScheduleRule(rule: ScheduleRule): string | null {
  if (!rule.edid_id.trim()) return "Choose a monitor";
  if (!rule.preset_id.trim()) return "Choose a preset";
  const s = parseHhmm(rule.start_time);
  if (s === null) return `Invalid start time "${rule.start_time}" (use HH:MM)`;
  const e = parseHhmm(rule.end_time);
  if (e === null) return `Invalid end time "${rule.end_time}" (use HH:MM)`;
  if (s === e) return "Start and end time cannot be the same";
  return null;
}

/** Concise window readout, e.g. "21:00 – 07:00 (overnight)". */
export function formatScheduleWindow(startTime: string, endTime: string): string {
  const base = `${startTime} – ${endTime}`;
  return isOvernightWindow(startTime, endTime) ? `${base} (overnight)` : base;
}

/** Concise one-line toast text for a bundle import outcome. */
export function formatImportSummary(summary: ImportSummary): string {
  const parts = [
    `Imported ${summary.imported_count} preset${summary.imported_count === 1 ? "" : "s"}`,
  ];
  if (summary.icc_imported_count > 0) {
    parts.push(`${summary.icc_imported_count} ICC`);
  }
  if (summary.renamed_collisions.length > 0) {
    parts.push(`${summary.renamed_collisions.length} renamed`);
  }
  const stripped =
    summary.stripped_conflicting_programs.length +
    summary.stripped_conflicting_shortcuts.length;
  if (stripped > 0) {
    parts.push(`${stripped} conflict${stripped === 1 ? "" : "s"} cleared`);
  }
  return parts.join(" · ");
}
