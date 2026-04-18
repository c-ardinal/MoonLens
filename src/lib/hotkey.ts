/**
 * Hotkey string helpers. The backend uses Tauri's accelerator grammar
 * (`CommandOrControl+Shift+Space`); the UI prefers platform-native
 * symbols (`⌘⇧Space`). Both conversions are pure and worth testing so
 * the display never drifts from what actually fires.
 */

const SYMBOLS: Record<string, string> = {
  CommandOrControl: '⌘',
  Cmd: '⌘',
  Command: '⌘',
  Control: '⌃',
  Ctrl: '⌃',
  Option: '⌥',
  Alt: '⌥',
  Shift: '⇧',
  Meta: '⌘',
  Super: '⌘',
  Space: 'Space',
  Escape: 'Esc',
  Return: '↵',
  Enter: '↵',
  Tab: '⇥',
  Backspace: '⌫',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→'
};

/** Convert a Tauri accelerator to a compact display string. */
export function formatAccelerator(accel: string): string {
  if (!accel) return '';
  return accel
    .split('+')
    .map((part) => SYMBOLS[part] ?? part)
    .join('');
}

/** Validate an accelerator string (shallow — no key-code table). */
export function isValidAccelerator(accel: string): boolean {
  if (!accel) return false;
  const parts = accel.split('+').map((p) => p.trim());
  if (parts.some((p) => p.length === 0)) return false;
  // At least one modifier + one key.
  const modifiers = new Set([
    'CommandOrControl',
    'Cmd',
    'Command',
    'Control',
    'Ctrl',
    'Option',
    'Alt',
    'Shift',
    'Meta',
    'Super'
  ]);
  const mods = parts.filter((p) => modifiers.has(p));
  const keys = parts.filter((p) => !modifiers.has(p));
  return mods.length >= 1 && keys.length === 1;
}
