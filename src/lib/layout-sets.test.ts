import { describe, expect, it } from 'vitest';
import { parseView, setHelp, setSummary, type LayoutSet, type LensView } from './layout-sets';

const set = (patch: Partial<LayoutSet> = {}): LayoutSet => ({
  id: 'a',
  name: 'Game',
  app: { key: 'game', name: 'Game' },
  auto_restore: true,
  windows: [
    { rect: { x: 0, y: 0, w: 1, h: 1 }, target_language: null, read_mode: 'subtitle' as const, view: null, attach: null },
    { rect: { x: 0, y: 0, w: 1, h: 1 }, target_language: null, read_mode: 'subtitle' as const, view: null, attach: null }
  ],
  saved_at: 0,
  active: false,
  ...patch
});

describe('layout set rows', () => {
  it('summarises the windows and the app', () => {
    expect(setSummary(set())).toBe('2 個・Game');
    expect(setSummary(set({ app: null }))).toBe('2 個・アプリ指定なし');
  });

  it('explains when a set comes back', () => {
    expect(setHelp(set())).toBe('Game の起動時に自動復元、終了時に自動終了。');
    expect(setHelp(set({ auto_restore: false }))).toBe('Game 用。自動復元は OFF。');
    expect(setHelp(set({ app: null, auto_restore: false }))).toBe(
      '吸着先なしのため、復元は手動のみ。'
    );
    expect(setHelp(set({ active: true }))).toMatch(/^表示中。/);
  });
});

describe('the view state of a restored window', () => {
  const fallback: LensView = { continuous: true, text: false, placement: 'inside', speak: false };

  it('is what was saved', () => {
    const saved = { continuous: false, text: true, placement: 'bottom', speak: true };
    expect(parseView(saved, fallback)).toEqual(saved);
  });

  it('falls back per item for a window opened by hand or an older set', () => {
    expect(parseView(null, fallback)).toEqual(fallback);
    expect(parseView({ text: true }, fallback)).toEqual({ ...fallback, text: true });
    expect(parseView({ continuous: 'no', placement: 'diagonal' }, fallback)).toEqual(fallback);
  });
});
