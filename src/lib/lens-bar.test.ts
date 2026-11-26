import { describe, expect, it } from 'vitest';
import { fitCount } from './lens-bar';

describe('fitCount', () => {
  const widths = [24, 24, 24, 70, 24];
  const all = 24 * 4 + 70 + 4; // 間隔 1px × 4

  it('shows every item when the row is wide enough', () => {
    expect(fitCount(widths, 1, all, 24)).toBe(5);
    expect(fitCount(widths, 1, 1000, 24)).toBe(5);
  });

  it('keeps room for the menu button once an item does not fit', () => {
    // 1px 不足：メニューボタンの幅を確保するため、幅広の項目以降を収納
    expect(fitCount(widths, 1, all - 1, 24)).toBe(3);
    // 24+1 + 24+1 + 24+1 = 75、メニュー 24 → 99
    expect(fitCount(widths, 1, 99, 24)).toBe(3);
    expect(fitCount(widths, 1, 98, 24)).toBe(2);
  });

  it('never shows an item after a hidden one', () => {
    // 幅広の項目（言語選択）が収納不可の場合、後続の狭い項目も縦メニューへ収納
    expect(fitCount([24, 70, 24], 1, 24 + 1 + 24 + 30, 24)).toBe(1);
  });

  it('shows nothing but the menu button in a very narrow row', () => {
    expect(fitCount(widths, 1, 30, 24)).toBe(0);
    expect(fitCount(widths, 1, 0, 24)).toBe(0);
  });

  it('handles an empty row', () => {
    expect(fitCount([], 1, 0, 24)).toBe(0);
  });
});
