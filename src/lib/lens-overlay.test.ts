import { describe, expect, it } from 'vitest';
import {
  allSameLanguage,
  applyStreaming,
  blockCssRect,
  dragRect,
  growRoom,
  joinedText,
  languageLabel,
  languageName,
  maskClip,
  rgb,
  splitStreaming,
  toCssRect,
  toFrameRect,
  toolbarPosition
} from './lens-overlay';
import type { LensBlock } from './ipc';

const block = (over: Partial<LensBlock> = {}): LensBlock => ({
  bbox: [10, 20, 100, 40],
  line_height: 16,
  source: 'src',
  translation: 'tr',
  colors: null,
  ...over
});

describe('dragRect', () => {
  it('normalises any drag direction', () => {
    expect(dragRect(50, 60, 10, 20)).toEqual({
      left: 10,
      top: 20,
      width: 40,
      height: 40
    });
  });
});

describe('frame <-> css conversion', () => {
  it('scales by device pixel ratio and round-trips', () => {
    const f = toFrameRect({ left: 10.4, top: 20, width: 100, height: 50 }, 2);
    expect(f).toEqual({ x: 21, y: 40, w: 200, h: 100 });
    expect(toCssRect({ x: 20, y: 40, w: 200, h: 100 }, 2)).toEqual({
      left: 10,
      top: 20,
      width: 100,
      height: 50
    });
  });

  it('never produces negative coordinates', () => {
    expect(toFrameRect({ left: -5, top: -5, width: 10, height: 10 }, 1)).toEqual({
      x: 0,
      y: 0,
      w: 5,
      h: 5
    });
  });
});

describe('blockCssRect', () => {
  it('offsets by the crop origin and pads', () => {
    const r = blockCssRect(block(), { x: 100, y: 200, w: 300, h: 300 }, 2, 2);
    expect(r).toEqual({ left: 53, top: 108, width: 54, height: 24 });
  });
});

describe('splitStreaming', () => {
  it('fills blocks as paragraphs arrive', () => {
    expect(splitStreaming('', 2)).toEqual([null, null]);
    expect(splitStreaming('こんに', 2)).toEqual(['こんに', null]);
    expect(splitStreaming('こんにちは\n\n二つ', 2)).toEqual(['こんにちは', '二つ']);
  });

  it('gives everything to a single block', () => {
    expect(splitStreaming('a\n\nb', 1)).toEqual(['a\n\nb']);
  });

  it('folds overflow paragraphs into the last block', () => {
    expect(splitStreaming('a\n\nb\n\nc', 2)).toEqual(['a', 'b c']);
  });

  it('handles zero blocks', () => {
    expect(splitStreaming('x', 0)).toEqual([]);
  });
});

describe('toolbarPosition', () => {
  const vp = { width: 1000, height: 800 };
  const bar = { width: 300, height: 40 };
  it('goes below the selection when there is room', () => {
    expect(toolbarPosition({ left: 100, top: 100, width: 200, height: 100 }, bar, vp)).toEqual({
      left: 50,
      top: 210
    });
  });
  it('flips above when the selection touches the bottom', () => {
    const p = toolbarPosition({ left: 100, top: 600, width: 200, height: 190 }, bar, vp);
    expect(p.top).toBe(550);
  });
  it('stays inside when the selection is full-screen', () => {
    const p = toolbarPosition({ left: 0, top: 0, width: 1000, height: 800 }, bar, vp);
    expect(p.top).toBe(750);
    expect(p.left).toBe(350);
  });
  it('clamps horizontally', () => {
    const p = toolbarPosition({ left: 950, top: 10, width: 40, height: 40 }, bar, vp);
    expect(p.left).toBe(690);
  });
});

describe('misc', () => {
  it('rgb formats colours with fallback', () => {
    expect(rgb([1, 2, 3], 'x')).toBe('rgb(1, 2, 3)');
    expect(rgb(null, 'white')).toBe('white');
  });
  it('joinedText falls back to source for untranslated blocks', () => {
    const bs = [block({ translation: 'A' }), block({ source: 'b', translation: null })];
    expect(joinedText(bs, 'translation')).toBe('A\n\nb');
    expect(joinedText(bs, 'source')).toBe('src\n\nb');
  });
});

