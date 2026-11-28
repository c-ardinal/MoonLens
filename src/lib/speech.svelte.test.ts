import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { speak } from './ipc';
import { NOTE_MS, Speaker } from './speech.svelte';

vi.mock('./ipc', () => ({
  onSpeech: vi.fn(),
  speak: vi.fn(() => Promise.resolve()),
  stopSpeaking: vi.fn(() => Promise.resolve())
}));

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('Speaker', () => {
  it('開始・終了で読上中の表示を切替', () => {
    const s = new Speaker();
    s.receive({ kind: 'started' });
    expect(s.speaking).toBe(true);
    s.receive({ kind: 'ended' });
    expect(s.speaking).toBe(false);
  });

  it('見送り理由の案内は一定時間で消去', () => {
    const s = new Speaker();
    s.receive({ kind: 'skipped', reason: 'no_voice', lang: 'ko' });
    expect(s.note).toContain('韓国語');
    vi.advanceTimersByTime(NOTE_MS);
    expect(s.note).toBeNull();
  });

  it('読上の開始で案内を消去', () => {
    const s = new Speaker();
    s.receive({ kind: 'skipped', reason: 'blocked', lang: 'ja' });
    s.receive({ kind: 'started' });
    expect(s.note).toBeNull();
    vi.advanceTimersByTime(NOTE_MS);
    expect(s.speaking).toBe(true);
  });

  it('依頼から開始までは待機中、開始で読上中', () => {
    const s = new Speaker();
    s.say('訳文', 'ja');
    expect(s.busy).toBe(true);
    expect(s.speaking).toBe(false);
    s.receive({ kind: 'started' });
    expect(s.waiting).toBe(false);
    expect(s.busy).toBe(true);
  });

  it('待機中の停止・破棄・見送りで待機を解除', () => {
    for (const end of [
      (s: Speaker) => s.stop(),
      (s: Speaker) => s.receive({ kind: 'dropped' }),
      (s: Speaker) => s.receive({ kind: 'skipped', reason: 'failed', lang: 'ja' })
    ]) {
      const s = new Speaker();
      s.say('訳文', 'ja');
      end(s);
      expect(s.busy).toBe(false);
    }
  });

  it('空の文面は依頼なし', () => {
    const s = new Speaker();
    s.say('  ', 'ja');
    expect(s.busy).toBe(false);
  });

  it('元の発話の長さを読上の依頼へ添付（話速への追随）', () => {
    new Speaker().say('駅はどこですか', 'ja', 2400);
    expect(speak).toHaveBeenLastCalledWith('駅はどこですか', 'ja', 2400);
  });
});
