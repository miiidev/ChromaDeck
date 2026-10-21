/**
 * Pure presentation logic for an apply response.
 *
 * The backend reports one outcome per adjustment (applied / skipped / failed)
 * plus an honest `verified` flag (apply paths confirm writes, not panel
 * read-back). This module turns that into a compact, ordered summary the
 * Apply dialog can render without inventing success. Legacy responses without
 * `adjustments` fall back to the previous boolean copy.
 */

import type { AdjustmentOutcome, AdjustmentStatus, ApplyResult } from "./types";

export interface OutcomeRow {
  adjustment: string;
  label: string;
  status: AdjustmentStatus;
  verified: boolean;
  reason?: string;
}

export interface ApplySummary {
  rows: OutcomeRow[];
  /** One-line result used next to the existing confirmation copy. */
  headline: string;
  appliedCount: number;
  skippedCount: number;
  failedCount: number;
  /** A global error or any per-adjustment failure. */
  hasFailures: boolean;
  /** Applied rows exist but none were read-back verified. */
  unverifiedApplied: boolean;
  /** Footnote explaining the verification limit, or null. */
  verificationNote: string | null;
}

const LABELS: Record<string, string> = {
  icc: "ICC",
  gamma: "Gamma",
  brightness: "Brightness",
  contrast: "Contrast",
  rgb_gains: "RGB gains",
  saturation: "Saturation",
  vibrance: "Vibrance",
  hue: "Hue",
};

/** Readable label for a backend adjustment key (unknown keys degrade safely). */
export function adjustmentLabel(adjustment: string): string {
  return LABELS[adjustment] ?? adjustment.replace(/_/g, " ");
}

function legacyHeadline(result: ApplyResult): string {
  const applied = [
    result.icc_applied && "ICC applied",
    result.gamma_applied && "gamma applied",
    result.vibrance_applied && "vibrance applied",
  ].filter(Boolean);

  if (result.error) return "Apply failed";
  return applied.join(" + ") || "Applied";
}

/**
 * Summarize an apply response.
 *
 * - Per-adjustment rows are kept in backend order.
 * - Headline counts skips/failures instead of hiding them.
 * - Neutral adjustments are already omitted by the backend (nothing was
 *   requested), so a skipped row means a non-neutral value was not delivered.
 * - Without `adjustments` (legacy response) the old boolean copy is preserved.
 */
export function summarizeApply(result: ApplyResult): ApplySummary {
  const rows: OutcomeRow[] = (result.adjustments ?? []).map((outcome: AdjustmentOutcome) => ({
    adjustment: outcome.adjustment,
    label: adjustmentLabel(outcome.adjustment),
    status: outcome.status,
    verified: outcome.verified,
    reason: outcome.reason,
  }));

  const applied = rows.filter((row) => row.status === "applied");
  const skipped = rows.filter((row) => row.status === "skipped");
  const failed = rows.filter((row) => row.status === "failed");

  let headline: string;
  if (rows.length === 0) {
    headline = legacyHeadline(result);
  } else if (result.error && applied.length === 0) {
    headline = "Apply failed";
  } else if (failed.length > 0) {
    headline = `Applied with ${failed.length} failed`;
  } else if (skipped.length > 0) {
    headline = `Applied with ${skipped.length} skipped`;
  } else if (applied.length > 0) {
    headline = `Applied - ${applied.map((row) => row.label).join(", ")}`;
  } else {
    headline = "Applied";
  }

  const unverifiedApplied = applied.some((row) => !row.verified);

  return {
    rows,
    headline,
    appliedCount: applied.length,
    skippedCount: skipped.length,
    failedCount: failed.length,
    hasFailures: Boolean(result.error) || failed.length > 0,
    unverifiedApplied,
    verificationNote: unverifiedApplied
      ? "Writes accepted by the backend; not read-back verified - use Probe to check the panel."
      : null,
  };
}
