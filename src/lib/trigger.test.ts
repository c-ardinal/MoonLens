import { describe, expect, it } from 'vitest';
import { sourceText } from './i18n';
import { displayTrigger } from './trigger';

describe('displayTrigger', () => {
  it('names mouse buttons with their modifiers', () => {
    expect(displayTrigger('Mouse4')).toBe(sourceText('trigger:mouse_4'));
    expect(displayTrigger('Control+Shift+MouseMiddle')).toBe(`Ctrl+Shift+${sourceText('trigger:mouse_middle')}`);
  });

  it('names a double tap and a gamepad combination', () => {
    expect(displayTrigger('Double:Control')).toBe(sourceText('trigger:double_tap').replace('{key}', 'Ctrl'));
    expect(displayTrigger('Pad:Back+Y')).toBe(sourceText('trigger:pad').replace('{buttons}', 'Back+Y'));
  });

  it('shows empty and unknown text unchanged', () => {
    expect(displayTrigger('')).toBe('');
    expect(displayTrigger('Control+Q')).toBe('Control+Q');
  });
});
