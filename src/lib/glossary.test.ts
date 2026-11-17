import { describe, expect, it } from 'vitest';
import { addEntry, compactGlossary, removeEntry, updateEntry } from './glossary';

describe('glossary helpers', () => {
  it('adds, edits and removes rows without mutating the input', () => {
    const a = addEntry([], 'ja');
    expect(a).toEqual([{ source: '', target: '', lang: 'ja' }]);
    const b = updateEntry(a, 0, { source: 'Dashboard', target: '管理画面' });
    expect(b[0]).toEqual({ source: 'Dashboard', target: '管理画面', lang: 'ja' });
    expect(a[0].source).toBe('');
    expect(removeEntry(b, 0)).toEqual([]);
  });

  it('drops rows left blank', () => {
    const list = [
      { source: ' ', target: '', lang: 'ja' },
      { source: 'Owner', target: '', lang: '' }
    ];
    expect(compactGlossary(list)).toEqual([{ source: 'Owner', target: '', lang: '' }]);
    expect(compactGlossary(undefined)).toEqual([]);
  });
});
