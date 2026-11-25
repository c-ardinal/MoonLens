// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { applyAppearance, bodyZoom } from './appearance';

describe('本文画面の文字サイズ', () => {
  it('段階ごとの拡縮率（32px 設計基準）', () => {
    expect(bodyZoom('xsmall')).toBe(0.5);
    expect(bodyZoom('small')).toBe(0.625);
    expect(bodyZoom('medium')).toBe(0.75);
    expect(bodyZoom('large')).toBe(0.875);
    expect(bodyZoom('xlarge')).toBe(1);
  });
  it('未指定・未知の値は標準', () => {
    expect(bodyZoom(undefined)).toBe(0.75);
    expect(bodyZoom('huge')).toBe(0.75);
  });
  it('設定適用で CSS 変数を更新', () => {
    const root = document.createElement('div');
    applyAppearance({ appearance: { text_size: 'small' } }, root);
    expect(root.style.getPropertyValue('--ml-body-zoom')).toBe('0.625');
  });
});
