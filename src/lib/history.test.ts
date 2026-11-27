import { describe, expect, it } from 'vitest';
import type { HistoryEntry } from './ipc';
import { applyFilter, countByFilter, filterOf, kindLabel, nextSelection } from './history';

const row = (id: number, kind: string): HistoryEntry => ({
  id,
  created_at: 0,
  kind,
  source_text: null,
  result_text: null,
  provider: null,
  via_cloud: false,
  image_path: null
});

const rows = [
  row(1, 'capture'),
  row(2, 'translate'),
  row(3, 'lens'),
  row(4, 'live'),
  row(5, 'text'),
  row(6, 'qr'),
  row(7, 'explain'),
  row(8, 'image'),
  row(9, 'subtitle'),
  row(10, 'asr')
];

describe('history filter', () => {
  it('treats legacy "translate" rows as window translations', () => {
    expect(filterOf('translate')).toBe('window');
    expect(kindLabel('translate')).toBe('ウィンドウ翻訳');
  });

  it('filters by kind', () => {
    expect(applyFilter(rows, 'all')).toHaveLength(10);
    // One-off, continuous and pre-merge live rows share the window tab.
    expect(applyFilter(rows, 'window').map((e) => e.id)).toEqual([1, 2, 3, 4]);
    expect(applyFilter(rows, 'image').map((e) => e.id)).toEqual([8]);
    expect(applyFilter(rows, 'text').map((e) => e.id)).toEqual([5]);
    expect(applyFilter(rows, 'qr').map((e) => e.id)).toEqual([6]);
    expect(applyFilter(rows, 'audio').map((e) => e.id)).toEqual([9, 10]);
  });

  it('counts rows per filter; uncovered kinds count only in "all"', () => {
    expect(countByFilter(rows)).toEqual({ all: 10, window: 4, image: 1, text: 1, qr: 1, audio: 2 });
  });

  it('labels subtitle rows as audio translation', () => {
    expect(kindLabel('subtitle')).toBe('音声翻訳');
    expect(filterOf('subtitle')).toBe('audio');
  });

  it('shows unknown kinds as is', () => {
    expect(kindLabel('plugin.x')).toBe('plugin.x');
    expect(filterOf('plugin.x')).toBeNull();
  });
});

describe('nextSelection', () => {
  it('keeps a visible selection', () => {
    expect(nextSelection(rows, 3)).toBe(3);
  });
  it('falls back to the first visible row', () => {
    expect(nextSelection(applyFilter(rows, 'image'), 1)).toBe(8);
    expect(nextSelection(rows, null)).toBe(1);
  });
  it('is null for an empty list', () => {
    expect(nextSelection([], 1)).toBeNull();
  });
});
