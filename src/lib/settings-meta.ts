/**
 * What the settings screen shows, in which group, under which Japanese
 * label. The settings struct (src-tauri/src/settings/schema.rs) holds more
 * fields than this list on purpose: fields the app does not act on yet,
 * cloud / network switches (MoonLens is fully local), and internal state
 * stay in config.toml only, so the screen never offers a switch that does
 * nothing. When a field starts doing something, add it here.
 */

import { compactGlossary } from './glossary';
import { LANGUAGES, SOURCE_LANGUAGES } from './lens-overlay';

export type Option = { value: string; label: string };

export type Field =
  | { kind: 'toggle'; path: string; label: string; help?: string }
  | { kind: 'select'; path: string; label: string; help?: string; options: Option[] }
  | {
      kind: 'number';
      path: string;
      label: string;
      help?: string;
      min: number;
      max: number;
      step: number;
      unit?: string;
      /** Render as a slider with the value next to it. */
      slider?: boolean;
      /** Shown instead of the number when the value is 0 (e.g. 無期限). */
      zeroLabel?: string;
    }
  | { kind: 'color'; path: string; label: string; help?: string }
  | { kind: 'hotkey'; path: string; label: string; help?: string }
  /** Editable term list (rendered by GlossaryEditor). */
  | { kind: 'glossary'; path: string; label: string; help?: string };

export type Section =
  | { id: string; title: string; description?: string; fields: Field[] }
  /** A section rendered by a dedicated component. */
  | { id: string; title: string; description?: string; component: SectionComponent };

export type SectionComponent = 'llm-translate' | 'llm-assist' | 'ocr' | 'asr' | 'data' | 'plugins';

/**
 * 大分類（左の一覧）。各分類は SECTIONS の小分類（右枠上部のタブ）を束ねる。
 * 小分類が 1 個の分類はタブ非表示。
 */
export type Group = { id: string; title: string; sections: string[] };

const LANGUAGE_OPTIONS: Option[] = LANGUAGES.map((l) => ({ value: l.code, label: l.label }));
const SOURCE_LANGUAGE_OPTIONS: Option[] = SOURCE_LANGUAGES.map((l) => ({
  value: l.code,
  label: l.label
}));

