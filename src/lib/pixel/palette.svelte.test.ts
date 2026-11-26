// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { PAL } from './pix';
import { applyPalette, buildPalette, hexToOklch, oklchToHex, palette } from './palette.svelte';

const lum = (hex: string) => hexToOklch(hex).l;

describe('配色の算出', () => {
  it('既定のアクセントカラーは基準配色（琥珀）と同一', () => {
    expect(buildPalette('dark', '#FFB547')).toEqual(buildPalette('dark'));
    expect(buildPalette('dark')[5]).toBe('#ffb547');
    expect(buildPalette('dark', 'not-a-colour')).toEqual(buildPalette('dark'));
  });
  it('色変換の往復で同一色', () => {
    for (const hex of ['#ffb547', '#0b0805', '#3b82f6', '#22c55e', '#ffffff']) {
      expect(oklchToHex(hexToOklch(hex))).toBe(hex);
    }
  });
  it('アクセントカラー変更：色相を反映し、各段の明度を維持', () => {
    const base = buildPalette('dark');
    const blue = buildPalette('dark', '#3b82f6');
    const want = hexToOklch('#3b82f6').h;
    for (let i = 0; i < 7; i++) {
      expect(Math.abs(lum(blue[i]) - lum(base[i]))).toBeLessThan(0.02);
    }
    expect(Math.abs(hexToOklch(blue[5]).h - want)).toBeLessThan(0.15);
    // 異常・正常・強調の 3 色は固定
    expect(blue.slice(7)).toEqual(base.slice(7));
  });
  it('無彩色のアクセントカラーは全段が無彩色', () => {
    for (const hex of buildPalette('dark', '#808080').slice(0, 7)) {
      expect(hexToOklch(hex).c).toBeLessThan(0.01);
    }
  });
  it('明色・暗色とも文字（6）・主色（5）は地（0）と十分な明度差', () => {
    for (const scheme of ['dark', 'light'] as const)
      for (const accent of ['#ffb547', '#3b82f6', '#22c55e', '#ef4444', '#000000', '#ffffff']) {
        const p = buildPalette(scheme, accent);
        expect(Math.abs(lum(p[6]) - lum(p[0]))).toBeGreaterThan(0.6);
        expect(Math.abs(lum(p[5]) - lum(p[0]))).toBeGreaterThan(0.4);
      }
    expect(lum(buildPalette('light')[0])).toBeGreaterThan(0.9);
    expect(lum(buildPalette('dark')[0])).toBeLessThan(0.2);
  });
});

describe('配色の適用', () => {
  it('CSS 変数・Canvas 用パレット・通知を同時に更新', () => {
    const root = document.createElement('div');
    applyPalette('dark', '#ffb547', root);
    const before = palette.version;
    applyPalette('light', '#3b82f6', root);
    const want = buildPalette('light', '#3b82f6');
    expect(root.dataset.scheme).toBe('light');
    expect(root.style.getPropertyValue('--ml-px-black')).toBe(want[0]);
    expect(root.style.getPropertyValue('--ml-px-amber')).toBe(want[5]);
    expect([...PAL]).toEqual(want);
    expect(palette.version).toBe(before + 1);
    // 同一配色の再適用は通知なし
    applyPalette('light', '#3b82f6', root);
    expect(palette.version).toBe(before + 1);
    applyPalette('dark', undefined, root);
  });
});
