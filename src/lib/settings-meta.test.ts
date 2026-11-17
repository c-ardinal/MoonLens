import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { allFields, changedPaths, fieldMatches, getPath, schemaDefault, SECTIONS, setPath } from './settings-meta';

/** `section.key` pairs declared in the embedded default config. */
function defaultTomlPaths(): Set<string> {
  const toml = readFileSync('src-tauri/src/settings/default.toml', 'utf8');
  const out = new Set<string>();
  let section = '';
  for (const line of toml.split('\n')) {
    const header = line.match(/^\[([a-z_.]+)\]/);
    if (header) section = header[1];
    const kv = line.match(/^([a-z_]+)\s*=/);
    if (kv && section) out.add(`${section}.${kv[1]}`);
  }
  return out;
}

describe('settings-meta', () => {
  it('only offers fields that exist in the settings struct', () => {
    const known = defaultTomlPaths();
    for (const f of allFields()) expect(known, f.path).toContain(f.path);
  });

  it('never surfaces cloud / network switches', () => {
    const paths = allFields().map((f) => f.path);
    expect(paths.some((p) => /cloud|network|reverse_image|providers\./.test(p))).toBe(false);
  });

  it('has unique paths and section ids', () => {
    const paths = allFields().map((f) => f.path);
    expect(new Set(paths).size).toBe(paths.length);
    expect(new Set(SECTIONS.map((s) => s.id)).size).toBe(SECTIONS.length);
  });

  it('gets and sets nested values without mutating the input', () => {
    const a = { subtitle: { target_language: 'ja', chunk_seconds: 3 }, other: 1 };
    const b = setPath(a, 'subtitle.target_language', 'en');
    expect(getPath(b, 'subtitle.target_language')).toBe('en');
    expect(getPath(b, 'subtitle.chunk_seconds')).toBe(3);
    expect(b.other).toBe(1);
    expect(a.subtitle.target_language).toBe('ja');
  });

  it('lists the paths that changed', () => {
    const a = { appearance: { theme: 'system' }, clipboard: { auto_copy_qr: true } };
    const b = setPath(a, 'appearance.theme', 'dark');
    expect(changedPaths(a, b)).toEqual(['appearance.theme']);
  });

  it('reads defaults from the schema section defaults', () => {
    const schema = { properties: { overlay: { default: { follow_source: true } } } };
    expect(schemaDefault(schema, 'overlay.follow_source')).toBe(true);
    expect(schemaDefault(schema, 'missing.key')).toBeUndefined();
  });

  it('searches labels, help text and section titles', () => {
    const section = SECTIONS.find((s) => s.id === 'performance')!;
    const gpu = allFields().find((f) => f.path === 'performance.use_gpu_if_available')!;
    expect(fieldMatches(gpu, section, 'gpu')).toBe(true);
    expect(fieldMatches(gpu, section, 'パフォーマンス')).toBe(true);
    expect(fieldMatches(gpu, section, '字幕')).toBe(false);
  });
});