describe('target language', () => {
  it('streams only into blocks that were sent for translation', () => {
    const bs = [
      block({ source: 'a', translation: null }),
      block({ source: 'すでに日本語', translation: 'すでに日本語', same_language: true }),
      block({ source: 'b', translation: null })
    ];
    const out = applyStreaming(bs, 'A\n\nB');
    expect(out.map((b) => b.translation)).toEqual(['A', 'すでに日本語', 'B']);
    expect(applyStreaming(bs, 'A').map((b) => b.translation)).toEqual(['A', 'すでに日本語', null]);
  });

  it('streams into the blocks of the current part only', () => {
    const bs = [
      block({ source: 'a', translation: 'A' }),
      block({ source: 'b', translation: null }),
      block({ source: 'c', translation: null })
    ];
    // The first part is done; the second request covers block 1 only.
    expect(applyStreaming(bs, 'B', [1]).map((b) => b.translation)).toEqual(['A', 'B', null]);
    expect(applyStreaming(bs, 'B\n\nC', [1, 2]).map((b) => b.translation)).toEqual(['A', 'B', 'C']);
  });

  it('skips cached blocks too', () => {
    const bs = [
      block({ source: 'a', translation: 'キャッシュ', from_cache: true }),
      block({ source: 'b', translation: null })
    ];
    expect(applyStreaming(bs, 'B').map((b) => b.translation)).toEqual(['キャッシュ', 'B']);
  });

  it('labels languages', () => {
    expect(languageLabel('ja')).toBe('日本語');
    expect(languageLabel('EN')).toBe('英語');
    expect(languageLabel('en-US')).toBe('英語');
    expect(languageLabel('zh')).toBe('中国語');
    expect(languageLabel('xx')).toBe('xx');
    expect(languageLabel(null)).toBe('');
  });

  it('resolves regional tags to the Japanese name used in prompts', () => {
    expect(languageName('en-US')).toBe('英語');
    expect(languageName('fr_FR')).toBe('フランス語');
    expect(languageName('zh-CN')).toBe('中国語');
    expect(languageName('xx')).toBe('');
  });

  it('detects when nothing needed translating', () => {
    expect(allSameLanguage([])).toBe(false);
    expect(allSameLanguage([block({ same_language: true })])).toBe(true);
    expect(allSameLanguage([block({ same_language: true }), block()])).toBe(false);
  });
});

describe('growRoom', () => {
  const b = (bbox: [number, number, number, number], lines = 1): LensBlock =>
    block({ bbox, lines: Array.from({ length: lines }, () => bbox) });
  it('lets a single-row label grow by its width, at least 8 line heights', () => {
    expect(growRoom([b([10, 10, 300, 20])], 0, 1000)).toBe(300);
    expect(growRoom([b([10, 10, 60, 20])], 0, 1000)).toBe(160);
  });
  it('stops before the next block on the same row and at the edge', () => {
    const blocks = [b([10, 10, 60, 20]), b([100, 12, 50, 20])];
    expect(growRoom(blocks, 0, 1000)).toBe(100 - 20 * 0.4 - 70);
    expect(growRoom([b([10, 10, 60, 20])], 0, 90)).toBe(20);
  });
  it('keeps paragraphs at their width', () => {
    expect(growRoom([b([10, 10, 300, 60], 3)], 0, 1000)).toBe(0);
  });
});

describe('maskClip', () => {
  const rows: [number, number, number, number][] = [
    [10, 10, 180, 20],
    [10, 40, 180, 20],
    [10, 70, 380, 20]
  ];
  const base = block({ bbox: [10, 10, 380, 80], lines: rows });

  it('leaves the mask whole when no row is limited', () => {
    expect(maskClip(base, 1)).toBe('');
    expect(maskClip({ ...base, row_limits: [390, 390, 390] }, 1)).toBe('');
  });

  it('ends the bands of limited rows at their limit', () => {
    // Picture from x = 200 beside the first two rows; pad 2 px.
    const clip = maskClip({ ...base, row_limits: [200, 200, 390] }, 1);
    expect(clip).toBe(
      'polygon(0px 0px, 194px 0px, 194px 27px, 194px 27px, 194px 57px, 384px 57px, 384px 84px, 0px 84px)'
    );
  });

  it('scales with the device pixel ratio', () => {
    const clip = maskClip({ ...base, row_limits: [200, 200, 390] }, 2);
    expect(clip.startsWith('polygon(0px 0px, 99px 0px, 99px 14.5px')).toBe(true);
  });
});
