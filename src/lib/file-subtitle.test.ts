import { describe, expect, it } from 'vitest';
import {
  IDLE,
  RECENT_CUES,
  positionLabel,
  progressRatio,
  withCue,
  type FileSubtitleCue,
  type FileSubtitleStatus
} from './file-subtitle';

const running = (over: Partial<FileSubtitleStatus>): FileSubtitleStatus => ({
  ...IDLE,
  phase: 'running',
  ...over
});

describe('progressRatio', () => {
  it('is the share of the audio that is done', () => {
    expect(progressRatio(running({ duration_ms: 60_000, done_ms: 15_000 }))).toBe(0.25);
  });

  it('is unknown without a duration', () => {
    expect(progressRatio(running({ duration_ms: null, done_ms: 15_000 }))).toBeNull();
  });

  it('never passes 1 when the track is longer than the container says', () => {
    expect(progressRatio(running({ duration_ms: 60_000, done_ms: 61_000 }))).toBe(1);
  });

  it('is 1 once done, even without a duration', () => {
    expect(progressRatio({ ...IDLE, phase: 'done' })).toBe(1);
  });
});

describe('positionLabel', () => {
  it('shows the position and the length', () => {
    expect(positionLabel(running({ duration_ms: 3_723_000, done_ms: 754_000 }))).toBe('12:34 / 1:02:03');
  });

  it('shows the position alone without a duration', () => {
    expect(positionLabel(running({ done_ms: 754_000 }))).toBe('12:34');
  });
});

describe('withCue', () => {
  const cue = (n: number): FileSubtitleCue => ({ start_ms: n, end_ms: n + 1, original: `${n}`, translated: '' });

  it('keeps only the latest cues', () => {
    let recent: FileSubtitleCue[] = [];
    for (let n = 0; n < RECENT_CUES + 5; n++) recent = withCue(recent, cue(n));
    expect(recent).toHaveLength(RECENT_CUES);
    expect(recent[0].original).toBe('5');
    expect(recent.at(-1)!.original).toBe(`${RECENT_CUES + 4}`);
  });
});
