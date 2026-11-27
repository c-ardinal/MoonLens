import { describe, expect, it } from 'vitest';
import { breakLines, flowLayout, tokenize } from './flow-layout';

// Monospace stand-in: CJK = 1 em, everything else = 0.5 em.
const measure = (t: string) =>
  Array.from(t).reduce((w, c) => w + (/[　-鿿＀-￯]/.test(c) ? 1 : 0.5), 0);

const rows = (n: number, pitch = 28, h = 20, left = 0, width = 400) =>
  Array.from({ length: n }, (_, i) => ({ left, top: i * pitch, width, height: h }));

describe('tokenize', () => {
  it('keeps closing punctuation and small kana off the line start', () => {
    const t = tokenize('北風は「強い」。ちょっと');
    expect(t.some((x) => x.startsWith('。'))).toBe(false);
    expect(t.some((x) => x.startsWith('」'))).toBe(false);
    expect(t.some((x) => x.startsWith('ょ'))).toBe(false);
    expect(t.some((x) => x.endsWith('「'))).toBe(false);
    expect(t.join('')).toBe('北風は「強い」。ちょっと');
  });

  it('splits Latin text into words and spaces', () => {
    expect(tokenize('Hello big world').filter((x) => x.trim())).toEqual(['Hello', 'big', 'world']);
  });
});

describe('breakLines', () => {
  it('wraps at word boundaries and drops trailing spaces', () => {
    const lines = breakLines(tokenize('aaaa bbbb cccc'), () => 9 * 0.5, measure);
    expect(lines).toEqual(['aaaa bbbb', 'cccc']);
  });

  it('splits a token wider than the line', () => {
    const lines = breakLines(['abcdefgh'], () => 2, measure);
    expect(lines).toEqual(['abcd', 'efgh']);
  });

  it('uses each line’s own width', () => {
    const lines = breakLines(tokenize('あいうえおかきくけこ'), (i) => (i === 0 ? 3 : 10), measure);
    expect(lines[0]).toBe('あいう');
  });
});

describe('flowLayout', () => {
  it('puts the translation on the source rows at the largest fitting size', () => {
    const r = flowLayout('あ'.repeat(60), {
      width: 400,
      height: 132,
      rows: rows(5),
      align: 'left',
      maxFont: 18,
      measure
    });
    expect(r.overflow).toBe(false);
    expect(r.fontSize).toBe(18);
    // 400 / 18 = 22 chars per row → 3 rows, at the source row positions.
    expect(r.lines.map((l) => l.top)).toEqual([0, 28, 56]);
    expect(r.lines.every((l) => l.height === 20)).toBe(true);
  });

  it('keeps a first-line indent', () => {
    const src = rows(3);
    src[0] = { ...src[0], left: 24, width: 376 };
    const r = flowLayout('あ'.repeat(40), {
      width: 400,
      height: 76,
      rows: src,
      align: 'left',
      maxFont: 16,
      measure
    });
    expect(r.lines[0].left).toBe(24);
    expect(r.lines[0].width).toBe(376);
    expect(r.lines[1].left).toBe(0);
  });

  it('shrinks the font before leaving the source rows', () => {
    // 50 chars over 2 rows of 400 px: 16 px keeps both rows.
    const r = flowLayout('あ'.repeat(50), {
      width: 400,
      height: 40,
      rows: rows(2, 20, 20),
      align: 'left',
      maxFont: 18,
      measure
    });
    expect(r.overflow).toBe(false);
    expect(r.fontSize).toBe(16);
    expect(r.lines.map((l) => l.top)).toEqual([0, 20]);
  });

  it('spreads the lines when the rows would cost much of the font size', () => {
    // 2 widely spaced rows: 100 chars need 8 px on them, 11.5 px spread out.
    const r = flowLayout('あ'.repeat(100), {
      width: 400,
      height: 48,
      rows: rows(2, 28, 20),
      align: 'left',
      maxFont: 18,
      measure
    });
    expect(r.overflow).toBe(true);
    expect(r.fontSize).toBe(11.5);
    expect(r.lines.length).toBe(3);
  });

  it('spreads over the box when a single-line source gets a long translation', () => {
    const r = flowLayout('あ'.repeat(30), {
      width: 100,
      height: 60,
      rows: [{ left: 0, top: 20, width: 100, height: 20 }],
      align: 'left',
      maxFont: 18,
      measure
    });
    expect(r.lines.length).toBeGreaterThan(1);
    expect(r.overflow).toBe(true);
    const last = r.lines[r.lines.length - 1];
    expect(last.top + last.height).toBeLessThanOrEqual(60.5);
  });

  it('uses the whole box as one row without row data', () => {
    const r = flowLayout('Hi', { width: 100, height: 20, rows: [], align: 'center', maxFont: 14, measure });
    expect(r.lines).toEqual([{ text: 'Hi', left: 0, top: 0, width: 100, height: 20, textWidth: 14 }]);
    expect(r.fontSize).toBe(14);
  });
});

describe('flowLayout grow', () => {
  it('lets a short label run to the right instead of shrinking', () => {
    const base = { width: 60, height: 20, rows: [], align: 'left' as const, maxFont: 16, measure };
    expect(flowLayout('メインページ', base).fontSize).toBe(10);
    const grown = flowLayout('メインページ', { ...base, grow: 60 });
    expect(grown.fontSize).toBe(16);
    expect(grown.lines[0].textWidth).toBe(96);
    expect(grown.lines[0].width).toBe(60);
  });
});

describe('flowLayout overflow', () => {
  it('keeps a readable pitch when nothing fits', () => {
    const r = flowLayout('あ'.repeat(100), { width: 100, height: 20, rows: [], align: 'left', maxFont: 14, measure });
    expect(r.fontSize).toBe(7);
    for (const [a, b] of r.lines.slice(1).map((l, i) => [r.lines[i], l])) {
      expect(b.top - a.top).toBeGreaterThanOrEqual(7 * 1.15 - 1e-9);
    }
  });
});

describe('flowLayout beside a picture', () => {
  it('ends the rows beside the picture at the picture', () => {
    // Two rows beside a picture that starts at x = 200, two rows below it.
    const r = flowLayout('あ'.repeat(50), {
      width: 400,
      height: 104,
      rows: rows(4).map((row, i) => (i < 2 ? { ...row, width: 190, limit: 200 } : row)),
      align: 'left',
      maxFont: 20,
      measure
    });
    expect(r.overflow).toBe(false);
    expect(r.fontSize).toBe(20);
    expect(r.lines.map((l) => l.text.length)).toEqual([10, 10, 20, 10]);
    expect(r.lines.map((l) => l.width)).toEqual([200, 200, 400, 400]);
  });

  it('keeps spread lines clear of the picture', () => {
    const r = flowLayout('あ'.repeat(40), {
      width: 400,
      height: 48,
      rows: rows(2).map((row) => ({ ...row, width: 190, limit: 200 })),
      align: 'left',
      maxFont: 20,
      measure
    });
    expect(r.lines.every((l) => l.textWidth <= 200.01)).toBe(true);
  });
});
