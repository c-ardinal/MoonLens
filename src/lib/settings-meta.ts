/**
 * What the settings screen shows, in which group, under which label
 * (text keys of `locales/`: the screen passes labels, help texts and
 * titles through `t()` when it renders them). The settings struct (src-tauri/src/settings/schema.rs) holds more
 * fields than this list on purpose: fields the app does not act on yet,
 * cloud / network switches (MoonLens is fully local), and internal state
 * stay in config.toml only, so the screen never offers a switch that does
 * nothing. When a field starts doing something, add it here.
 */

import { compactGlossary } from './glossary';
import { LANGUAGES, SOURCE_LANGUAGES } from './lens-overlay';
import { LOCALES, t } from './i18n';

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
  | { kind: 'glossary'; path: string; label: string; help?: string }
  /** Editable list of phrases (rendered by PhraseList). */
  | { kind: 'phrases'; path: string; label: string; help?: string; placeholder?: string };

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
    title: 'settings:general',
    description: 'settings:settings_shared_by_window_and_audio',
    fields: [
      {
        kind: 'select',
        path: 'subtitle.target_language',
        label: 'common:target_language',
        help: 'settings:shared_by_window_and_audio_translation_the',
        options: LANGUAGE_OPTIONS
      },
      {
        kind: 'select',
        path: 'translation.style',
        label: 'settings:style',
        help: 'settings:applies_only_when_translating_into_japanese',
        options: [
          { value: 'auto', label: 'settings:auto' },
          { value: 'polite', label: 'settings:polite_desu_masu' },
          { value: 'plain', label: 'settings:plain_da_de_aru' }
        ]
      }
    ]
  },
  {
    id: 'glossary',
    title: 'settings:glossary',
    description: 'settings:pins_terms_in_the_source_to_the',
    fields: [
      {
        kind: 'glossary',
        path: 'translation.glossary',
        label: 'settings:glossary',
        help: 'settings:pins_terms_in_the_source_to_the'
      }
    ]
  },
  {
    id: 'models',
    title: 'common:translation',
    description: 'settings:translation_model_recommended_plamo_3',
    component: 'llm-translate'
  },
  {
    id: 'model-assist',
    title: 'common:explain_summarize',
    description: 'settings:model_for_explanations_and_summaries',
    component: 'llm-assist'
  },
  {
    id: 'model-ocr',
    title: 'common:text_recognition',
    description: 'settings:model_for_reading_on_screen_text_ocr',
    component: 'ocr'
  },
  {
    id: 'model-asr',
    title: 'common:speech_recognition',
    description: 'settings:speech_recognition_model_for_audio',
    component: 'asr'
  },
  {
    id: 'hotkeys',
    title: 'settings:shortcuts',
    description: 'settings:global_shortcuts_that_work_in_every_app',
    fields: [
      {
        kind: 'hotkey',
        path: 'hotkeys.capture_region',
        label: 'common:window_translation',
        help: 'settings:adds_a_translation_window_at_the_pointer'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.toggle_pin_overlay',
        label: 'settings:window_translation_open_close_all',
        help: 'settings:no_window_open_opens_one_windows_open'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.toggle_window_operate',
        label: 'settings:window_translation_click_through_interact',
        help: 'settings:switches_every_window_click_through_clicks'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.toggle_window_pause',
        label: 'common:window_translation_pause_resume',
        help: 'settings:stops_reading_and_translation_in_all_windows'
      },
      {
        kind: 'hotkey',
        path: 'hotkeys.hold_window_original',
        label: 'settings:window_translation_show_source_while_held',
        help: 'settings:hides_translations_in_all_windows_and_shows'
      },
      { kind: 'hotkey', path: 'hotkeys.toggle_subtitle', label: 'common:audio_translation_start_stop' },
      { kind: 'hotkey', path: 'hotkeys.open_command_palette', label: 'settings:command_palette' },
      { kind: 'hotkey', path: 'hotkeys.open_history', label: 'settings:open_history' }
    ]
  },
  {
    id: 'subtitle',
    title: 'common:audio_translation',
    description: 'common:subtitles_for_system_audio_translated_live',
    fields: [
      {
        kind: 'select',
        path: 'subtitle.source_language',
        label: 'settings:spoken_language',
        options: [{ value: 'auto', label: 'common:detect' }, ...SOURCE_LANGUAGE_OPTIONS]
      },
      {
        kind: 'toggle',
        path: 'subtitle.show_original',
        label: 'common:show_source',
        help: 'settings:shows_the_source_above_the_translation'
      },
      {
        kind: 'number',
        path: 'subtitle.font_size_px',
        label: 'settings:text_size',
        min: 14,
        max: 48,
        step: 1,
        unit: 'px',
        slider: true
      },
      {
        kind: 'number',
        path: 'subtitle.chunk_seconds',
        label: 'settings:chunk_length',
        help: 'settings:short_lower_delay_long_more_context_and',
        min: 1,
        max: 10,
        step: 0.5,
        unit: 'settings:s',
        slider: true
      },
      {
        kind: 'number',
        path: 'subtitle.overlap_seconds',
        label: 'settings:overlap',
        help: 'settings:prevents_words_from_being_lost_at_chunk',
        min: 0,
        max: 2,
        step: 0.1,
        unit: 'settings:s',
        slider: true
      },
      {
        kind: 'toggle',
        path: 'subtitle.realtime_correction',
        label: 'settings:real_time_correction',
        help: 'settings:when_a_sentence_is_fully_recognized'
      },
      {
        kind: 'toggle',
        path: 'subtitle.batch_correction',
        label: 'settings:batch_correction',
        help: 'settings:retranslates_uncorrected_sentences_in_one'
      },
      {
        kind: 'toggle',
        path: 'subtitle.block_stock_phrases',
        label: 'settings:block_stock_misrecognitions',
        help: 'settings:removes_stock_phrases_that_speech'
      },
      {
        kind: 'phrases',
        path: 'subtitle.blocked_phrases',
        label: 'settings:blocked_phrases_additions',
        help: 'settings:additions_to_the_bundled_list_blocks_on',
        placeholder: 'settings:e_g_thank_you'
      }
    ]
  },
  {
    id: 'pin',
    title: 'common:continuous',
    description: 'settings:continuous_mode_of_window_translation',
    fields: [
      {
        kind: 'number',
        path: 'performance.scan_margin_px',
        label: 'settings:scan_margin',
        help: 'settings:extra_width_read_outside_the_window_frame',
        min: 0,
        max: 128,
        step: 4,
        unit: 'px',
        zeroLabel: 'settings:off',
        slider: true
      },
      {
        kind: 'number',
        path: 'performance.region_watcher_poll_ms',
        label: 'settings:check_interval',
        help: 'settings:how_often_the_screen_is_checked_for',
        min: 100,
        max: 2000,
        step: 50,
        unit: 'settings:ms',
        slider: true
      },
      {
        kind: 'toggle',
        path: 'overlay.show_in_screenshots',
        label: 'settings:show_in_captures',
        help: 'settings:show_translation_windows_in_screenshots_and'
      }
    ]
  },
  {
    id: 'clipboard',
    title: 'settings:auto_copy',
    description: 'settings:copies_translation_and_recognition_results',
    fields: [
      { kind: 'toggle', path: 'clipboard.auto_copy_translation', label: 'common:translated_text' },
      {
        kind: 'toggle',
        path: 'clipboard.auto_copy_source',
        label: 'common:source',
        help: 'settings:when_both_are_on_the_translation_is'
      },
      { kind: 'toggle', path: 'clipboard.auto_copy_qr', label: 'settings:qr_code_content' },
      { kind: 'toggle', path: 'clipboard.auto_copy_transcript', label: 'settings:audio_translation_transcript' }
    ]
  },
  {
    id: 'history',
    title: 'common:history',
    description: 'settings:history_is_stored_on_this_pc_only',
    fields: [
      { kind: 'toggle', path: 'storage.keep_history', label: 'settings:save_history' },
      {
        kind: 'number',
        path: 'storage.retention_days',
        label: 'settings:retention',
        help: 'settings:older_items_are_deleted_automatically_0_keep',
        min: 0,
        max: 3650,
        step: 1,
        unit: 'settings:days',
        zeroLabel: 'settings:forever'
      },
      {
        kind: 'number',
        path: 'storage.max_entries',
        label: 'settings:maximum_items',
        help: 'settings:beyond_this_the_oldest_items_are_deleted',
        min: 0,
        max: 1000000,
        step: 100,
        unit: 'settings:items',
        zeroLabel: 'settings:unlimited'
      }
    ]
  },
  {
    id: 'appearance',
    title: 'settings:appearance',
    description: 'settings:display_language_and_main_window_appearance',
    fields: [
      {
        kind: 'select',
        path: 'appearance.ui_language',
        label: 'settings:display_language',
        help: 'settings:language_of_all_windows_notifications_and',
        options: [
          { value: 'auto', label: 'settings:auto_os_language' },
          ...LOCALES.map((l) => ({ value: l.code, label: l.name }))
        ]
      },
      {
        kind: 'select',
        path: 'appearance.theme',
        label: 'settings:theme',
        help: 'settings:main_window_colors_system_setting_follows',
        options: [
          { value: 'system', label: 'settings:system_setting' },
          { value: 'light', label: 'settings:light' },
          { value: 'dark', label: 'settings:dark' }
        ]
      },
      {
        kind: 'select',
        path: 'appearance.text_size',
        label: 'settings:text_size',
        help: 'settings:text_size_of_the_history_settings_and',
        options: [
          { value: 'xsmall', label: 'settings:smallest_16px' },
          { value: 'small', label: 'settings:small_20px' },
          { value: 'medium', label: 'settings:medium_24px' },
          { value: 'large', label: 'settings:large_28px' },
          { value: 'xlarge', label: 'settings:largest_32px' }
        ]
      },
      {
        kind: 'select',
        path: 'appearance.density',
        label: 'settings:density',
        help: 'settings:row_and_section_spacing_in_settings_and',
        options: [
          { value: 'compact', label: 'settings:compact' },
          { value: 'normal', label: 'settings:normal' },
          { value: 'comfortable', label: 'settings:comfortable' }
        ]
      },
      {
        kind: 'color',
        path: 'appearance.accent',
        label: 'settings:accent_color',
        help: 'settings:base_color_of_the_main_window_hue'
      },
      {
        kind: 'toggle',
        path: 'appearance.reduced_motion',
        label: 'settings:reduce_motion',
        help: 'settings:turns_animations_off'
      }
    ]
  },
  {
    id: 'performance',
    title: 'settings:performance',
    description: 'settings:translation_speed_and_load',
    fields: [
      {
        kind: 'toggle',
        path: 'performance.use_gpu_if_available',
        label: 'settings:use_gpu',
        help: 'settings:speeds_up_translation_with_an_available_gpu'
      },
      {
        kind: 'number',
        path: 'performance.max_concurrent_tasks',
        label: 'settings:parallel_translations',
        help: 'settings:concurrent_per_paragraph_requests_when_an',
        min: 1,
        max: 8,
        step: 1,
        slider: true
      },
      {
        kind: 'toggle',
        path: 'storage.keep_translation_cache',
        label: 'settings:save_translation_cache',
        help: 'settings:shows_the_saved_translation_at_once_when'
      }
    ]
  },
  {
    id: 'startup',
    title: 'settings:startup',
    description: 'settings:behavior_at_os_sign_in',
    fields: [
      {
        kind: 'toggle',
        path: 'startup.launch_at_login',
        label: 'settings:launch_at_login',
        help: 'settings:starts_in_the_notification_area_when_you'
      }
    ]
  },
  {
    id: 'plugins',
    title: 'common:extensions',
    description: 'settings:add_and_enable_extensions_and_review_their',
    component: 'plugins'
  },
  {
    id: 'data',
    title: 'settings:data',
    description: 'settings:storage_locations_and_sizes_to_remove',
    component: 'data'
  },
  {
    id: 'advanced',
    title: 'settings:advanced',
    description: 'settings:settings_for_troubleshooting',
    fields: [
      {
        kind: 'select',
        path: 'privacy.log_level',
        label: 'settings:log_level',
        help: 'settings:change_only_when_investigating_a_problem',
        options: [
          { value: 'error', label: 'settings:error' },
          { value: 'warn', label: 'settings:warning' },
          { value: 'info', label: 'settings:normal' },
          { value: 'debug', label: 'settings:advanced' },
          { value: 'trace', label: 'settings:trace' }
        ]
      }
    ]
  }
];

