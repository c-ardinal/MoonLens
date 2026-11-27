/**
 * 表示言語（UI 言語）。
 *
 * 文言は言語ファイル `locales/<言語コード>.json` で一覧管理（1 言語 1 ファイル、Rust 側
 * `src-tauri/src/i18n.rs` と共用）。ソースはキー ID（`区分:名称`）で参照：`t('common:save')`。
 * 表示言語のファイルに無いキーは日本語（`ja.json`）で代替表示。
 * 言語の追加：`locales/` へファイルを追加（`@language`：設定画面に表示する言語名）。
 * ソースの変更は不要。
 * 文言の追加手順：全言語ファイルへキーと文言を追加 → `t('区分:名称')` で参照
 * （検査：`npm run lint:i18n`）。
 *
 * 本モジュールは Svelte 非依存（Node のテストから利用可能）。画面の再描画は
 * `reactive.svelte.ts` が `setTracker` 経由で接続。
 */
/** 基準の言語：文言の代替表示と、文体検査の対象。 */
export const SOURCE_LOCALE = 'ja';
/** OS の言語に該当する言語ファイルが無い場合の表示言語。 */
const FALLBACK_LOCALE = 'en';
/** 言語ファイル中、言語名（当該言語での自称）を保持する項目。 */
const NAME_KEY = '@language';

/** 言語コード（`ja`、または `locales/` のファイル名）。 */
export type Locale = string;

type Dict = Record<string, string>;
const DICTS: Record<Locale, Dict> = Object.fromEntries(
  Object.entries(import.meta.glob<Dict>('../../../locales/*.json', { eager: true, import: 'default' })).map(
    ([file, dict]) => [file.replace(/^.*\/([^/]+)\.json$/, '$1'), dict]
  )
);

/** 表示言語の一覧（基準の言語、他は言語コード順）。name：当該言語での自称。 */
export const LOCALES: { code: Locale; name: string }[] = Object.keys(DICTS)
  .sort((a, b) => Number(b === SOURCE_LOCALE) - Number(a === SOURCE_LOCALE) || (a < b ? -1 : 1))
  .map((code) => ({ code, name: DICTS[code][NAME_KEY] ?? code }));

const known = (code: unknown): code is Locale => typeof code === 'string' && code in DICTS;
/** 前回の表示言語の控え（起動直後、設定の取得前の表示に使用）。 */
const CACHE_KEY = 'moonlens.locale';

function cached(): Locale {
  try {
    const code = localStorage.getItem(CACHE_KEY);
    return known(code) ? code : SOURCE_LOCALE;
  } catch {
    return SOURCE_LOCALE;
  }
}

let current: Locale = cached();
let track: () => unknown = () => 0;
const listeners = new Set<(l: Locale) => void>();

/**
 * 設定値（`appearance.ui_language`）と OS の言語から表示言語を決定。
 * auto・未知の値：OS の言語に該当する言語（無ければ英語）。
 */
export function resolveLocale(setting: unknown, systemLanguage?: string): Locale {
  if (known(setting)) return setting;
  const sys = systemLanguage ?? (typeof navigator === 'undefined' ? SOURCE_LOCALE : navigator.language);
  const primary = (sys ?? '').split(/[-_]/)[0].toLowerCase();
  if (known(primary)) return primary;
  return known(FALLBACK_LOCALE) ? FALLBACK_LOCALE : SOURCE_LOCALE;
}

/** 現在の表示言語。描画中の呼出しは言語切替時の再描画対象。 */
export function locale(): Locale {
  track();
  return current;
}

export function setLocale(next: Locale) {
  if (typeof document !== 'undefined') document.documentElement.lang = next;
  if (next === current) return;
  current = next;
  try {
    localStorage.setItem(CACHE_KEY, next);
  } catch {
    /* 保存不可の環境：次回起動時は設定取得後に反映 */
  }
  for (const f of listeners) f(next);
}

/** 設定（get_settings／settings-changed の値）から表示言語を適用。 */
export function applyLocale(settings: any) {
  setLocale(resolveLocale(settings?.appearance?.ui_language));
}

export function onLocaleChange(f: (l: Locale) => void): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}

/** 再描画の接続口（`reactive.svelte.ts` 専用）。 */
export function setTracker(f: () => unknown) {
  track = f;
}

/**
 * 文言の表示幅の概算（em 単位）：全角 1、半角 0.6。
 * 内容の幅へ追従する選択欄（ウィンドウ翻訳の言語欄等）に使用。
 */
export function textEm(s: string): number {
  let em = 0;
  for (const ch of s) em += /[\u0000-\u00ff]/.test(ch) ? 0.6 : 1;
  return Math.round(em * 10) / 10;
}

export type Vars = Record<string, string | number>;

const fill = (s: string, vars?: Vars) =>
  vars ? s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m)) : s;

/**
 * 文言の取得。`key` は言語ファイルのキー ID、`{name}` は `vars` の値へ置換。
 * 例：`t('history:count', { n: 3 })`。
 */
export function t(key: string, vars?: Vars): string {
  track();
  return fill(DICTS[current]?.[key] ?? DICTS[SOURCE_LOCALE]?.[key] ?? key, vars);
}

/** 基準の言語（日本語）の文言。表示言語に依存しない用途（LLM プロンプト等）に使用。 */
export function sourceText(key: string, vars?: Vars): string {
  return fill(DICTS[SOURCE_LOCALE]?.[key] ?? key, vars);
}
