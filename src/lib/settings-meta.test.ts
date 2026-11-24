import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  allFields,
  changedPaths,
  duplicateHotkeys,
  fieldMatches,
  GROUPS,
  getPath,
  schemaDefault,
  SECTIONS,
  setPath
} from './settings-meta';

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
    // Optional fields (Option<T>) have no line in default.toml.
    const schemaRs = readFileSync('src-tauri/src/settings/schema.rs', 'utf8');
    for (const f of allFields()) {
      const key = f.path.split('.').pop()!;
      expect(known.has(f.path) || schemaRs.includes(`pub ${key}: Option<`), f.path).toBe(true);
    }
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

  it('puts every section in exactly one group', () => {
    const listed = GROUPS.flatMap((g) => g.sections);
    expect(listed.sort()).toEqual(SECTIONS.map((s) => s.id).sort());
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

  it('finds hotkeys bound to the same shortcut', () => {
    const base = {
      hotkeys: {
        capture_region: 'CommandOrControl+Shift+Space',
        capture_window: 'CommandOrControl+Shift+W',
        toggle_pin_overlay: '',
        toggle_subtitle: 'Shift+CommandOrControl+W',
        open_command_palette: 'CommandOrControl+K',
        open_history: ''
      }
    };
    expect(duplicateHotkeys(base).sort()).toEqual(['hotkeys.capture_window', 'hotkeys.toggle_subtitle']);
    // Empty (disabled) bindings never clash.
    expect(duplicateHotkeys(setPath(base, 'hotkeys.toggle_subtitle', ''))).toEqual([]);
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
