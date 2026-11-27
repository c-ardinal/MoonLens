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
      { kind: 'started', device: '', source: { kind: 'system' } },
      { kind: 'error', message: '音声認識失敗：x' }
    ]);
    expect(v.error).toBe('音声認識失敗：x');
    expect(statusLabel(v)).toBe('入力未検出');
    v = reduce(v, { kind: 'chunk', original: 'a', translated: 'b' });
    expect(v.error).toBeNull();
  });

  it('連続失敗による停止後もエラー表示を継続', () => {
    const v = run([
      { kind: 'started', device: '', source: { kind: 'system' } },
      { kind: 'chunk', original: 'a', translated: 'b' },
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
      { kind: 'chunk', original: 'a', translated: 'b' },
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
});
