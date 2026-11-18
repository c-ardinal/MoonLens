/**
 * What the settings screen shows, in which group, under which Japanese
 * label. The settings struct (src-tauri/src/settings/schema.rs) holds more
 * fields than this list on purpose: fields the app does not act on yet,
 * cloud / network switches (MoonLens is fully local), and internal state
 * stay in config.toml only, so the screen never offers a switch that does
 * nothing. When a field starts doing something, add it here.
 */

import { LANGUAGES } from './lens-overlay';

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
  | { id: string; title: string; description?: string; component: 'models' | 'data' };

const LANGUAGE_OPTIONS: Option[] = LANGUAGES.map((l) => ({ value: l.code, label: l.label }));

export const SECTIONS: Section[] = [
  {
    id: 'translation',
    title: '翻訳',
    description: '画面翻訳・ライブ字幕の共通設定。',
    fields: [
      {
        kind: 'select',
        path: 'subtitle.target_language',
        label: '翻訳先言語',
        help: '画面翻訳・ライブ字幕に共通。範囲選択ツールバーでも一時変更可能。',
        options: LANGUAGE_OPTIONS
      },
      {
        kind: 'select',
        path: 'translation.style',
        label: '文体',
        help: '日本語への翻訳時のみ適用。',
        options: [
          { value: 'auto', label: '自動' },
          { value: 'polite', label: '敬体（です・ます）' },
          { value: 'plain', label: '常体（だ・である）' }
        ]
      },
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
    title: 'モデル',
    description: '全モデルをローカル保存・ローカル実行。',
    component: 'models'
  },
  {
    id: 'hotkeys',
    title: 'ショートカット',
    description: '全アプリ共通のグローバルショートカット。欄をクリック後、キー入力で変更。',
    fields: [
      {
        kind: 'hotkey',
        path: 'hotkeys.capture_region',
        label: '範囲翻訳',
        help: '画面を静止し、ドラッグ範囲を翻訳。'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.capture_window',
        label: '全画面翻訳',
        help: 'カーソル位置のディスプレイ全体を翻訳。'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.toggle_pin_overlay',
        label: 'ライブ翻訳切替',
        help: '直前の翻訳範囲を監視し、変化時に自動再翻訳。'
      },
      { kind: 'hotkey', path: 'hotkeys.toggle_subtitle', label: 'ライブ字幕 開始／停止' },
      { kind: 'hotkey', path: 'hotkeys.open_command_palette', label: 'コマンドパレット' },
      { kind: 'hotkey', path: 'hotkeys.open_history', label: '履歴表示' }
    ]
  },
  {
    id: 'subtitle',
    title: 'ライブ字幕',
    description: 'システム音声を文字起こし・翻訳して字幕表示。',
    fields: [
      {
        kind: 'select',
        path: 'subtitle.source_language',
        label: '音声の言語',
        options: [{ value: 'auto', label: '自動判定' }, ...LANGUAGE_OPTIONS]
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
        help: '短い：低遅延。長い：文脈保持・訳質安定。',
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
      }
    ]
  },
  {
    id: 'pin',
    title: 'ライブ翻訳',
    description: '選択範囲の上に訳文を重ね、画面の変化に追従して再翻訳。',
    fields: [
      {
        kind: 'toggle',
        path: 'overlay.follow_source',
        label: 'ライブ翻訳',
        help: 'オフ時はライブ翻訳を停止。'
      },
      {
        kind: 'number',
        path: 'performance.region_watcher_poll_ms',
        label: '監視間隔',
        help: '画面変化の検出間隔。短いほど高応答・高 CPU 負荷。',
        min: 100,
        max: 2000,
        step: 50,
        unit: 'ミリ秒',
        slider: true
      }
    ]
  },
  {
    id: 'popup',
    title: '訳文ポップアップ',
    description: 'ピン留め結果などを表示する小型ウィンドウの外観。',
    fields: [
      {
        kind: 'number',
        path: 'overlay.font_size_px',
        label: '文字サイズ',
        min: 10,
        max: 28,
        step: 1,
        unit: 'px',
        slider: true
      },
      {
        kind: 'number',
        path: 'overlay.line_height',
        label: '行間',
        min: 1.1,
        max: 2,
        step: 0.05,
        unit: '倍',
        slider: true
      },
      {
        kind: 'number',
        path: 'overlay.max_width_px',
        label: '最大幅',
        min: 240,
        max: 480,
        step: 10,
        unit: 'px',
        slider: true
      },
      {
        kind: 'number',
        path: 'overlay.opacity',
        label: '背景不透明度',
        min: 0.5,
        max: 1,
        step: 0.02,
        slider: true
      },
      {
        kind: 'number',
        path: 'overlay.blur_px',
        label: '背景ぼかし',
        min: 0,
        max: 40,
        step: 1,
        unit: 'px',
        slider: true
      },
      {
        kind: 'number',
        path: 'overlay.auto_hide_ms',
        label: '自動非表示',
        help: '表示後、指定時間で非表示。ピン留め中・ホバー中は無効。0：無効。',
        min: 0,
        max: 30000,
        step: 500,
        unit: 'ミリ秒',
        slider: true,
        zeroLabel: '無効'
      }
    ]
  },
  {
    id: 'clipboard',
    title: 'クリップボード',
    description: '結果の自動コピー。',
    fields: [
      { kind: 'toggle', path: 'clipboard.auto_copy_translation', label: '訳文' },
      {
        kind: 'toggle',
        path: 'clipboard.auto_copy_source',
        label: '原文',
        help: '訳文と併用時は訳文を優先。'
      },
      { kind: 'toggle', path: 'clipboard.auto_copy_qr', label: 'QR コード内容' },
      { kind: 'toggle', path: 'clipboard.auto_copy_transcript', label: 'ライブ字幕の文字起こし' }
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
        help: '超過分を古い順に自動削除。0：無制限。',
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
    fields: [
      {
        kind: 'select',
        path: 'appearance.theme',
        label: 'テーマ',
        options: [
          { value: 'system', label: 'システム設定' },
          { value: 'light', label: 'ライト' },
          { value: 'dark', label: 'ダーク' }
        ]
      },
      {
        kind: 'select',
        path: 'appearance.density',
        label: '表示密度',
        options: [
          { value: 'compact', label: 'コンパクト' },
          { value: 'normal', label: '標準' },
          { value: 'comfortable', label: 'ワイド' }
        ]
      },
      { kind: 'color', path: 'appearance.accent', label: 'アクセントカラー' },
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
        help: '段落単位の再翻訳時の同時処理数。低メモリ環境では減少推奨。',
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
    id: 'data',
    title: 'データ',
    description: '保存先と使用容量。アンインストール時の削除対象。',
    component: 'data'
  },
  {
    id: 'advanced',
    title: '詳細',
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
  const hay = [field.label, field.help ?? '', section.title, field.path].join(' ').toLowerCase();
  return q.split(/\s+/).every((t) => hay.includes(t));
}

/** Paths whose value differs between two settings objects. */
export function changedPaths(a: any, b: any): string[] {
  return allFields()
    .map((f) => f.path)
    .filter((p) => JSON.stringify(getPath(a, p) ?? null) !== JSON.stringify(getPath(b, p) ?? null));
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
