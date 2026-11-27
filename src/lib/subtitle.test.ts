import { describe, expect, it } from 'vitest';
import { MAX_LINES, initialView, reduce, statusLabel, type SubtitleView } from './subtitle';
import type { SubtitleEvent } from './ipc';

const run = (events: SubtitleEvent[], from: SubtitleView = initialView()) =>
  events.reduce(reduce, from);

describe('音声翻訳ウィンドウの表示状態', () => {
  it('開始から字幕表示までの状態表記', () => {
    let v = initialView();
    expect(statusLabel(v)).toBe('停止中');
    v = reduce(v, { kind: 'started', device: 'スピーカー' });
    expect(v.device).toBe('スピーカー');
    expect(statusLabel(v)).toBe('音声待機中');
    v = reduce(v, { kind: 'status', phase: 'recognizing' });
    expect(statusLabel(v)).toBe('認識中…');
    v = reduce(v, { kind: 'status', phase: 'translating' });
    expect(statusLabel(v)).toBe('翻訳中…');
    v = reduce(v, { kind: 'chunk', id: 1, original: 'Hello.', translated: 'こんにちは。' });
    expect(v.lines).toEqual([{ id: 1, original: 'Hello.', translated: 'こんにちは。' }]);
  });

  it('字幕は時系列で蓄積、上限超過分は旧字幕から破棄', () => {
    const events: SubtitleEvent[] = Array.from({ length: MAX_LINES + 5 }, (_, i) => ({
      kind: 'chunk',
      id: i + 1,
      original: `o${i}`,
      translated: `t${i}`
    }));
    const v = run(events);
    expect(v.lines).toHaveLength(MAX_LINES);
    expect(v.lines[0].original).toBe('o5');
    expect(v.lines.at(-1)?.id).toBe(MAX_LINES + 5);
    expect(new Set(v.lines.map((l) => l.id)).size).toBe(MAX_LINES);
  });

  it('エラーは次の字幕で解消', () => {
    let v = run([
      { kind: 'started', device: '' },
      { kind: 'error', message: '音声認識失敗：x' }
    ]);
    expect(v.error).toBe('音声認識失敗：x');
    expect(statusLabel(v)).toBe('音声待機中');
    v = reduce(v, { kind: 'chunk', id: 1, original: 'a', translated: 'b' });
    expect(v.error).toBeNull();
  });

  it('連続失敗による停止後もエラー表示を継続', () => {
    const v = run([
      { kind: 'started', device: '' },
      { kind: 'chunk', id: 1, original: 'a', translated: 'b' },
      { kind: 'error', message: '音声認識の連続失敗により停止：x' },
      { kind: 'stopped' }
    ]);
    expect(v.running).toBe(false);
    expect(v.error).toContain('停止');
    expect(v.lines).toHaveLength(1);
    expect(statusLabel(v)).toBe('エラー停止');
  });

  it('再開始で前回の字幕・エラーを消去', () => {
    const v = run([
      { kind: 'chunk', id: 1, original: 'a', translated: 'b' },
      { kind: 'error', message: 'x' },
      { kind: 'stopped' },
      { kind: 'started', device: 'd' }
    ]);
    expect(v).toEqual({ ...initialView(), running: true, device: 'd' });
  });

  it('文単位の再翻訳結果で該当行を置換', () => {
    const chunk = (id: number, original: string): SubtitleEvent => ({
      kind: 'chunk',
      id,
      original,
      translated: `<${original}>`
    });
    let v = run([
      { kind: 'started', device: '' },
      chunk(1, 'ask not what'),
      chunk(2, 'your country can do. Ask'),
      chunk(3, 'what you can do')
    ]);
    v = reduce(v, { kind: 'status', phase: 'revising' });
    expect(statusLabel(v)).toBe('補正中…');
    // 行 1・2 → 文 1 行＋次文の冒頭。後続の行 3 は位置を維持。
    v = reduce(v, {
      kind: 'revised',
      replaces: [1, 2],
      id: 4,
      original: 'ask not what your country can do.',
      translated: '国に求めるな。',
      rest: { id: 5, original: 'Ask', translated: '問え' }
    });
    expect(v.lines.map((l) => [l.id, l.translated, !!l.revised])).toEqual([
      [4, '国に求めるな。', true],
      [5, '問え', false],
      [3, '<what you can do>', false]
    ]);
    // 次文：冒頭行（5）と行 3 を置換。
    v = reduce(v, {
      kind: 'revised',
      replaces: [5, 3],
      id: 6,
      original: 'Ask what you can do',
      translated: '可能な貢献を問え',
      rest: null
    });
    expect(v.lines.map((l) => l.id)).toEqual([4, 6]);
    // 破棄済みの行に対する補正は無視。
    const same = reduce(v, {
      kind: 'revised',
      replaces: [1, 2],
      id: 9,
      original: 'x',
      translated: 'y',
      rest: null
    });
    expect(same).toBe(v);
  });
});
