import { describe, expect, it } from 'vitest';
import { flattenPaths, matchesQuery } from './schema-filter';

describe('matchesQuery', () => {
  it('returns true for any path when the query is empty', () => {
    expect(matchesQuery('providers.translate.chain', '')).toBe(true);
  });

  it('treats whitespace-only queries as empty', () => {
    expect(matchesQuery('anything', '   ')).toBe(true);
  });

  it('matches case-insensitively', () => {
    expect(matchesQuery('Hotkeys.CaptureRegion', 'capture')).toBe(true);
  });

  it('requires every token to appear', () => {
    expect(matchesQuery('providers.translate.chain', 'translate chain')).toBe(true);
    expect(matchesQuery('providers.translate.chain', 'translate banana')).toBe(false);
  });

  it('is order-independent across tokens', () => {
    expect(matchesQuery('providers.translate.chain', 'chain translate')).toBe(true);
  });

  it('rejects non-matching queries', () => {
    expect(matchesQuery('appearance.theme', 'chain')).toBe(false);
  });

  it('matches Japanese substrings', () => {
    // The UI may compose haystacks from translated labels; ensure unicode
    // tokens work too.
    expect(matchesQuery('設定.外観.テーマ', '外観')).toBe(true);
  });
});

describe('flattenPaths', () => {
  const schema = {
    properties: {
      a: {
        properties: {
          x: {},
          y: { properties: { deep: {} } }
        }
      },
      b: {}
    }
  };

  it('lists every nested property as a dotted path', () => {
    const paths = flattenPaths(schema);
    expect(paths).toContain('a');
    expect(paths).toContain('a.x');
    expect(paths).toContain('a.y');
    expect(paths).toContain('a.y.deep');
    expect(paths).toContain('b');
  });

  it('returns an empty list for a non-object input', () => {
    expect(flattenPaths(null)).toEqual([]);
    expect(flattenPaths(undefined)).toEqual([]);
    expect(flattenPaths('string')).toEqual([]);
  });

  it('returns an empty list for a schema with no properties block', () => {
    expect(flattenPaths({})).toEqual([]);
  });

  it('does not include the prefix as a standalone entry', () => {
    // A call with prefix = "root" should not emit "root" itself — only
    // its descendants.
    const paths = flattenPaths({ properties: { a: {} } }, 'root');
    expect(paths).toEqual(['root.a']);
  });
});