export const SECTIONS: Section[] = [
  {
    id: 'translation',
    title: '基本',
    description: 'ウィンドウ翻訳・音声翻訳の共通設定。',
    fields: [
      {
        kind: 'select',
        path: 'subtitle.target_language',
        label: '翻訳先言語',
        help: 'ウィンドウ翻訳・音声翻訳に共通。ウィンドウ翻訳のツールバーでも一時変更可能。',
        options: LANGUAGE_OPTIONS
      },
      {
        kind: 'select',
        path: 'translation.style',
        label: '文体',
        help: '日本語への翻訳時のみ適用。やさしい日本語は対象外。',
        options: [
          { value: 'auto', label: '自動' },
          { value: 'polite', label: '敬体（です・ます）' },
          { value: 'plain', label: '常体（だ・である）' }
        ]
      }
    ]
  },
  {
    id: 'glossary',
    title: '用語集',
    description: '原文中の用語を指定の訳語に固定。',
    fields: [
      {
        kind: 'glossary',
        path: 'translation.glossary',
        label: '用語集',
        help: '原文中の用語を指定の訳語に固定。'
      }
    ]
  },
  {
    id: 'models',
    title: '翻訳',
    description: '翻訳用モデル。推奨：PLaMo 3 Translate。',
    component: 'llm-translate'
  },
  {
    id: 'model-assist',
    title: '説明・要約',
    description: '説明・要約用モデル。推奨：Gemma 4。',
    component: 'llm-assist'
  },
  {
    id: 'model-ocr',
    title: '文字認識',
    description: '画面上の文字認識（OCR）用モデル。',
    component: 'ocr'
  },
  {
    id: 'model-asr',
    title: '音声認識',
    description: '音声翻訳用の音声認識モデル。',
    component: 'asr'
  },
  {
    id: 'hotkeys',
    title: 'ショートカット',
    description: '全アプリ共通のグローバルショートカット。欄をクリック後、キー入力で変更。',
    fields: [
      {
        kind: 'hotkey',
        path: 'hotkeys.capture_region',
        label: 'ウィンドウ翻訳',
        help: 'マウス位置へウィンドウ翻訳を追加。'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.toggle_pin_overlay',
        label: 'ウィンドウ翻訳 表示／全終了',
        help: 'ウィンドウなし：1 件表示。ウィンドウあり：全件終了。'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.toggle_window_operate',
        label: 'ウィンドウ翻訳 透過／操作',
        help: '全ウィンドウの切替。透過：ウィンドウ上のクリックは背面へ。操作：訳文の選択・コピー可。'
      },
      { kind: 'hotkey', path: 'hotkeys.toggle_subtitle', label: '音声翻訳 開始／停止' },
      { kind: 'hotkey', path: 'hotkeys.open_command_palette', label: 'コマンドパレット' },
      { kind: 'hotkey', path: 'hotkeys.open_history', label: '履歴表示' }
    ]
  },
  {
    id: 'subtitle',
    title: '音声翻訳',
    description: 'システム音声を認識・翻訳し、字幕表示。',
    fields: [
      {
        kind: 'select',
        path: 'subtitle.source_language',
        label: '音声言語',
        options: [{ value: 'auto', label: '自動判定' }, ...SOURCE_LANGUAGE_OPTIONS]
      },
      {
        kind: 'toggle',
        path: 'subtitle.show_original',
        label: '原文表示',
        help: '訳文の上に原文を併記。'
      },
      {
        kind: 'number',
        path: 'subtitle.font_size_px',
        label: '文字サイズ',
        min: 14,
        max: 48,
        step: 1,
        unit: 'px',
        slider: true
      },
      {
        kind: 'number',
        path: 'subtitle.chunk_seconds',
        label: '分割長',
        help: '短：低遅延。長：文脈保持・訳質安定。',
        min: 1,
        max: 10,
        step: 0.5,
        unit: '秒',
        slider: true
      },
      {
        kind: 'number',
        path: 'subtitle.overlap_seconds',
        label: '重複区間',
        help: '分割境界での語の欠落防止用。',
        min: 0,
        max: 2,
        step: 0.1,
        unit: '秒',
        slider: true
      },
      {
        kind: 'toggle',
        path: 'subtitle.realtime_correction',
        label: 'リアルタイム補正',
        help: '文の認識完了時、文脈を参照して文全体を再翻訳、訳文を更新。負荷増。'
      },
      {
        kind: 'toggle',
        path: 'subtitle.batch_correction',
        label: '一括補正',
        help: '無音時・終了時に未補正の文を一括で再翻訳。発話中の負荷なし。'
      }
    ]
  },
  {
    id: 'pin',
    title: '常時翻訳',
    description: 'ウィンドウ翻訳の常時モード。画面変化へ追従して再翻訳。',
    fields: [
      {
        kind: 'number',
        path: 'performance.scan_margin_px',
        label: '読取余白',
        help: 'ウィンドウ枠の外側へ加算する読取幅。枠で分断された文字を完全な形で認識。訳文の表示はウィンドウ内に限定。',
        min: 0,
        max: 128,
        step: 4,
        unit: 'px',
        zeroLabel: '無効',
        slider: true
      },
      {
        kind: 'number',
        path: 'performance.region_watcher_poll_ms',
        label: '監視間隔',
        help: '画面変化の検出間隔。短時間ほど応答性向上、CPU 負荷増大。',
        min: 100,
        max: 2000,
        step: 50,
        unit: 'ミリ秒',
        slider: true
      },
      {
        kind: 'toggle',
        path: 'overlay.show_in_screenshots',
        label: '撮影への表示',
        help: 'スクリーンショット・録画へ翻訳ウィンドウを表示。OFF：常時除外。Windows 専用。'
      }
    ]
  },
  {
    id: 'clipboard',
    title: '自動コピー',
    description: '翻訳・認識の結果をクリップボードへ自動コピー。',
    fields: [
      { kind: 'toggle', path: 'clipboard.auto_copy_translation', label: '訳文' },
      {
        kind: 'toggle',
        path: 'clipboard.auto_copy_source',
        label: '原文',
        help: '訳文と併用時は訳文を優先。'
      },
      { kind: 'toggle', path: 'clipboard.auto_copy_qr', label: 'QR コード内容' },
      { kind: 'toggle', path: 'clipboard.auto_copy_transcript', label: '音声翻訳の認識結果' }
    ]
  },
  {
    id: 'history',
    title: '履歴',
    description: '履歴はローカルにのみ保存。',
    fields: [
      { kind: 'toggle', path: 'storage.keep_history', label: '履歴保存' },
      {
        kind: 'number',
        path: 'storage.retention_days',
        label: '保持期間',
        help: '期間超過分を自動削除。0：無期限。',
        min: 0,
        max: 3650,
        step: 1,
        unit: '日',
        zeroLabel: '無期限'
      },
      {
        kind: 'number',
        path: 'storage.max_entries',
        label: '最大件数',
        help: '超過時、最古の履歴から自動削除。0：無制限。',
        min: 0,
        max: 1000000,
        step: 100,
        unit: '件',
        zeroLabel: '無制限'
      }
    ]
  },
  {
    id: 'appearance',
    title: '外観',
    description: 'メインウィンドウの外観。',
    fields: [
      {
        kind: 'select',
        path: 'appearance.theme',
        label: 'テーマ',
        help: 'メインウィンドウの配色。システム設定：OS の配色へ追従。',
        options: [
          { value: 'system', label: 'システム設定' },
          { value: 'light', label: 'ライト' },
          { value: 'dark', label: 'ダーク' }
        ]
      },
      {
        kind: 'select',
        path: 'appearance.text_size',
        label: '文字サイズ',
        help: '履歴・設定・翻訳画面の文字寸法。操作部も同率で拡縮。ホームは文字のみ拡縮。ナビゲーション・見出しは対象外。',
        options: [
          { value: 'xsmall', label: '最小（16px）' },
          { value: 'small', label: '小（20px）' },
          { value: 'medium', label: '標準（24px）' },
          { value: 'large', label: '大（28px）' },
          { value: 'xlarge', label: '最大（32px）' }
        ]
      },
      {
        kind: 'select',
        path: 'appearance.density',
        label: '表示密度',
        help: '設定・履歴の行間と区画間隔。文字・操作部の寸法は「文字サイズ」で変更。',
        options: [
          { value: 'compact', label: 'コンパクト' },
          { value: 'normal', label: '標準' },
          { value: 'comfortable', label: 'ワイド' }
        ]
      },
      {
        kind: 'color',
        path: 'appearance.accent',
        label: 'アクセントカラー',
        help: 'メインウィンドウの基調色。色相・彩度を反映、明度はテーマ単位で固定（可読性維持）。'
      },
      {
        kind: 'toggle',
        path: 'appearance.reduced_motion',
        label: 'モーション軽減',
        help: 'アニメーション無効化。'
      }
    ]
  },
  {
    id: 'performance',
    title: 'パフォーマンス',
    description: '翻訳処理の速度・負荷。',
    fields: [
      {
        kind: 'toggle',
        path: 'performance.use_gpu_if_available',
        label: 'GPU 使用',
        help: '利用可能な GPU で翻訳を高速化。次回翻訳時から適用。'
      },
      {
        kind: 'number',
        path: 'performance.max_concurrent_tasks',
        label: '並列翻訳数',
        help: '拡張機能の翻訳エンジン使用時、段落単位の再翻訳の同時要求数。内蔵エンジンは逐次実行のため対象外。',
        min: 1,
        max: 8,
        step: 1,
        slider: true
      },
      {
        kind: 'number',
        path: 'performance.cache_ttl_seconds',
        label: '翻訳キャッシュ保持期間',
        help: '同一文の再選択時、期間内なら前回の訳文を即時表示。0：アプリ終了まで。',
        min: 0,
        max: 604800,
        step: 60,
        unit: '秒',
        zeroLabel: '無期限'
      }
    ]
  },
  {
    id: 'plugins',
    title: '拡張機能',
    description: '拡張機能の追加・有効化・権限確認。変更は即時反映。',
    component: 'plugins'
  },
  {
    id: 'data',
    title: 'データ',
    description: '保存先と使用容量。完全削除：本体の削除後、各保存先を削除。',
    component: 'data'
  },
  {
    id: 'advanced',
    title: '詳細',
    description: '不具合調査用の設定。',
    fields: [
      {
        kind: 'select',
        path: 'privacy.log_level',
        label: 'ログレベル',
        help: '不具合調査時のみ変更推奨。出力先はローカルのみ。',
        options: [
          { value: 'error', label: 'エラー' },
          { value: 'warn', label: '警告' },
          { value: 'info', label: '標準' },
          { value: 'debug', label: '詳細' },
          { value: 'trace', label: 'トレース' }
        ]
      }
    ]
  }
];

