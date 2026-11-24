/**
 * Hotkey string helpers. The backend uses Tauri's accelerator grammar
 * (`CommandOrControl+Shift+Space`); the UI prefers platform-native
 * symbols (`⌘⇧Space`). Both conversions are pure and worth testing so
 * the display never drifts from what actually fires.
 */

const SYMBOLS: Record<string, string> = {
  CommandOrControl: '⌘',
  CmdOrCtrl: '⌘',
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

/** Names used on Windows / Linux, where ⌘-style symbols mean nothing. */
const PC_NAMES: Record<string, string> = {
  CommandOrControl: 'Ctrl',
  CmdOrCtrl: 'Ctrl',
  Cmd: 'Ctrl',
  Command: 'Ctrl',
  Control: 'Ctrl',
  Ctrl: 'Ctrl',
  Option: 'Alt',
  Alt: 'Alt',
  Shift: 'Shift',
  Meta: 'Win',
  Super: 'Win',
  Escape: 'Esc',
  Return: 'Enter',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→'
};

/** True on macOS (where shortcuts are shown as ⌘⇧…). */
export function isMac(): boolean {
  if (typeof navigator === 'undefined') return true;
  return /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent);
}

/**
 * Convert a Tauri accelerator to a display string: compact symbols on
 * macOS (`⌘⇧Space`), `Ctrl+Shift+Space` elsewhere.
 */
export function formatAccelerator(accel: string, mac = true): string {
  if (!accel) return '';
  const parts = accel.split('+');
  if (mac) return parts.map((part) => SYMBOLS[part] ?? part).join('');
  return parts.map((part) => PC_NAMES[part] ?? part).join('+');
}

/** `formatAccelerator` for the platform the app is running on. */
export function displayAccelerator(accel: string): string {
  return formatAccelerator(accel, isMac());
}

const KEY_NAMES: Record<string, string> = {
  ' ': 'Space',
  Spacebar: 'Space',
  Esc: 'Escape',
  Enter: 'Enter',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight'
};

/**
 * Build a Tauri accelerator from a key press, for the shortcut recorder.
 * Returns null while only modifiers are held, or for keys that cannot be
 * a global shortcut. ⌘ (macOS) and Ctrl (elsewhere) both map to
 * `CommandOrControl` so the binding works on every platform.
 */
export function acceleratorFromEvent(
  e: Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>
): string | null {
  if (['Control', 'Meta', 'Alt', 'Shift', 'OS', 'AltGraph'].includes(e.key)) return null;
  let key: string | null = null;
  if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.code)) key = e.code;
  else if (e.code === 'Space') key = 'Space';
  else if (KEY_NAMES[e.key]) key = KEY_NAMES[e.key];
  if (!key) return null;
  const mods: string[] = [];
  if (e.metaKey || e.ctrlKey) mods.push('CommandOrControl');
  if (e.altKey) mods.push('Alt');
  if (e.shiftKey) mods.push('Shift');
  return [...mods, key].join('+');
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
