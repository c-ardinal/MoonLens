import { describe, expect, it } from 'vitest';
import type { LensBlock } from './ipc';
import {
  OBSERVE_MAX_MS,
  OBSERVE_MIN_MS,
  SAMPLES_WANTED,
  addSample,
  observed,
  remainingSeconds,
  subtitleBand,
  type Sample
} from './lens-fit';

const block = (x: number, y: number, w: number, h: number, source: string, line = 40): LensBlock => ({
  bbox: [x, y, w, h],
  line_height: line,
  source,
  translation: null,
  colors: null
});

const AREA = { w: 1920, h: 1080 };
const HUD = block(40, 30, 300, 40, 'SCORE 000120');

function collect(readings: LensBlock[][]): Sample[] {
  return readings.reduce<Sample[]>((s, r) => addSample(s, r), []);
}

describe('addSample', () => {
  it('skips empty readings and repeats of the last reading', () => {
    const a = [block(0, 0, 100, 40, 'Hello')];
    let s = addSample([], a);
    s = addSample(s, [{ ...a[0], translation: 'こんにちは' }]);
    s = addSample(s, []);
    s = addSample(s, [block(0, 0, 100, 40, '   ')]);
    expect(s).toHaveLength(1);
    s = addSample(s, [block(0, 0, 120, 40, 'Goodbye')]);
    expect(s).toHaveLength(2);
  });
});

describe('observed', () => {
  it('ends at the minimum time once enough readings arrived, else at the maximum', () => {
    expect(observed(OBSERVE_MIN_MS - 1, SAMPLES_WANTED)).toBe(false);
    expect(observed(OBSERVE_MIN_MS, SAMPLES_WANTED)).toBe(true);
    expect(observed(OBSERVE_MIN_MS, SAMPLES_WANTED - 1)).toBe(false);
    expect(observed(OBSERVE_MAX_MS, 0)).toBe(true);
  });

  it('counts down to the limit that applies', () => {
    expect(remainingSeconds(5_000, 0)).toBe(40);
    expect(remainingSeconds(5_000, SAMPLES_WANTED)).toBe(10);
    expect(remainingSeconds(OBSERVE_MAX_MS + 1, 0)).toBe(0);
  });
});

describe('subtitleBand', () => {
  it('reports no text when nothing was read', () => {
    expect(subtitleBand([], AREA)).toEqual({ kind: 'none' });
  });

  it('fits the band where changing lines appear and leaves the fixed HUD out', () => {
    const samples = collect([
      [HUD, block(600, 940, 720, 44, 'Where are you going tonight?')],
      [HUD, block(700, 940, 520, 44, 'I do not know yet.')],
      [HUD, block(560, 890, 800, 94, 'Then come with us.\nWe leave at dawn.')],
      [HUD, block(760, 940, 400, 44, 'All right.')]
    ]);
    const band = subtitleBand(samples, AREA);
    expect(band.kind).toBe('fit');
    if (band.kind !== 'fit') return;
    const [x, y, w, h] = band.rect;
    // Every subtitle line is inside, with room around it.
    expect(x).toBeLessThan(560);
    expect(x + w).toBeGreaterThan(1360);
    expect(y).toBeLessThanOrEqual(890 - 20);
    expect(y + h).toBeGreaterThanOrEqual(984 + 20);
    // The HUD at the top is not.
    expect(y).toBeGreaterThan(70);
    expect(h).toBeLessThan(200);
  });

  it('treats a fixed block as fixed despite small recognition differences', () => {
    const samples = collect([
      [HUD, block(600, 940, 720, 44, 'First line of the film')],
      [block(41, 30, 299, 40, 'SCORE 0O0120'), block(650, 940, 600, 44, 'Second line here')],
      [HUD, block(700, 940, 500, 44, 'Third one')]
    ]);
    const band = subtitleBand(samples, AREA);
    expect(band.kind === 'fit' && band.rect[1]).toBeGreaterThan(800);
  });

  it('prefers the band with more text over a ticking clock', () => {
    const clock = (t: string) => block(1700, 20, 120, 30, t, 30);
    const samples = collect([
      [clock('12:00:01'), block(600, 940, 720, 44, 'Where are you going tonight?')],
      [clock('12:00:02'), block(600, 940, 720, 44, 'Where are you going tonight?')],
      [clock('12:00:03'), block(700, 940, 520, 44, 'I do not know yet.')],
      [clock('12:00:04'), block(700, 940, 520, 44, 'I do not know yet.')]
    ]);
    const band = subtitleBand(samples, AREA);
    expect(band.kind === 'fit' && band.rect[1]).toBeGreaterThan(800);
  });

  it('keeps a fixed name plate next to the band', () => {
    const name = block(420, 880, 160, 40, 'Alice');
    const samples = collect([
      [HUD, name, block(420, 930, 900, 44, 'I have been waiting for you.')],
      [HUD, name, block(420, 930, 700, 44, 'Shall we begin?')],
      [HUD, name, block(420, 930, 500, 44, 'Follow me.')]
    ]);
    const band = subtitleBand(samples, AREA);
    expect(band.kind).toBe('fit');
    if (band.kind !== 'fit') return;
    expect(band.rect[1]).toBeLessThanOrEqual(880);
    expect(band.rect[1]).toBeGreaterThan(70);
  });

  it('fits to all text when nothing changed during the observation', () => {
    const samples = collect([[block(200, 300, 600, 200, 'A fixed notice\nwith two lines')]]);
    const band = subtitleBand(samples, AREA);
    expect(band.kind).toBe('fit');
    if (band.kind !== 'fit') return;
    expect(band.rect[1]).toBe(280);
    expect(band.rect[3]).toBe(240);
  });

  it('stays inside the observed range', () => {
    const samples = collect([
      [block(0, 1040, 1920, 40, 'A line as wide as the whole range')],
      [block(10, 1040, 1900, 40, 'Another line just as wide as that')]
    ]);
    const band = subtitleBand(samples, AREA);
    expect(band).toEqual({ kind: 'fit', rect: [0, 1020, 1920, 60] });
  });

  it('reports that no fitting is needed when text fills the range', () => {
    const samples = collect([[block(10, 10, 440, 280, 'Text all over the range')]]);
    expect(subtitleBand(samples, { w: 460, h: 300 })).toEqual({ kind: 'fits' });
  });
});