export const GROUPS: Group[] = [
  { id: 'translation', title: '翻訳', sections: ['translation', 'glossary', 'pin'] },
  { id: 'subtitle', title: '音声翻訳', sections: ['subtitle'] },
  { id: 'models', title: 'モデル', sections: ['models', 'model-assist', 'model-ocr', 'model-asr'] },
  { id: 'output', title: '履歴・コピー', sections: ['history', 'clipboard'] },
  { id: 'hotkeys', title: 'ショートカット', sections: ['hotkeys'] },
  { id: 'plugins', title: '拡張', sections: ['plugins'] },
  { id: 'system', title: 'システム', sections: ['appearance', 'performance', 'data', 'advanced'] }
];

/** 小分類の所属する大分類。 */
export function groupOf(sectionId: string): Group | undefined {
  return GROUPS.find((g) => g.sections.includes(sectionId));
}

/* ------------------------------ value helpers ----------------------------- */

export function getPath(obj: any, path: string): any {
  return path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), obj);
}

/** Immutable set: returns a copy of `obj` with `path` replaced. */
export function setPath(obj: any, path: string, value: any): any {
  const [head, ...rest] = path.split('.');
  const base = obj ?? {};
  return {
    ...base,
    [head]: rest.length === 0 ? value : setPath(base[head], rest.join('.'), value)
  };
}

