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

describe('外観設定の適用', () => {
  it('テーマ・アクセントカラー・表示密度を反映', () => {
    const root = document.createElement('div');
    applyAppearance({ appearance: { theme: 'light', accent: '#3b82f6', density: 'compact' } }, root);
    expect(root.dataset.scheme).toBe('light');
    expect(root.dataset.density).toBe('compact');
    expect(root.style.getPropertyValue('--ml-accent')).toBe('#3b82f6');
    const light = root.style.getPropertyValue('--ml-px-black');
    applyAppearance({ appearance: { theme: 'dark', accent: '#3b82f6' } }, root);
    expect(root.dataset.scheme).toBe('dark');
    expect(root.dataset.density).toBe('normal');
    expect(root.style.getPropertyValue('--ml-px-black')).not.toBe(light);
  });
  it('未設定は暗色・既定色', () => {
    const root = document.createElement('div');
    applyAppearance({}, root);
    expect(root.dataset.scheme).toBe('dark');
    expect(root.style.getPropertyValue('--ml-px-amber')).toBe('#ffb547');
  });
  it('モーション軽減の切替', () => {
    const root = document.createElement('div');
    applyAppearance({ appearance: { reduced_motion: true } }, root);
    expect(root.dataset.reducedMotion).toBe('true');
    applyAppearance({ appearance: { reduced_motion: false } }, root);
    expect(root.dataset.reducedMotion).toBeUndefined();
  });
});
