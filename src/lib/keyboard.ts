/** Map a DOM KeyboardEvent into Tauri's shortcut-string key segment. */
export function keyToTauri(e: KeyboardEvent): string | null {
  const key = e.key;
  if (key === ' ') return 'Space';
  if (key === 'Escape') return 'Escape';
  if (key === 'Tab') return 'Tab';
  if (key === 'Enter') return 'Enter';
  if (key === 'Backspace') return 'Backspace';
  if (key === 'ArrowUp') return 'Up';
  if (key === 'ArrowDown') return 'Down';
  if (key === 'ArrowLeft') return 'Left';
  if (key === 'ArrowRight') return 'Right';
  if (key === 'PageUp') return 'PageUp';
  if (key === 'PageDown') return 'PageDown';
  if (key === 'Home') return 'Home';
  if (key === 'End') return 'End';
  if (/^F\d{1,2}$/.test(key)) return key;
  if (key.length === 1) return key.toUpperCase();
  if (e.code && e.code.length > 0) return e.code;
  return null;
}

/** Build a Tauri shortcut string from a keydown event. Returns null if the
 *  event doesn't include a non-modifier key. */
export function buildShortcutString(e: KeyboardEvent): string | null {
  const mods: string[] = [];
  if (e.metaKey || e.ctrlKey) mods.push('CmdOrCtrl');
  if (e.altKey) mods.push('Alt');
  if (e.shiftKey) mods.push('Shift');

  const key = keyToTauri(e);
  if (!key) return null;
  return [...mods, key].join('+');
}

/** Pretty-print a Tauri shortcut string with macOS glyphs (⌘⇧⌃⌥). */
export function formatShortcut(s: string | null | undefined, fallback = '⌘⇧Space'): string {
  if (!s) return fallback;
  return s
    .split('+')
    .map((part) => {
      switch (part) {
        case 'CmdOrCtrl':
        case 'Cmd':
        case 'Command':
        case 'Meta':
          return '⌘';
        case 'Ctrl':
        case 'Control':
          return '⌃';
        case 'Alt':
        case 'Option':
          return '⌥';
        case 'Shift':
          return '⇧';
        default:
          return part;
      }
    })
    .join('');
}

/** True when a built shortcut string includes at least one modifier. */
export function hasModifier(s: string): boolean {
  return /(CmdOrCtrl|Alt|Shift)/.test(s);
}
