import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  allFields,
  changedPaths,
  savable,
  heldHotkeys,
  saveDelay,
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

  it('ignores f32 rounding when listing changes', () => {
    const a = { subtitle: { overlap_seconds: 0.1 } };
    const b = { subtitle: { overlap_seconds: 0.10000000149011612 } };
    expect(changedPaths(a, b)).toEqual([]);
    expect(changedPaths(a, { subtitle: { overlap_seconds: 0.2 } })).toEqual(['subtitle.overlap_seconds']);
  });

  it('keeps clashing hotkeys and blank glossary rows out of what is saved', () => {
    const saved = {
      hotkeys: { capture_region: 'Alt+A', open_history: 'Alt+H' },
      appearance: { theme: 'dark' },
      translation: { glossary: [] }
    };
    const draft = {
      hotkeys: { capture_region: 'Alt+H', open_history: 'Alt+H' },
      appearance: { theme: 'light' },
      translation: { glossary: [{ source: '', target: '', lang: 'ja' }, { source: 'Lens', target: 'レンズ', lang: 'ja' }] }
    };
    const next = savable(saved, draft);
    expect(next.hotkeys).toEqual(saved.hotkeys);
    expect(next.appearance.theme).toBe('light');
    expect(next.translation.glossary).toEqual([{ source: 'Lens', target: 'レンズ', lang: 'ja' }]);
    expect(changedPaths(saved, next)).toEqual(['translation.glossary', 'appearance.theme']);
  });

  it('drops blank rows of application glossaries and keeps the applications', () => {
    const term = { source: 'Ether', target: 'エーテル', lang: '' };
    const draft: any = {
      translation: {
        glossary: [],
        app_glossaries: [
          { app: 'game', name: 'Game', glossary: [{ source: '', target: '', lang: 'ja' }, term] },
          { app: 'novel', name: 'Novel', glossary: [] }
        ]
      }
    };
    expect(savable({}, draft).translation.app_glossaries).toEqual([
      { app: 'game', name: 'Game', glossary: [term] },
      { app: 'novel', name: 'Novel', glossary: [] }
    ]);
    // Settings written before the list existed stay as they are.
    expect('app_glossaries' in savable<any>({}, { translation: { glossary: [] } }).translation).toBe(false);
    expect(saveDelay('translation.app_glossaries')).toBeGreaterThan(0);
  });

  it('holds back a change that would clash with a restored shortcut', () => {
    const saved = { hotkeys: { capture_region: 'Alt+A', toggle_subtitle: 'Alt+B', open_history: 'Alt+C' } };
    // B -> Alt+C (clash with C), then A -> Alt+B (clash with the restored B).
    const draft = { hotkeys: { capture_region: 'Alt+B', toggle_subtitle: 'Alt+C', open_history: 'Alt+C' } };
    const next = savable(saved, draft);
    expect(duplicateHotkeys(next)).toEqual([]);
    expect(next.hotkeys).toEqual(saved.hotkeys);
    expect(heldHotkeys(saved, draft)).toEqual(['hotkeys.capture_region', 'hotkeys.toggle_subtitle']);
    // Resolving the first clash releases both.
    const fixed = setPath(draft, 'hotkeys.open_history', 'Alt+D');
    expect(savable(saved, fixed).hotkeys).toEqual(fixed.hotkeys);
    expect(heldHotkeys(saved, fixed)).toEqual([]);
  });

  it('delays saving only for continuous inputs', () => {
    expect(saveDelay('appearance.theme')).toBe(0);
    expect(saveDelay('hotkeys.open_history')).toBe(0);
    expect(saveDelay('subtitle.font_size_px')).toBeGreaterThan(0);
    expect(saveDelay('appearance.accent')).toBeGreaterThan(0);
    expect(saveDelay('translation.glossary')).toBeGreaterThan(0);
  });

  it('finds hotkeys bound to the same shortcut', () => {
    const base = {
      hotkeys: {
        capture_region: 'CommandOrControl+Shift+Space',
        capture_window: 'CommandOrControl+Shift+W',
        toggle_pin_overlay: '',
        toggle_subtitle: 'Shift+CommandOrControl+Space',
        open_command_palette: 'CommandOrControl+K',
        open_history: 'Shift+CommandOrControl+W'
      }
    };
    // capture_window is no longer bound, so it clashes with nothing.
    expect(duplicateHotkeys(base).sort()).toEqual(['hotkeys.capture_region', 'hotkeys.toggle_subtitle']);
    // Empty (disabled) bindings never clash.
    expect(duplicateHotkeys(setPath(base, 'hotkeys.toggle_subtitle', ''))).toEqual([]);
  });

  it('holds extra triggers bound to the same input, apart from the shortcuts', () => {
    const saved = {
      hotkeys: { capture_region: 'Alt+A' },
      triggers: { capture_region: 'Mouse4', toggle_subtitle: '', open_history: 'Pad:Back+Y' }
    };
    const draft = setPath(saved, 'triggers.toggle_subtitle', 'Mouse4');
    expect(duplicateHotkeys(draft).sort()).toEqual(['triggers.capture_region', 'triggers.toggle_subtitle']);
    expect(heldHotkeys(saved, draft)).toEqual(['triggers.toggle_subtitle']);
    expect(savable(saved, draft).triggers).toEqual(saved.triggers);
    expect(duplicateHotkeys(setPath(saved, 'triggers.toggle_subtitle', 'Mouse5'))).toEqual([]);
  });

  it('offers an extra trigger for every shortcut action', () => {
    const paths = (kind: string) =>
      allFields()
        .filter((f) => f.kind === kind)
        .map((f) => f.path.split('.')[1]);
    expect(paths('trigger')).toEqual(paths('hotkey'));
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

describe('音声翻訳の遮断文', () => {
  it('空行は保存対象外、入力の間は保存を待機', () => {
    const saved = { subtitle: { blocked_phrases: [] } };
    const draft = { subtitle: { blocked_phrases: ['Thank you.', '', '  '] } };
    expect(savable(saved, draft).subtitle.blocked_phrases).toEqual(['Thank you.']);
    expect(saveDelay('subtitle.blocked_phrases')).toBeGreaterThan(0);
    // 項目のない設定（旧版の保存値）へは追加なし。
    expect(savable({}, { appearance: {} })).not.toHaveProperty('subtitle');
  });
});
