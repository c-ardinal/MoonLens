import { beforeEach, describe, expect, it, vi } from 'vitest';
import { loadPlacement, parsePlacement, savePlacement, textWindowLabel } from './lens-text';

describe('text placement', () => {
  beforeEach(() => {
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v)
    });
  });

  it('falls back to inside for unknown values', () => {
    expect(parsePlacement('bottom')).toBe('bottom');
    expect(parsePlacement('diagonal')).toBe('inside');
    expect(parsePlacement(null)).toBe('inside');
  });

  it('remembers the last choice for new windows', () => {
    expect(loadPlacement()).toBe('inside');
    savePlacement('free');
    expect(loadPlacement()).toBe('free');
  });

  it('labels the outside window after its lens', () => {
    expect(textWindowLabel('lens-18f-0')).toBe('lens-18f-0-text');
  });
});
