import { describe, expect, it } from 'vitest';
import {
  MAX_LINES,
  clear,
  conversationPair,
  partnerLanguages,
  sameSource,
  sourceKey,
  initialView,
  inputHint,
  reduce,
  restore,
  settle,
  sourceLabel,
  modeSwitchNotice,
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

describe('対面会話（音源：マイク）', () => {
  const mic = { kind: 'microphone' } as const;

  it('音源の表記・照合・選択状態', () => {
    const view = { source: mic, device: 'USB Microphone' };
    expect(sourceLabel(view)).toBe('マイク');
    expect(sameSource(mic, { kind: 'microphone' })).toBe(true);
    expect(sameSource(mic, { kind: 'system' })).toBe(false);
    expect(sameSource({ kind: 'app', pid: 1 }, mic)).toBe(false);
    const options = sourceOptions([], view);
    expect(options.map((o) => o.selected)).toEqual([false, true]);
    expect(new Set(options.map((o) => sourceKey(o.source))).size).toBe(options.length);
  });

  it('音源メニュー：対面会話の行は画面切替を明記', () => {
    const [, option] = sourceOptions([], { source: { kind: 'system' }, device: '' });
    expect(option.source).toEqual(mic);
    expect(option.label).toBe('対面会話');
    expect(option.detail).toContain('会話画面へ切替');
  });

  it('画面切替の案内：対面会話と字幕表示の間の変更のみ', () => {
    const system = { kind: 'system' } as const;
    const app = { kind: 'app', pid: 7 } as const;
    expect(modeSwitchNotice(system, mic)).toContain('対面会話へ切替');
    expect(modeSwitchNotice(app, mic)).toContain('対面会話へ切替');
    expect(modeSwitchNotice(mic, system)).toContain('字幕表示へ切替');
    expect(modeSwitchNotice(system, app)).toBeNull();
    expect(modeSwitchNotice(mic, mic)).toBeNull();
  });

  it('発話の話者を行へ保持、文単位の補正後も継承', () => {
    const v = run([
      { kind: 'started', device: 'mic', source: mic },
      { kind: 'chunk', id: 1, original: '駅は', translated: 'The station', speaker: 'own' },
      { kind: 'chunk', id: 2, original: 'どこですか。', translated: 'where is it?', speaker: 'own' },
      { kind: 'chunk', id: 3, original: 'Over there.', translated: 'あちらです。', speaker: 'partner' },
      {
        kind: 'revised',
        replaces: [1, 2],
        id: 4,
        original: '駅はどこですか。',
        translated: 'Where is the station?',
        rest: null
      }
    ]);
    expect(v.lines.map((l) => [l.original, l.speaker])).toEqual([
      ['駅はどこですか。', 'own'],
      ['Over there.', 'partner']
    ]);
    expect(v.lines[0].revised).toBe(true);
  });

  it('字幕（話者なし）の行は話者を保持しない', () => {
    const v = run([{ kind: 'chunk', id: 1, original: 'a', translated: 'b' }]);
    expect('speaker' in v.lines[0]).toBe(false);
  });

  it('言語対：相手の言語が本人と同一・未設定の場合は英語（本人が英語の場合は日本語）', () => {
    expect(conversationPair('ja', 'en')).toEqual({ own: 'ja', partner: 'en' });
    expect(conversationPair('ja', 'ja')).toEqual({ own: 'ja', partner: 'en' });
    expect(conversationPair('ja-easy', 'ja')).toEqual({ own: 'ja-easy', partner: 'en' });
    expect(conversationPair('en', 'en')).toEqual({ own: 'en', partner: 'ja' });
    expect(conversationPair('ko', '')).toEqual({ own: 'ko', partner: 'en' });
    expect(conversationPair('ko', 'zh-Hans')).toEqual({ own: 'ko', partner: 'zh-Hans' });
  });

  it('相手の言語の選択肢：本人と同一の言語は対象外', () => {
    const all = [{ code: 'ja' }, { code: 'en' }, { code: 'zh-Hans' }, { code: 'zh-Hant' }];
    expect(partnerLanguages(all, 'ja-easy').map((l) => l.code)).toEqual(['en', 'zh-Hans', 'zh-Hant']);
    expect(partnerLanguages(all, 'zh-Hans').map((l) => l.code)).toEqual(['ja', 'en']);
  });

  it('案内文：入力なしはマイクの確認、入力ありは発話待機', () => {
    expect(inputHint('none', null, true)).toContain('マイク');
    expect(inputHint('ok', null, true)).toBe(inputHint('low', null, true));
    expect(inputHint('ok', null, true)).not.toBe(inputHint('ok'));
  });
});

describe('音源メニューの選択肢', () => {
  const apps = [
    { pid: 10, name: 'chrome', title: '動画 - Google Chrome', playing: true },
    { pid: 20, name: 'vlc', title: null, playing: false }
  ];

  it('先頭は PC 全体、以降はアプリ一覧。選択中の音源を明示', () => {
    const options = sourceOptions(apps, { source: { kind: 'app', pid: 20 }, device: 'vlc' });
    expect(options.map((o) => o.label)).toEqual(['PC 全体', '対面会話', 'chrome', 'vlc']);
    expect(options.map((o) => o.selected)).toEqual([false, false, false, true]);
    // 対面会話（マイク）：再生状況の対象外
    expect(options.map((o) => o.playing)).toEqual([true, null, true, false]);
    expect(options[2].detail).toBe('動画 - Google Chrome');
  });

  it('選択中のアプリが一覧に不在でも選択肢として残置', () => {
    const options = sourceOptions(apps, { source: { kind: 'app', pid: 99 }, device: 'game' });
    expect(options.at(-1)).toMatchObject({ label: 'game', selected: true, playing: false });
    expect(sourceOptions([], { source: { kind: 'system' }, device: '' })).toHaveLength(2);
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

describe('遮断候補（反復検出）', () => {
  let next = 0;
  const heard = (original: string): SubtitleEvent => ({
    kind: 'chunk',
    id: ++next,
    original,
    translated: `訳：${original}`
  });
  const started: SubtitleEvent = { kind: 'started', device: 'スピーカー', source: { kind: 'system' } };

  it('候補は検出順に保持、同一文の重複なし、開始時に初期化', () => {
    let v = run([
      started,
      { kind: 'candidate', phrase: 'Thank you.' },
      { kind: 'candidate', phrase: 'Bye.' },
      { kind: 'candidate', phrase: 'Thank you.' }
    ]);
    expect(v.candidates).toEqual(['Thank you.', 'Bye.']);
    v = reduce(v, started);
    expect(v.candidates).toEqual([]);
  });

  it('遮断：候補と表示済みの同一文（表記差・反復を含む）を除去', () => {
    const v = run([
      heard('Thank you.'),
      heard('Thank you for coming.'),
      heard('thank you'),
      heard('Thank you. Thank you.'),
      { kind: 'candidate', phrase: 'Thank you.' }
    ]);
    const next = settle(v, 'Thank you.', true);
    expect(next.candidates).toEqual([]);
    expect(next.lines.map((l) => l.original)).toEqual(['Thank you for coming.']);
  });

  it('遮断済みの文の補正結果（原文が空）：置換対象の行を除去、次文の冒頭のみ残置', () => {
    const v = run([heard('Thank you.'), heard('Please sit.')]);
    const [first, second] = v.lines.map((l) => l.id);
    const next = reduce(settle(v, 'Thank you.', true), {
      kind: 'revised',
      replaces: [first, second],
      id: second + 1,
      original: '',
      translated: '',
      rest: { id: second + 2, original: 'Please sit.', translated: '' }
    });
    expect(next.lines.map((l) => l.original)).toEqual(['Please sit.']);
  });

  it('無視：候補のみ除去、字幕は維持', () => {
    const v = run([heard('Thank you.'), { kind: 'candidate', phrase: 'Thank you.' }]);
    const next = settle(v, 'Thank you.', false);
    expect(next.candidates).toEqual([]);
    expect(next.lines).toHaveLength(1);
  });
});

describe('字幕のクリア', () => {
  const started: SubtitleEvent = { kind: 'started', device: 'スピーカー', source: { kind: 'system' } };

  it('表示中の字幕のみ消去し、動作状態・エラーは維持', () => {
    let v = run([
      started,
      { kind: 'chunk', id: 1, original: 'Hello.', translated: 'こんにちは。' },
      { kind: 'status', phase: 'recognizing' }
    ]);
    v = clear(v);
    expect(v.lines).toEqual([]);
    expect(v.running).toBe(true);
    expect(v.phase).toBe('recognizing');
    // 空の状態：同一オブジェクトを返却（再描画なし）。
    expect(clear(v)).toBe(v);
  });

  it('消去済みの行への補正は無視、消去後の字幕は継続表示', () => {
    let v = run([
      started,
      { kind: 'chunk', id: 1, original: 'Where is', translated: 'どこ' },
      { kind: 'chunk', id: 2, original: 'the station?', translated: '駅は' }
    ]);
    v = clear(v);
    v = run(
      [
        { kind: 'revised', replaces: [1, 2], id: 3, original: 'Where is the station?', translated: '駅はどこですか。', rest: null },
        { kind: 'chunk', id: 4, original: 'Thanks.', translated: 'ありがとう。' }
      ],
      v
    );
    expect(v.lines).toEqual([{ id: 4, original: 'Thanks.', translated: 'ありがとう。' }]);
  });
});