export const GROUPS: Group[] = [
  { id: 'translation', title: 'common:translation', sections: ['translation', 'glossary', 'pin'] },
  // `#group`：英語表示は帯幅内の短縮名（Audio）
  { id: 'subtitle', title: 'settings:audio_group', sections: ['subtitle'] },
  { id: 'models', title: 'common:models', sections: ['models', 'model-assist', 'model-ocr', 'model-asr'] },
  { id: 'output', title: 'settings:history_copy', sections: ['history', 'clipboard'] },
  { id: 'hotkeys', title: 'settings:shortcuts', sections: ['hotkeys'] },
  { id: 'plugins', title: 'settings:extensions', sections: ['plugins'] },
  { id: 'system', title: 'settings:system', sections: ['appearance', 'performance', 'startup', 'data', 'advanced'] }
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
  // 原文と表示言語の訳文の双方を検索対象へ
  const words = [field.label, field.help ?? '', section.title, group];
  const hay = [...words, ...words.map((w) => t(w)), field.path].join(' ').toLowerCase();
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
  next = setPath(next, 'translation.glossary', compactGlossary(getPath(next, 'translation.glossary')));
  // 遮断文の空行も保存対象外（設定に項目の存在する場合のみ）。
  const phrases = getPath(next, 'subtitle.blocked_phrases');
  return Array.isArray(phrases)
    ? setPath(next, 'subtitle.blocked_phrases', phrases.filter((p: string) => p.trim() !== ''))
    : next;
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
  if (f?.kind === 'glossary' || f?.kind === 'phrases') return 600;
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
