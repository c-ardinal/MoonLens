/**
 * Display of extra triggers (mouse button, double tap of a modifier key,
 * gamepad combination). The text form comes from the backend
 * (`src-tauri/src/trigger.rs`): `Control+Mouse4`, `Double:Shift`,
 * `Pad:Back+Y`. The feature is Windows only, so key names are PC names.
 */

import { t } from './i18n';

const KEYS: Record<string, string> = { Control: 'Ctrl', Alt: 'Alt', Shift: 'Shift' };

const MOUSE: Record<string, string> = {
  MouseMiddle: 'trigger:mouse_middle',
  Mouse4: 'trigger:mouse_4',
  Mouse5: 'trigger:mouse_5'
};

/** Text shown for a trigger; text that is not a trigger is shown as is. */
export function displayTrigger(trigger: string): string {
  if (!trigger) return '';
  if (trigger.startsWith('Double:')) {
    const key = trigger.slice('Double:'.length);
    return t('trigger:double_tap', { key: KEYS[key] ?? key });
  }
  if (trigger.startsWith('Pad:')) {
    return t('trigger:pad', { buttons: trigger.slice('Pad:'.length) });
  }
  const parts = trigger.split('+');
  const button = MOUSE[parts[parts.length - 1]];
  if (!button) return trigger;
  return [...parts.slice(0, -1).map((p) => KEYS[p] ?? p), t(button)].join('+');
}
