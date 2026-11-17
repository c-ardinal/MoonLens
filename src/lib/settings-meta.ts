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
  | { kind: 'hotkey'; path: string; label: string; help?: string };

export type Section =
  | { id: string; title: string; description?: string; fields: Field[] }
  /** A section rendered by a dedicated component (model downloads). */
  | { id: string; title: string; description?: string; component: 'models' };

const LANGUAGE_OPTIONS: Option[] = LANGUAGES.map((l) => ({ value: l.code, label: l.label }));

export const SECTIONS: Section[] = [
  {
    id: 'translation',
    title: '翻訳',
    description: '画面翻訳とライブ字幕の訳し方。',
    fields: [
      {
        kind: 'select',
        path: 'subtitle.target_language',
        label: '翻訳先の言語',
        help: '画面の翻訳とライブ字幕の両方に使います。範囲選択のツールバーでもその場で切り替えられます。',
        options: LANGUAGE_OPTIONS
      }
    ]
  },
  {
    id: 'models',
    title: 'モデル',
    description: '翻訳・説明・文字認識のモデルは端末に保存され、すべて端末内で動きます。',
    component: 'models'
  },
  {
    id: 'hotkeys',
    title: 'ショートカット',
    description: 'どのアプリを使っていても効くキー操作です。欄をクリックして新しいキーを押すと変更できます。',
    fields: [
      {
        kind: 'hotkey',
        path: 'hotkeys.capture_region',
        label: '範囲を選んで翻訳',
        help: '画面を止めて、ドラッグした範囲をその場で翻訳します。'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.capture_window',
        label: '画面全体を翻訳',
        help: 'カーソルのある画面をそのまま全部翻訳します。'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.toggle_pin_overlay',
        label: 'ピン留めの切り替え',
        help: '直前に翻訳した範囲を見張り、内容が変わったら自動で訳し直します。'
      },
      { kind: 'hotkey', path: 'hotkeys.toggle_subtitle', label: 'ライブ字幕の開始・停止' },
      { kind: 'hotkey', path: 'hotkeys.open_command_palette', label: 'コマンドパレット' },
      { kind: 'hotkey', path: 'hotkeys.open_history', label: '履歴を開く' }
    ]
  },
  {
    id: 'subtitle',
    title: 'ライブ字幕',
    description: 'パソコンで再生中の音声を文字起こしして翻訳します。',
    fields: [
      {
        kind: 'select',
        path: 'subtitle.source_language',
        label: '話されている言語',
        options: [{ value: 'auto', label: '自動判定' }, ...LANGUAGE_OPTIONS]
      },
      {
        kind: 'toggle',
        path: 'subtitle.show_original',
        label: '原文も表示する',
        help: '訳文の上に、聞き取った原文を小さく表示します。'
      },
      {
        kind: 'number',
        path: 'subtitle.font_size_px',
        label: '文字の大きさ',
        min: 14,
        max: 48,
        step: 1,
        unit: 'px',
        slider: true
      },
      {
        kind: 'number',
        path: 'subtitle.chunk_seconds',
        label: '区切りの長さ',
        help: '短いほど早く表示され、長いほど文脈がつながって訳が安定します。',
        min: 1,
        max: 10,
        step: 0.5,
        unit: '秒',
        slider: true
      },
      {
        kind: 'number',
        path: 'subtitle.overlap_seconds',
        label: '区切りの重なり',
        help: '区切りの境目で言葉が切れないよう、前の区切りと重ねて聞き取る長さです。',
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
    title: 'ピン留め',
    description: '翻訳した範囲をピン留めすると、内容の変化を見張って自動で訳し直します。',
    fields: [
      {
        kind: 'toggle',
        path: 'overlay.follow_source',
        label: '変化を見張って自動で再翻訳',
        help: 'オフにすると、ピン留めしても自動では訳し直しません。'
      },
      {
        kind: 'number',
        path: 'performance.region_watcher_poll_ms',
        label: '見張る間隔',
        help: '短いほど早く反応し、CPU の負荷が上がります。',
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
    description: 'ピン留めした範囲の訳文などを表示する小さな窓の見た目です。',
    fields: [
      {
        kind: 'number',
        path: 'overlay.font_size_px',
        label: '文字の大きさ',
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
        label: '最大の幅',
        min: 240,
        max: 480,
        step: 10,
        unit: 'px',
        slider: true
      },
      {
        kind: 'number',
        path: 'overlay.opacity',
        label: '背景の不透明度',
        help: '下げると後ろの画面が透けて見えます。',
        min: 0.5,
        max: 1,
        step: 0.02,
        slider: true
      },
      {
        kind: 'number',
        path: 'overlay.blur_px',
        label: '背景のぼかし',
        min: 0,
        max: 40,
        step: 1,
        unit: 'px',
        slider: true
      },
      {
        kind: 'number',
        path: 'overlay.auto_hide_ms',
        label: '自動で閉じるまでの時間',
        help: '訳文を表示してからこの時間で閉じます。ピン留め中とマウスを乗せている間は閉じません。0 で閉じない。',
        min: 0,
        max: 30000,
        step: 500,
        unit: 'ミリ秒',
        slider: true,
        zeroLabel: '閉じない'
      }
    ]
  },
  {
    id: 'clipboard',
    title: 'クリップボード',
    description: '結果を自動でクリップボードにコピーします。',
    fields: [
      { kind: 'toggle', path: 'clipboard.auto_copy_translation', label: '訳文をコピー' },
      {
        kind: 'toggle',
        path: 'clipboard.auto_copy_source',
        label: '原文をコピー',
        help: '訳文のコピーもオンのときは、訳文がクリップボードに残ります。'
      },
      { kind: 'toggle', path: 'clipboard.auto_copy_qr', label: 'QR コードの中身をコピー' },
      { kind: 'toggle', path: 'clipboard.auto_copy_transcript', label: 'ライブ字幕の文字起こしをコピー' }
    ]
  },
  {
    id: 'history',
    title: '履歴',
    description: '履歴はこの端末にだけ保存されます。',
    fields: [
      { kind: 'toggle', path: 'storage.keep_history', label: '翻訳の履歴を残す' },
      {
        kind: 'number',
        path: 'storage.retention_days',
        label: '残す期間',
        help: 'これより古い履歴は自動で消えます。0 で無期限。',
        min: 0,
        max: 3650,
        step: 1,
        unit: '日',
        zeroLabel: '無期限'
      },
      {
        kind: 'number',
        path: 'storage.max_entries',
        label: '残す件数の上限',
        help: '超えた分は古いものから消えます。0 で上限なし。',
        min: 0,
        max: 1000000,
        step: 100,
        unit: '件',
        zeroLabel: '上限なし'
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
          { value: 'system', label: 'システムに合わせる' },
          { value: 'light', label: 'ライト' },
          { value: 'dark', label: 'ダーク' }
        ]
      },
      {
        kind: 'select',
        path: 'appearance.density',
        label: '表示の密度',
        options: [
          { value: 'compact', label: 'コンパクト' },
          { value: 'normal', label: '標準' },
          { value: 'comfortable', label: 'ゆったり' }
        ]
      },
      { kind: 'color', path: 'appearance.accent', label: 'アクセントカラー' },
      {
        kind: 'toggle',
        path: 'appearance.reduced_motion',
        label: '動きを減らす',
        help: 'アニメーションを止めます。'
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
        label: 'GPU を使う',
        help: '使える GPU があれば翻訳に使います。変更は次の翻訳から反映されます。'
      },
      {
        kind: 'number',
        path: 'performance.max_concurrent_tasks',
        label: '同時に翻訳する数',
        help: '段落を一つずつ訳し直すときに、いくつ並行して処理するか。メモリが少ない PC では減らしてください。',
        min: 1,
        max: 8,
        step: 1,
        slider: true
      },
      {
        kind: 'number',
        path: 'performance.cache_ttl_seconds',
        label: '訳文を使い回す期間',
        help: '同じ文章をもう一度選んだとき、この期間内なら前の訳文をすぐ表示します。0 で無期限（アプリを閉じるまで）。',
        min: 0,
        max: 604800,
        step: 60,
        unit: '秒',
        zeroLabel: '無期限'
      }
    ]
  },
  {
    id: 'advanced',
    title: '詳細',
    fields: [
      {
        kind: 'select',
        path: 'privacy.log_level',
        label: 'ログの詳しさ',
        help: '不具合を調べるときだけ上げてください。ログはこの端末の中にだけ出力されます。',
        options: [
          { value: 'error', label: 'エラーのみ' },
          { value: 'warn', label: '警告以上' },
          { value: 'info', label: '標準' },
          { value: 'debug', label: '詳細' },
          { value: 'trace', label: 'すべて' }
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

/** The schema's default for a field (from the section's `default` block). */
export function schemaDefault(schema: any, path: string): any {
  const [head, ...rest] = path.split('.');
  const section = schema?.properties?.[head];
  return rest.length ? getPath(section?.default, rest.join('.')) : section?.default;
}
