/**
 * Pure keyboard-event -> canonical shortcut formatter matching Rust `hotkeys::normalize_shortcut`.
 * Format: `Ctrl+Alt+Shift+Win+<Key>` (e.g. `Ctrl+Alt+F1`).
 */

const MODIFIER_KEYS = new Set([
  "Control",
  "Alt",
  "Shift",
  "Meta",
  "OS",
]);

function normalizeKey(e: { key: string; code?: string }): string | null {
  const k = e.key;
  if (!k || MODIFIER_KEYS.has(k)) return null;

  // Prefer physical Digit/Key codes when Alt/Shift alters `e.key` (e.g. Shift+1 -> '!')
  if (e.code) {
    if (/^Digit[0-9]$/.test(e.code)) return e.code.slice(5);
    if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3);
    if (/^F([1-9]|1[0-2])$/.test(e.code)) return e.code;
  }

  if (/^[a-zA-Z]$/.test(k)) return k.toUpperCase();
  if (/^[0-9]$/.test(k)) return k;
  if (/^F([1-9]|1[0-2])$/i.test(k)) return k.toUpperCase();

  switch (k) {
    case " ":
    case "Spacebar":
      return "Space";
    case "Home":
    case "End":
    case "Insert":
    case "Delete":
    case "PageUp":
    case "PageDown":
      return k;
    case "ArrowUp":
      return "Up";
    case "ArrowDown":
      return "Down";
    case "ArrowLeft":
      return "Left";
    case "ArrowRight":
      return "Right";
    case "-":
    case "=":
    case "[":
    case "]":
      return k;
    default:
      return null;
  }
}

export interface ShortcutKeyEvent {
  key: string;
  code?: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

/**
 * Convert a keydown event into a canonical shortcut string, or `null` when
 * the event is modifier-only, lacks modifiers, or uses an unsupported key.
 */
export function shortcutFromKeyEvent(e: ShortcutKeyEvent): string | null {
  if (!(e.ctrlKey || e.altKey || e.shiftKey || e.metaKey)) return null;
  const primary = normalizeKey(e);
  if (!primary) return null;

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Win");
  parts.push(primary);
  return parts.join("+");
}

/** Check whether another preset already owns `shortcut`. */
export function findShortcutConflict(
  shortcut: string,
  presets: Array<{ id: string; name: string; shortcut?: string | null }>,
  excludeId?: string,
): string | null {
  const target = shortcut.trim().toLowerCase();
  if (!target) return null;
  const owner = presets.find(
    (p) => p.id !== excludeId && (p.shortcut ?? "").trim().toLowerCase() === target,
  );
  return owner ? `${shortcut} is already bound to "${owner.name}"` : null;
}
