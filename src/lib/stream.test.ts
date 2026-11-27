import { describe, expect, it } from 'vitest';
import type { FlowResult } from './flow-layout';
import type { LensBlock } from './ipc';
import { liveBoxes } from './live';
import { streamItems } from './stream';

const block = (over: Partial<LensBlock> = {}): LensBlock => ({
  bbox: [40, 20, 200, 40],
  line_height: 20,
  source: 'Hello',
  translation: 'こんにちは',
  colors: null,
  ...over
});

const layout = (over: Partial<FlowResult> = {}): FlowResult => ({
  fontSize: 9,
  overflow: false,
  lines: [{ text: 'こんにちは', left: 2, top: 2, width: 100, height: 20, textWidth: 45 }],
  ...over
});

describe('streamItems', () => {
  it('places boxes and lines in physical pixels of the client area', () => {
    const boxes = liveBoxes([block()], new Set(), 2);
    const [item] = streamItems(boxes, new Map([[boxes[0].rect.key, layout()]]), 2, 32);
    // Box: bbox / dpr with 2 CSS px of padding, below a 32 px header.
    expect(item.rect).toEqual([36, 80, 208, 48]);
    expect(item.font_px).toBe(18);
    expect(item.lines).toEqual([
      { text: 'こんにちは', rect: [40, 84, 200, 40], text_width: 90, backed: false }
    ]);
  });

  it('keeps the background off a picture beside the rows', () => {
    const rows: [number, number, number, number][] = [
      [10, 10, 180, 20],
      [10, 40, 380, 20]
    ];
    const limited = block({ bbox: [10, 10, 380, 50], lines: rows, row_limits: [200, 390] });
    const boxes = liveBoxes([limited, block()], new Set(), 1);
    const [item, whole] = streamItems(boxes, new Map(), 1, 32);
    // First row ends at the picture (x = 200, 2 px of padding each
    // side); the second takes the whole width.
    expect(item.rect).toEqual([8, 40, 384, 54]);
    expect(item.fills).toEqual([
      [8, 40, 194, 27],
      [8, 67, 384, 27]
    ]);
    expect(whole.fills).toEqual([]);
  });

  it('uses the sampled colours, or the window defaults', () => {
    const boxes = liveBoxes(
      [
        block({ source: 'a' }),
        block({ source: 'b', colors: { background: [1, 2, 3], foreground: [4, 5, 6] } })
      ],
      new Set(),
      1
    );
    const items = streamItems(boxes, new Map(), 1, 0);
    expect(items[0].background).toEqual([255, 255, 255]);
    expect(items[0].foreground).toEqual([17, 17, 17]);
    expect(items[1].background).toEqual([1, 2, 3]);
    expect(items[1].foreground).toEqual([4, 5, 6]);
  });

  it('sends a block that is still being translated as a box without text', () => {
    const boxes = liveBoxes([block({ translation: null })], new Set(), 1);
    const [item] = streamItems(boxes, new Map([[boxes[0].rect.key, layout()]]), 1, 0);
    expect(item.lines).toEqual([]);
  });

  it('marks lines that run past their box', () => {
    const boxes = liveBoxes([block()], new Set(), 1);
    const wide = layout({
      lines: [{ text: 'long', left: 0, top: 0, width: 50, height: 20, textWidth: 80 }]
    });
    const [item] = streamItems(boxes, new Map([[boxes[0].rect.key, wide]]), 1, 0);
    expect(item.lines[0].backed).toBe(true);
  });

  it('leaves out hidden blocks', () => {
    const boxes = liveBoxes([block({ source: 'a' }), block({ source: 'b' })], new Set([0]), 1);
    expect(streamItems(boxes, new Map(), 1, 0)).toHaveLength(1);
  });
});