/** Every field path shown on the screen. */
export function allFields(): Field[] {
  return SECTIONS.flatMap((s) => ('fields' in s ? s.fields : []));
}

/** Case-insensitive match on label, help text and section title. */
export function fieldMatches(field: Field, section: Section, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const group = groupOf(section.id)?.title ?? '';
  const hay = [field.label, field.help ?? '', section.title, group, field.path].join(' ').toLowerCase();
  return q.split(/\s+/).every((t) => hay.includes(t));
}

// f32 values come back as 0.10000000149011612: not a change.
const sameValue = (a: any, b: any) =>
  typeof a === 'number' && typeof b === 'number'
    ? Math.abs(a - b) < 1e-6
    : JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

/** Paths whose value differs between two settings objects. */
export function changedPaths(a: any, b: any): string[] {
  return allFields()
    .map((f) => f.path)
    .filter((p) => !sameValue(getPath(a, p), getPath(b, p)));
}

/**
 * 即時保存の対象：編集中の値から保存不可の値を除外した設定。
 * 重複に関与する変更済ショートカットは保存済の値を維持、用語集の空行は除去。
 * 復元後の値と新規に重複する変更も同様に除外（保存結果に重複を残さない）。
 */
export function savable<T>(saved: T, draft: T): T {
  let next = draft;
  for (;;) {
    const held = duplicateHotkeys(next).filter((p) => !sameValue(getPath(next, p), getPath(saved, p)));
    if (held.length === 0) break;
    for (const p of held) next = setPath(next, p, getPath(saved, p));
  }
  return setPath(next, 'translation.glossary', compactGlossary(getPath(next, 'translation.glossary')));
}

/** 保存対象外のショートカット（重複の解消まで保存済の値を維持する項目）。 */
export function heldHotkeys(saved: any, draft: any): string[] {
  const next = savable(saved, draft);
  return allFields()
    .filter((f) => f.kind === 'hotkey' && !sameValue(getPath(next, f.path), getPath(draft, f.path)))
    .map((f) => f.path);
}

/** 連続入力の項目（スライダー・色・用語集）：保存までの待機時間（ミリ秒）。 */
export function saveDelay(path: string): number {
  const f = allFields().find((x) => x.path === path);
  if (f?.kind === 'glossary') return 600;
  return f?.kind === 'color' || (f?.kind === 'number' && f.slider) ? 300 : 0;
}

/**
 * Hotkey fields that share a shortcut with another one. Only one of them
 * could be registered, so the screen refuses to save such a set.
 * Returns the paths involved (empty when all are distinct).
 */
export function duplicateHotkeys(settings: any): string[] {
  const seen = new Map<string, string[]>();
  for (const f of allFields()) {
    if (f.kind !== 'hotkey') continue;
    const v = String(getPath(settings, f.path) ?? '')
      .trim()
      .toLowerCase();
    if (!v) continue;
    const norm = v.split('+').map((k) => (k === 'cmdorctrl' ? 'commandorcontrol' : k));
    const key = [...norm.slice(0, -1).sort(), norm[norm.length - 1]].join('+');
    seen.set(key, [...(seen.get(key) ?? []), f.path]);
  }
  return [...seen.values()].filter((p) => p.length > 1).flat();
}

/** The schema's default for a field (from the section's `default` block). */
export function schemaDefault(schema: any, path: string): any {
  const [head, ...rest] = path.split('.');
  const section = schema?.properties?.[head];
  return rest.length ? getPath(section?.default, rest.join('.')) : section?.default;
}
