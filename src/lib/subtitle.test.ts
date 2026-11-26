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
    v = reduce(v, { kind: 'chunk', original: 'Hello.', translated: 'こんにちは。' });
    expect(v.lines).toEqual([{ id: 1, original: 'Hello.', translated: 'こんにちは。' }]);
  });

  it('字幕は時系列で蓄積、上限超過分は旧字幕から破棄', () => {
    const events: SubtitleEvent[] = Array.from({ length: MAX_LINES + 5 }, (_, i) => ({
      kind: 'chunk',
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
    v = reduce(v, { kind: 'chunk', original: 'a', translated: 'b' });
    expect(v.error).toBeNull();
  });

  it('連続失敗による停止後もエラー表示を継続', () => {
    const v = run([
      { kind: 'started', device: '' },
      { kind: 'chunk', original: 'a', translated: 'b' },
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
      { kind: 'chunk', original: 'a', translated: 'b' },
      { kind: 'error', message: 'x' },
      { kind: 'stopped' },
      { kind: 'started', device: 'd' }
    ]);
    expect(v).toEqual({ ...initialView(), running: true, device: 'd' });
  });
});
