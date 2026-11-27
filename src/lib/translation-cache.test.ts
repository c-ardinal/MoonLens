import { describe, expect, it } from 'vitest';
import { cacheSize, scopeLabel } from './translation-cache';

describe('cacheSize', () => {
  it('shows kilobytes below one megabyte and megabytes above', () => {
    expect(cacheSize(0)).toBe('0 KB');
    expect(cacheSize(300)).toBe('1 KB');
    expect(cacheSize(40_960)).toBe('40 KB');
    expect(cacheSize(3 * 1_048_576 + 524_288)).toBe('3.5 MB');
  });
});

describe('scopeLabel', () => {
  it('names an app and the shared entries', () => {
    expect(scopeLabel('game')).toBe('アプリ「game」');
    expect(scopeLabel('')).toBe('共通（追従先アプリ未指定）');
  });
});
