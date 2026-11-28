import { describe, expect, it } from 'vitest';
import type { LensBlock } from './ipc';
import { FORGET_MS, skipNote, unspoken, type Spoken } from './speech';

const block = (translation: string | null, extra: Partial<LensBlock> = {}): LensBlock => ({
  bbox: [0, 0, 10, 10],
  line_height: 10,
  source: 'source',
  translation,
  colors: null,
  ...extra
});

describe('unspoken', () => {
  it('表示順で未読上の訳文のみ返却', () => {
    const spoken: Spoken = new Map();
    expect(unspoken(spoken, [block('一行目'), block('二行目')], 0)).toEqual(['一行目', '二行目']);
    expect(unspoken(spoken, [block('一行目'), block('二行目'), block('三行目')], 250)).toEqual(['三行目']);
  });

  it('翻訳待ち・翻訳不要・発音対象なしのブロックは対象外', () => {
    const spoken: Spoken = new Map();
    const blocks = [block(null), block('原文と同一', { same_language: true }), block(' … '), block('訳文')];
    expect(unspoken(spoken, blocks, 0)).toEqual(['訳文']);
  });

  it('表記の揺らぎ（空白・約物・大小文字）は同一の文として再読上せず', () => {
    const spoken: Spoken = new Map();
    unspoken(spoken, [block('Hello, world!')], 0);
    expect(unspoken(spoken, [block('hello world')], 250)).toEqual([]);
  });

  it('短時間の消失後の再表示は再読上せず', () => {
    const spoken: Spoken = new Map();
    unspoken(spoken, [block('台詞')], 0);
    unspoken(spoken, [], 1000);
    expect(unspoken(spoken, [block('台詞')], 2000)).toEqual([]);
  });

  it('消失から記憶期間の経過後の再表示は再読上', () => {
    const spoken: Spoken = new Map();
    unspoken(spoken, [block('台詞')], 0);
    unspoken(spoken, [], 1000);
    expect(unspoken(spoken, [block('台詞')], 1000 + FORGET_MS + 1)).toEqual(['台詞']);
  });

  it('読取結果の通知が長時間途絶しても、表示の継続中は再読上せず', () => {
    const spoken: Spoken = new Map();
    unspoken(spoken, [block('常設の見出し')], 0);
    expect(unspoken(spoken, [block('常設の見出し'), block('新規')], FORGET_MS * 5)).toEqual(['新規']);
  });

  it('同一文の複数ブロックは 1 回のみ', () => {
    expect(unspoken(new Map(), [block('はい'), block('はい')], 0)).toEqual(['はい']);
  });

  it('消失済みの記憶は期間経過後に破棄', () => {
    const spoken: Spoken = new Map();
    unspoken(spoken, [block('台詞')], 0);
    unspoken(spoken, [], 1000);
    unspoken(spoken, [], 1000 + FORGET_MS + 1);
    expect(spoken.size).toBe(0);
  });

  it('表示中の訳文は期間を超過しても記憶を維持', () => {
    const spoken: Spoken = new Map();
    for (let now = 0; now <= FORGET_MS * 3; now += 1000) {
      expect(unspoken(spoken, [block('常設の見出し')], now)).toEqual(now === 0 ? ['常設の見出し'] : []);
    }
    expect(spoken.size).toBe(1);
  });
});

describe('skipNote', () => {
  it('理由別の案内文', () => {
    expect(skipNote({ reason: 'blocked', lang: 'ja' })).toContain('PC 全体');
    expect(skipNote({ reason: 'no_voice', lang: 'ko' })).toContain('韓国語');
    expect(skipNote({ reason: 'failed', lang: 'ja' })).not.toBe('');
  });
});
