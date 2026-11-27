import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NOTE_MS, Speaker } from './speech.svelte';

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
});
