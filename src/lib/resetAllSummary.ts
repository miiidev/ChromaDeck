/// Human-readable outcome summary for the Reset-all action.
/// Exact format pinned by tests: "Reset all: <N> OK · <M> failed · <P> pins removed".
export function resetAllSummary(
  results: Pick<ApplyResultLike, "error">[],
  pinsRemoved: number,
): string {
  const ok = results.filter((r) => !r.error).length;
  const failed = results.length - ok;
  return `Reset all: ${ok} OK · ${failed} failed · ${pinsRemoved} pins removed`;
}

interface ApplyResultLike {
  error?: string | null;
}
