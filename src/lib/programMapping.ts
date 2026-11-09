/**
 * Pure helpers for mapping executables to presets (auto-apply).
 *
 * The backend enforces one-program-one-preset; these helpers give the editor
 * the same rule up front so the user sees the conflict inline instead of after
 * a failed save.
 */

import type { Preset } from "./types";

/**
 * Reduce a typed name or picked path to the stored identity: trimmed base
 * name, lowercased. Returns null for empty input or a name containing
 * whitespace (a path with spaces is fine because the directory part is
 * dropped first).
 */
export function normalizeProgramName(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  const base = trimmed.split(/[\\/]/).pop()?.trim() ?? "";
  if (!base || /\s/.test(base)) return null;
  return base.toLowerCase();
}

/**
 * Name of the other preset that already owns `program`, or null when the
 * program is free. `selfId` is the preset being edited so its own mappings
 * never collide with themselves.
 */
export function duplicateOwner(
  presets: Preset[],
  program: string,
  selfId?: string,
): string | null {
  const name = normalizeProgramName(program);
  if (!name) return null;
  const owner = presets.find(
    (preset) =>
      preset.id !== selfId && (preset.attached_programs ?? []).includes(name),
  );
  return owner ? owner.name : null;
}

/**
 * Add a program to the draft mapping list. Duplicates already in this list
 * are ignored silently; a program owned by another preset returns an error
 * naming that preset and leaves the list untouched.
 */
export function addProgram(
  list: string[],
  raw: string,
  presets: Preset[],
  selfId?: string,
): { list: string[]; error?: string } {
  const name = normalizeProgramName(raw);
  if (!name) {
    return { list, error: "Enter an executable name like chrome.exe" };
  }
  if (list.includes(name)) return { list };

  const owner = duplicateOwner(presets, name, selfId);
  if (owner) {
    return { list, error: `${name} is already mapped to "${owner}"` };
  }
  return { list: [...list, name] };
}
