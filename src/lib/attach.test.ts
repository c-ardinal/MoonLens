import { describe, expect, it } from 'vitest';
import { isUnder, targetLines, targetName, type AttachCandidate } from './attach';

const target = (app: string, title: string) => ({ id: 7, pid: 1, app, title });
const candidate = (overlap: number): AttachCandidate => ({
  ...target('game', 'Game'),
  rect: { x: 0, y: 0, w: 800, h: 600 },
  overlap
});

describe('attach target names', () => {
  it('names a target after its app, without the file extension', () => {
    expect(targetName(target('Game.exe', 'Chapter 1'))).toBe('Game');
    expect(targetName(target('firefox', 'Docs'))).toBe('firefox');
  });

  it('falls back to the window title, then to the id', () => {
    expect(targetName(target('', 'Chapter 1'))).toBe('Chapter 1');
    expect(targetName(target(' ', ' '))).toBe('ウィンドウ 7');
  });

  it('shows the title beside the app only when it adds something', () => {
    expect(targetLines(target('Game.exe', 'Chapter 1'))).toEqual({ name: 'Game', detail: 'Chapter 1' });
    expect(targetLines(target('Game.exe', 'Game')).detail).toBe('');
    expect(targetLines(target('', 'Chapter 1')).detail).toBe('');
  });
});

describe('the window under the follower', () => {
  it('is the first candidate when most of the follower lies on it', () => {
    expect(isUnder([candidate(0.8), candidate(0.2)], 0)).toBe(true);
    expect(isUnder([candidate(0.8), candidate(0.6)], 1)).toBe(false);
    expect(isUnder([candidate(0.3)], 0)).toBe(false);
    expect(isUnder([], 0)).toBe(false);
  });
});
