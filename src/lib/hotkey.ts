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

/** All hotkey action IDs we surface in the UI. Each matches a field
 *  on the `Hotkeys` settings struct (or the loose `custom` map). */
export type HotkeyAction =
  | 'capture_region'
  | 'capture_window'
  | 'toggle_pin_overlay'
  | 'toggle_subtitle'
  | 'open_command_palette'
  | 'open_history'
  | 'reverse_image_search'
  | 'stop_all'
  | 'new_lens'
  | 'open_settings'
  | 'open_home';

/**
 * Resolve the formatted hotkey for an action from a settings tree.
 *
 * Returns the empty string when the user hasn't bound the action
 * (every UI consumer is expected to hide its kbd chip in that case
 * rather than render a misleading default). Callers receive the
 * settings snapshot from the global `currentSettings` store, which
 * Svelte subscribes to so the displayed hotkey re-renders the
 * instant the user changes it in Settings — no remounting required.
 */
export function hotkeyFor(
  action: HotkeyAction,
  settings: unknown | null | undefined,
): string {
  if (!settings || typeof settings !== 'object') return '';
  const hk = (settings as { hotkeys?: Record<string, unknown> }).hotkeys;
  if (!hk || typeof hk !== 'object') return '';
  const raw = (hk as Record<string, unknown>)[action];
  if (typeof raw !== 'string') return '';
  return formatAccelerator(raw.trim());
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
