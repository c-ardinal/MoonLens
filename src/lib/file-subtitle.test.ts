import { describe, expect, it } from 'vitest';
import {
  IDLE,
  positionLabel,
  progressRatio,
  applyUpdate,
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

describe('applyUpdate', () => {
  const cue = (n: number): FileSubtitleCue => ({ start_ms: n * 1000, end_ms: n * 1000 + 900, original: `${n}`, translated: '' });
  const update = (cues: number, added: FileSubtitleCue | null) => ({ status: running({ cues }), cue: added });

  it('appends the subtitle the update carries', () => {
    const next = applyUpdate([cue(0)], update(2, cue(1)));
    expect(next).toEqual({ cues: [cue(0), cue(1)], reload: false });
  });

  it('keeps the subtitles on a progress update', () => {
    const cues = [cue(0)];
    expect(applyUpdate(cues, update(1, null))).toEqual({ cues, reload: false });
    expect(applyUpdate(cues, update(1, null)).cues).toBe(cues);
  });

  it('asks for all subtitles when some were missed', () => {
    expect(applyUpdate([], update(3, cue(2))).reload).toBe(true);
    expect(applyUpdate([cue(0)], update(3, null)).reload).toBe(true);
  });

  it('asks for all subtitles when a new file started', () => {
    expect(applyUpdate([cue(0), cue(1)], update(0, null)).reload).toBe(true);
  });
});
