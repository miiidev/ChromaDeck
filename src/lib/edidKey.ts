/// EDID-key matching shared by every legacy record lookup (pins, applied
/// map, aliases). Mirrors the Rust `monitor::key_match` rule exactly:
/// exact key first; otherwise equal base identities (the key shape the
/// app used before port-qualified keys, where the first matching
/// monitor "was" the identity).

/** Clean EDID key from a possibly port-qualified one: cut at the first `|`. */
export function baseEdidKey(qualified: string): string {
  const i = qualified.indexOf("|");
  return i === -1 ? qualified : qualified.slice(0, i);
}

/** Whether `candidate` (enumerated monitor key) satisfies `target`
 *  (stored record key) under legacy first-match fallback semantics. */
export function edidKeyMatch(candidate: string, target: string): boolean {
  return candidate === target || baseEdidKey(candidate) === baseEdidKey(target);
}

/** Map lookup for possibly-qualified keys: exact entry first, then the
 *  first entry (insertion order) matching under the same fallback rule. */
export function valueFor(
  map: Record<string, string>,
  key: string,
): string | undefined {
  if (map[key] !== undefined) return map[key];
  for (const [entry, value] of Object.entries(map)) {
    if (edidKeyMatch(entry, key)) return value;
  }
  return undefined;
}
