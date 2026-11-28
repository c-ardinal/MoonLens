import { describe, expect, it } from 'vitest';
import { IDLE, type FileSubtitleCue, type FileSubtitleStatus } from './file-subtitle';
import { RESUME_AHEAD_MS, WAIT_LEAD_MS, cueIndexAt, cueLines, isAhead, mayResume, mustWait } from './media-player';

const cue = (start: number, end: number, translated = `訳${start}`): FileSubtitleCue => ({
  start_ms: start,
  end_ms: end,
  original: `src${start}`,
  translated
});

const status = (over: Partial<FileSubtitleStatus>): FileSubtitleStatus => ({
  ...IDLE,
  phase: 'running',
  ...over
});

describe('cueIndexAt', () => {
  const cues = [cue(1000, 3000), cue(3000, 5000), cue(8000, 9000)];

  it('finds the subtitle under the position', () => {
    expect(cueIndexAt(cues, 1000)).toBe(0);
    expect(cueIndexAt(cues, 2999)).toBe(0);
    expect(cueIndexAt(cues, 3000)).toBe(1);
    expect(cueIndexAt(cues, 8500)).toBe(2);
  });

  it('finds none before, between and after the subtitles', () => {
    expect(cueIndexAt(cues, 0)).toBe(-1);
    expect(cueIndexAt(cues, 6000)).toBe(-1);
    expect(cueIndexAt(cues, 9000)).toBe(-1);
    expect(cueIndexAt([], 1000)).toBe(-1);
  });

  it('stays fast on a long file', () => {
    const many = Array.from({ length: 10_000 }, (_, n) => cue(n * 2000, n * 2000 + 1500));
    expect(cueIndexAt(many, 2000 * 7777 + 10)).toBe(7777);
    expect(cueIndexAt(many, 2000 * 7777 + 1600)).toBe(-1);
  });
});

describe('cueLines', () => {
  it('shows what was chosen', () => {
    expect(cueLines(cue(0, 1), 'translated')).toEqual(['訳0']);
    expect(cueLines(cue(0, 1), 'original')).toEqual(['src0']);
    expect(cueLines(cue(0, 1), 'both')).toEqual(['訳0', 'src0']);
  });

  it('falls back to the original when the translation failed', () => {
    expect(cueLines(cue(0, 1, ''), 'translated')).toEqual(['src0']);
    expect(cueLines(cue(0, 1, ''), 'both')).toEqual(['src0']);
  });
});

describe('waiting for subtitles', () => {
  it('waits shortly before the position the generation reached', () => {
    const s = status({ done_ms: 60_000 });
    expect(mustWait(s, 60_000 - WAIT_LEAD_MS - 1)).toBe(false);
    expect(mustWait(s, 60_000 - WAIT_LEAD_MS + 1)).toBe(true);
  });

  it('never waits once the generation ended', () => {
    for (const phase of ['done', 'cancelled', 'failed'] as const) {
      expect(mustWait(status({ phase, done_ms: 10_000 }), 50_000)).toBe(false);
      expect(mayResume(status({ phase, done_ms: 10_000 }), 50_000)).toBe(true);
    }
  });

  it('resumes once the generation is well ahead', () => {
    expect(mayResume(status({ done_ms: 60_000 }), 59_000)).toBe(false);
    expect(mayResume(status({ done_ms: 59_000 + RESUME_AHEAD_MS }), 59_000)).toBe(true);
  });

  it('a resumed playback does not wait again at once', () => {
    const pos = 59_000;
    expect(mustWait(status({ done_ms: pos + RESUME_AHEAD_MS }), pos)).toBe(false);
  });

  it('tells a position past the generation', () => {
    expect(isAhead(status({ done_ms: 10_000 }), 12_000)).toBe(true);
    expect(isAhead(status({ done_ms: 10_000 }), 8_000)).toBe(false);
    expect(isAhead(status({ phase: 'done', done_ms: 10_000 }), 12_000)).toBe(false);
  });
});
