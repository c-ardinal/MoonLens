import { describe, expect, it, vi } from 'vitest';
import type { SubtitleLine } from './ipc';
import {
  cost,
  durationLabel,
  groupSentences,
  joinText,
  retranslate,
  timeLabel
} from './subtitle-record';

const line = (seq: number, start_ms: number, end_ms: number, original: string, translated = ''): SubtitleLine => ({
  seq,
  start_ms,
  end_ms,
  original,
  translated
});

describe('timeLabel / durationLabel', () => {
  it('formats M:SS and H:MM:SS', () => {
    expect(timeLabel(0)).toBe('0:00');
    expect(timeLabel(59_999)).toBe('0:59');
    expect(timeLabel(61_000)).toBe('1:01');
    expect(timeLabel(3_600_000)).toBe('1:00:00');
    expect(timeLabel(3_725_000)).toBe('1:02:05');
    expect(timeLabel(-5)).toBe('0:00');
  });
  it('uses the latest end time as duration', () => {
    expect(durationLabel([line(0, 0, 4000, 'a'), line(1, 4000, 125_000, 'b')])).toBe('2:05');
    expect(durationLabel([])).toBe('0:00');
  });
});

describe('joinText / cost', () => {
  it('inserts a space only between ASCII sides', () => {
    expect(joinText('Hello', 'world')).toBe('Hello world');
    expect(joinText('こんにちは', '世界')).toBe('こんにちは世界');
    expect(joinText('Hello', '世界')).toBe('Hello世界');
    expect(joinText('', 'a')).toBe('a');
  });
  it('costs ASCII 1 and others 3', () => {
    expect(cost('abc')).toBe(3);
    expect(cost('あい')).toBe(6);
    expect(cost('aあ')).toBe(4);
  });
});

describe('groupSentences', () => {
  it('ends a unit at sentence-final punctuation', () => {
    const units = groupSentences([
      line(0, 0, 1000, 'I think we should'),
      line(1, 1000, 2000, 'head back.'),
      line(2, 2000, 3000, 'Okay')
    ]);
    expect(units.map((u) => u.text)).toEqual(['I think we should head back.', 'Okay']);
    expect(units[0].lines).toHaveLength(2);
  });
  it('accepts closing quotes after the punctuation', () => {
    const units = groupSentences([line(0, 0, 1, '「行こう。」'), line(1, 1, 2, '彼は言った')]);
    expect(units.map((u) => u.text)).toEqual(['「行こう。」', '彼は言った']);
  });
  it('ends a unit at a gap over 1500 ms, not at exactly 1500 ms', () => {
    const over = groupSentences([line(0, 0, 1000, 'one'), line(1, 2501, 3000, 'two')]);
    expect(over).toHaveLength(2);
    const exact = groupSentences([line(0, 0, 1000, 'one'), line(1, 2500, 3000, 'two')]);
    expect(exact.map((u) => u.text)).toEqual(['one two']);
  });
  it('ends a unit when its cost passes 600', () => {
    const long = 'a'.repeat(400);
    const units = groupSentences([line(0, 0, 1, long), line(1, 1, 2, long), line(2, 2, 3, 'x')]);
    expect(units.map((u) => u.lines.length)).toEqual([2, 1]);
    // 200 CJK chars = cost 600: not yet past
    const cjk = 'あ'.repeat(200);
    expect(groupSentences([line(0, 0, 1, cjk), line(1, 1, 2, 'x')])).toHaveLength(1);
  });
  it('joins CJK without a space', () => {
    const units = groupSentences([line(0, 0, 1, '今日は'), line(1, 1, 2, '天気が'), line(2, 2, 3, 'いい')]);
    expect(units[0].text).toBe('今日は天気がいい');
  });
  it('returns nothing for no lines', () => {
    expect(groupSentences([])).toEqual([]);
  });
});

const sample = [
  line(0, 0, 1000, 'Hello there', '古い一'),
  line(1, 1000, 2000, 'my friend.', '古い二'),
  line(2, 5000, 6000, 'See you.', '古い三')
];

describe('retranslate', () => {
  it('translates per unit with accumulated history and renumbers', async () => {
    const calls: [string, [string, string][]][] = [];
    const translate = vi.fn(async (text: string, history: [string, string][]) => {
      calls.push([text, history]);
      return `訳:${text}`;
    });
    const progress: [number, number][] = [];
    const r = await retranslate(sample, translate, { onProgress: (d, t) => progress.push([d, t]) });
    expect(r.failed).toBe(0);
    expect(r.lines).toEqual([
      { seq: 0, start_ms: 0, end_ms: 2000, original: 'Hello there my friend.', translated: '訳:Hello there my friend.' },
      { seq: 1, start_ms: 5000, end_ms: 6000, original: 'See you.', translated: '訳:See you.' }
    ]);
    expect(calls[0][1]).toEqual([]);
    expect(calls[1][1]).toEqual([['Hello there my friend.', '訳:Hello there my friend.']]);
    expect(progress).toEqual([[1, 2], [2, 2]]);
  });

  it('keeps old translations for a failed or empty unit and counts it', async () => {
    const translate = vi
      .fn<(t: string, h: [string, string][]) => Promise<string>>()
      .mockRejectedValueOnce(new Error('boom'))
      .mockResolvedValueOnce('  ');
    const r = await retranslate(sample, translate);
    expect(r.failed).toBe(2);
    expect(r.lines.map((l) => l.translated)).toEqual(['古い一古い二', '古い三']);
  });

  it('rejects without a partial result when aborted', async () => {
    const ctl = new AbortController();
    const translate = vi.fn(async (t: string) => {
      ctl.abort();
      return t;
    });
    await expect(retranslate(sample, translate, { signal: ctl.signal })).rejects.toBeDefined();
    expect(translate).toHaveBeenCalledTimes(1);
  });

  it('rejects at once for an already aborted signal', async () => {
    const ctl = new AbortController();
    ctl.abort();
    const translate = vi.fn(async (t: string) => t);
    await expect(retranslate(sample, translate, { signal: ctl.signal })).rejects.toBeDefined();
    expect(translate).not.toHaveBeenCalled();
  });

  it('does not call translate for an empty original', async () => {
    const translate = vi.fn(async (t: string) => t);
    const r = await retranslate([line(0, 0, 1, '', '')], translate);
    expect(translate).not.toHaveBeenCalled();
    expect(r.failed).toBe(0);
    expect(r.lines).toHaveLength(1);
  });
});
