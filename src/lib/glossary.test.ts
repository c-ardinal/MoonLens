import { describe, expect, it } from 'vitest';
import { addEntry, compactGlossary, createRemovalLog, insertEntry, isBlankEntry, removeEntry, updateEntry } from './glossary';

describe('glossary helpers', () => {
  it('adds, edits and removes rows without mutating the input', () => {
    const a = addEntry([], 'ja');
    expect(a).toEqual([{ source: '', target: '', lang: 'ja' }]);
    const b = updateEntry(a, 0, { source: 'Dashboard', target: '管理画面' });
    expect(b[0]).toEqual({ source: 'Dashboard', target: '管理画面', lang: 'ja' });
    expect(a[0].source).toBe('');
    expect(removeEntry(b, 0)).toEqual([]);
  });

  it('puts a removed row back at its position', () => {
    const list = [
      { source: 'A', target: 'あ', lang: '' },
      { source: 'B', target: 'い', lang: 'ja' },
      { source: 'C', target: 'う', lang: '' }
    ];
    expect(insertEntry(removeEntry(list, 1), 1, list[1])).toEqual(list);
    expect(insertEntry(removeEntry(list, 2), 2, list[2])).toEqual(list);
    // The list shrank meanwhile: the row goes to the end.
    expect(insertEntry([list[0]], 5, list[2])).toEqual([list[0], list[2]]);
  });

  it('restores several removed rows to their original order, in any undo order', () => {
    const full = ['A', 'B', 'C', 'D'].map((source) => ({ source, target: source.toLowerCase(), lang: '' }));
    const names = (l: { source: string }[]) => l.map((e) => e.source).join('');
    // Every removal order of three rows × every undo order.
    const orders = [
      [0, 1, 2],
      [0, 2, 1],
      [1, 0, 2],
      [1, 2, 0],
      [2, 0, 1],
      [2, 1, 0]
    ];
    for (const removal of [['A', 'B', 'C'], ['C', 'A', 'D'], ['B', 'D', 'A'], ['D', 'C', 'B'], ['B', 'A', 'C']]) {
      for (const undo of orders) {
        const log = createRemovalLog();
        let list = full;
        const rows = removal.map((name) => {
          const r = log.remove(list, list.findIndex((e) => e.source === name));
          list = r.list;
          return r.row;
        });
        expect(list).toHaveLength(1);
        for (const i of undo) list = log.restore(list, rows[i]);
        expect(names(list), `${removal.join('')} / ${undo.join('')}`).toBe('ABCD');
      }
    }
  });

  it('restores a row once, and next to rows added meanwhile', () => {
    const log = createRemovalLog();
    const a = { source: 'A', target: 'a', lang: '' };
    const b = { source: 'B', target: 'b', lang: '' };
    const { list, row } = log.remove([a, b], 0);
    const grown = addEntry(list, 'ja');
    const back = log.restore(grown, row);
    expect(back.map((e) => e.source)).toEqual(['A', 'B', '']);
    expect(log.restore(back, row)).toBe(back);
  });

  it('tells blank rows from filled ones', () => {
    expect(isBlankEntry({ source: ' ', target: '', lang: 'ja' })).toBe(true);
    expect(isBlankEntry({ source: '', target: '訳', lang: '' })).toBe(false);
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
