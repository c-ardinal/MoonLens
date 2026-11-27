import { describe, expect, it } from 'vitest';
import {
  MAX_LINES,
  initialView,
  inputHint,
  reduce,
  restore,
  sourceLabel,
  sourceOptions,
  statusLabel,
  type SubtitleView
} from './subtitle';
import type { SubtitleEvent } from './ipc';

const run = (events: SubtitleEvent[], from: SubtitleView = initialView()) =>
  events.reduce(reduce, from);

describe('音声翻訳ウィンドウの表示状態', () => {
  it('開始から字幕表示までの状態表記', () => {
    let v = initialView();
    expect(statusLabel(v)).toBe('停止中');
    v = reduce(v, { kind: 'started', device: 'スピーカー', source: { kind: 'system' } });
    expect(v.device).toBe('スピーカー');
    expect(statusLabel(v)).toBe('入力未検出');
    expect(statusLabel(v, 'low')).toBe('閾値未満');
    expect(statusLabel(v, 'ok')).toBe('音声検出');
    v = reduce(v, { kind: 'status', phase: 'recognizing' });
    expect(statusLabel(v, 'ok')).toBe('認識中…');
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
      { kind: 'started', device: '', source: { kind: 'system' } },
      { kind: 'error', message: '音声認識失敗：x' }
    ]);
    expect(v.error).toBe('音声認識失敗：x');
    expect(statusLabel(v)).toBe('入力未検出');
    v = reduce(v, { kind: 'chunk', id: 1, original: 'a', translated: 'b' });
    expect(v.error).toBeNull();
  });

  it('連続失敗による停止後もエラー表示を継続', () => {
    const v = run([
      { kind: 'started', device: '', source: { kind: 'system' } },
      { kind: 'chunk', id: 1, original: 'a', translated: 'b' },
      { kind: 'error', message: '音声認識の連続失敗により停止：x' },
      { kind: 'stopped' }
    ]);
    expect(v.running).toBe(false);
    expect(v.error).toContain('停止');
    expect(v.lines).toHaveLength(1);
    expect(statusLabel(v)).toBe('エラー停止');
  });

  it('案内文は入力状態単位で相違、停止中は入力状態を不問', () => {
    expect(new Set((['none', 'low', 'ok'] as const).map((i) => inputHint(i))).size).toBe(3);
    expect(inputHint('none', 'vlc')).toContain('vlc');
    expect(statusLabel(initialView(), 'ok')).toBe('停止中');
  });

  it('音源変更・再開：字幕は保持、エラーは消去', () => {
    const v = run([
      { kind: 'chunk', id: 1, original: 'a', translated: 'b' },
      { kind: 'error', message: 'x' },
      { kind: 'stopped' },
      { kind: 'started', device: 'vlc', source: { kind: 'app', pid: 7 } }
    ]);
    expect(v.running).toBe(true);
    expect(v.error).toBeNull();
    expect(v.lines).toHaveLength(1);
    expect(v.source).toEqual({ kind: 'app', pid: 7 });
    expect(sourceLabel(v)).toBe('vlc');
  });

  it('再読込時：照会した現況を反映', () => {
    const v = restore(initialView(), { source: { kind: 'app', pid: 7 }, device: 'vlc', running: true });
    expect(v.running).toBe(true);
    expect(sourceLabel(v)).toBe('vlc');
    expect(restore(v, null)).toBe(v);
    expect(sourceLabel(initialView())).toBe('PC 全体');
  });
});

describe('音源メニューの選択肢', () => {
  const apps = [
    { pid: 10, name: 'chrome', title: '動画 - Google Chrome', playing: true },
    { pid: 20, name: 'vlc', title: null, playing: false }
  ];

  it('先頭は PC 全体、以降はアプリ一覧。選択中の音源を明示', () => {
    const options = sourceOptions(apps, { source: { kind: 'app', pid: 20 }, device: 'vlc' });
    expect(options.map((o) => o.label)).toEqual(['PC 全体', 'chrome', 'vlc']);
    expect(options.map((o) => o.selected)).toEqual([false, false, true]);
    expect(options.map((o) => o.playing)).toEqual([true, true, false]);
    expect(options[1].detail).toBe('動画 - Google Chrome');
  });

  it('選択中のアプリが一覧に不在でも選択肢として残置', () => {
    const options = sourceOptions(apps, { source: { kind: 'app', pid: 99 }, device: 'game' });
    expect(options.at(-1)).toMatchObject({ label: 'game', selected: true, playing: false });
    expect(sourceOptions([], { source: { kind: 'system' }, device: '' })).toHaveLength(1);
  });

  it('文単位の再翻訳結果で該当行を置換', () => {
    const chunk = (id: number, original: string): SubtitleEvent => ({
      kind: 'chunk',
      id,
      original,
      translated: `<${original}>`
    });
    let v = run([
      { kind: 'started', device: '', source: { kind: 'system' } },
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
    expect(same.lines).toBe(v.lines);
  });

  it('音源変更・再開後も行 ID は一意、補正は新規の行のみ置換', () => {
    const started: SubtitleEvent = { kind: 'started', device: '', source: { kind: 'system' } };
    let v = run([
      started,
      { kind: 'chunk', id: 1, original: 'old', translated: '旧' },
      // 再開：バックエンドの採番は 1 から。
      started,
      { kind: 'chunk', id: 1, original: 'new a', translated: '新' },
      { kind: 'chunk', id: 2, original: 'new b.', translated: '規' }
    ]);
    expect(new Set(v.lines.map((l) => l.id)).size).toBe(3);
    v = reduce(v, {
      kind: 'revised',
      replaces: [1, 2],
      id: 3,
      original: 'new a new b.',
      translated: '新規。',
      rest: null
    });
    expect(v.lines.map((l) => l.original)).toEqual(['old', 'new a new b.']);
    expect(new Set(v.lines.map((l) => l.id)).size).toBe(2);
  });
});
