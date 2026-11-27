#!/usr/bin/env node
/**
 * 表示言語の検査：ソース中のキー ID と言語ファイル（locales/<言語コード>.json）の整合を確認。
 * 言語ファイルは画面側（src/）と Rust 側（src-tauri/src/）の共用。全言語ファイルが検査対象。
 *
 *   node scripts/i18n-lint.mjs            検査（違反時は終了コード 1）
 *   node scripts/i18n-lint.mjs --fix      全言語ファイルを整列し、未使用の項目を削除
 *   node scripts/i18n-lint.mjs --json     違反一覧を JSON で出力
 *
 * 規則
 *   raw       Svelte のマークアップ（本文・属性値）へ日本語を直接記述不可。`{t('区分:名称')}` で参照。
 *   literal   ソースの文字列リテラル・テンプレート文字列へ日本語を記述不可。言語ファイルへ登録し、キー ID で参照。
 *   missing   ソース中のキー ID（`区分:名称` 形式の文字列リテラル。Rust：`tr!`・`Text::new`・`text` の
 *             第 1 引数）は ja.json への登録が必須。ja.json の項目は全言語ファイルへの登録が必須。
 *   unused    言語ファイルの項目はソース中のキー ID と一致が必須。
 *   value     文言は空欄不可、`{name}` は ja.json と同一。日本語以外の言語ファイルは日本語の混入不可。
 *             キーは `区分:名称` 形式。`@language`（言語名）は必須。
 *   order     言語ファイルの項目は符号位置順（並行作業の競合を抑制）。
 *   args      Rust の `tr!` は文言の `{name}` と引数名（`name = 値`）が一致。
 * 例外：直前行または同一行の `i18n-ignore`（言語名の自称・判定用の文字集合・LLM プロンプト等）、SKIP。
 */
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
/** 言語ファイルの置場（1 言語 1 ファイル、ファイル名：言語コード）。 */
export const LOCALES_DIR = 'locales';
/** 基準の言語：キーの一覧と `{name}` の基準。 */
export const SOURCE = 'ja';
/** 言語名（当該言語での自称）の項目。設定画面の選択肢に表示。 */
export const NAME_KEY = '@language';
/** キー ID の形式：`区分:名称`。 */
export const KEY = /^[a-z][a-z0-9-]*:[a-z0-9_]+$/;

/** 検査対象外：テスト・ルーン字形・LLM プロンプト。 */
const SKIP = [/\.test\.[a-z]+$/, /^src\/lib\/pixel\/glyphs/, /prompt/i];

/** 日本語の判定：仮名・漢字・全角記号。 */
export const JA = /[　-〿ぁ-ゖァ-ヺー一-鿿！-ﾟ]/;
/** 他言語の文言への混入検査：仮名・漢字のみ（全角記号は検出対象外）。 */
const KANA_KANJI = /[ぁ-ゖァ-ヺ一-鿿]/;

const lineOf = (text, pos) => text.slice(0, pos).split('\n').length;

/** 文字列リテラルの分類：キー ID は keys、日本語は issues（literal）。 */
function classify(value, pos, keys, issues) {
  if (KEY.test(value)) keys.push({ pos, text: value });
  else if (JA.test(value)) issues.push({ rule: 'literal', pos, text: value });
}

/**
 * Svelte ファイルの走査。
 * 戻り値：keys（キー ID の出現箇所）、issues（raw／literal）。
 */
export function scanSvelte(text) {
  const { parse } = require('svelte/compiler');
  const ast = parse(text, { modern: true });
  const keys = [];
  const issues = [];
  const walk = (node) => {
    if (!node || typeof node !== 'object') return;
    if (Array.isArray(node)) return node.forEach(walk);
    switch (node.type) {
      case 'Comment':
      case 'StyleSheet':
        return;
      case 'Text':
        if (JA.test(node.data)) issues.push({ rule: 'raw', pos: node.start + node.raw.search(/\S/), text: node.data.trim() });
        return;
      case 'Literal':
        if (typeof node.value === 'string') classify(node.value, node.start, keys, issues);
        return;
      case 'TemplateLiteral':
        scanTemplate(node.quasis.map((q) => q.value.cooked ?? ''), node.start, keys, issues);
        return walk(node.expressions);
    }
    for (const [k, v] of Object.entries(node)) {
      if (k === 'css' || k === 'loc' || k === 'comments' || k === 'leadingComments' || k === 'trailingComments') continue;
      if (v && typeof v === 'object') walk(v);
    }
  };
  walk(ast.fragment);
  walk(ast.instance);
  walk(ast.module);
  return { keys, issues };
}

function scanTemplate(parts, pos, keys, issues) {
  if (parts.length === 1) classify(parts[0], pos, keys, issues);
  else if (parts.some((p) => JA.test(p))) issues.push({ rule: 'literal', pos, text: parts.join('${…}') });
}

/** TypeScript ファイルの走査。 */
export function scanTs(text, file = 'a.ts') {
  const ts = require('typescript');
  const src = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const keys = [];
  const issues = [];
  const walk = (node) => {
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
      classify(node.text, node.getStart(), keys, issues);
    } else if (ts.isTemplateExpression(node)) {
      scanTemplate([node.head.text, ...node.templateSpans.map((s) => s.literal.text)], node.getStart(), keys, issues);
    }
    ts.forEachChild(node, walk);
  };
  walk(src);
  return { keys, issues };
}

/** Rust の文字列リテラルの復号（エスケープ・行継続）。 */
function rustString(raw) {
  return raw.replace(/\\(\r?\n\s*|u\{([0-9a-fA-F]+)\}|.)/g, (_, e, hex) =>
    hex ? String.fromCodePoint(parseInt(hex, 16)) : e === 'n' ? '\n' : e === 't' ? '\t' : /\s/.test(e[0]) ? '' : e
  );
}

/** 呼出しの終端（対応する閉じ括弧）までを取得し、最上位のカンマで分割。 */
function callArgs(text, start) {
  const args = [];
  let depth = 1;
  let from = start;
  let i = start;
  for (; i < text.length && depth > 0; i++) {
    const c = text[i];
    if (c === '"') {
      for (i++; i < text.length && text[i] !== '"'; i++) if (text[i] === '\\') i++;
    } else if ('([{'.includes(c)) depth++;
    else if (')]}'.includes(c)) depth--;
    else if (c === ',' && depth === 1) {
      args.push(text.slice(from, i));
      from = i + 1;
    }
  }
  args.push(text.slice(from, i - 1));
  return args.map((a) => a.trim()).filter(Boolean);
}

/**
 * Rust ファイルの走査：`tr!("区分:名称", name = 値…)`・`Text::new("区分:名称")`・`text("区分:名称")`。
 * 第 1 引数は形式を問わずキー ID として照合。keys の args：`tr!` の引数名（他は null）。
 */
export function scanRust(text) {
  const keys = [];
  const call = /(?<![A-Za-z0-9_])(tr!|Text::new|text)\(\s*"((?:[^"\\]|\\[\s\S])*)"/g;
  for (const m of text.matchAll(call)) {
    const pos = m.index;
    // コメント行（文書中の使用例）は対象外
    if (/^\s*\/\//.test(text.slice(text.lastIndexOf('\n', pos) + 1, pos))) continue;
    const args =
      m[1] === 'tr!'
        ? callArgs(text, m.index + m[1].length + 1)
            .slice(1)
            .map((a) => /^([A-Za-z_]\w*)\s*=(?!=)/.exec(a)?.[1] ?? '?')
        : null;
    keys.push({ pos, text: rustString(m[2]), args });
  }
  return { keys, issues: [] };
}

/** 1 ファイルの走査結果へ行番号を付与し、`i18n-ignore` 指定行を除外。 */
export function scanFile(text, rel) {
  const { keys, issues } = rel.endsWith('.svelte') ? scanSvelte(text) : rel.endsWith('.rs') ? scanRust(text) : scanTs(text, rel);
  const lines = text.split('\n');
  const keep = (x) => {
    x.line = lineOf(text, x.pos);
    // 直前行の指定はコメントのみの行に限定（行末コメントの指定は同一行のみに適用）
    const prev = (lines[x.line - 2] ?? '').trim();
    const prevIgnores = /^(\/\/|\/\*|\*|<!--)/.test(prev) && prev.includes('i18n-ignore');
    return !lines[x.line - 1].includes('i18n-ignore') && !prevIgnores;
  };
  return { keys: keys.filter(keep), issues: issues.filter(keep) };
}

const placeholders = (s) => [...new Set([...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]))].sort().join(',');
/** 符号位置順の比較。 */
export const byCodePoint = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

/**
 * 言語ファイル単体の検査（value／order）。
 * source：基準の言語の辞書（`{name}` の照合用。基準の言語自体の検査では省略）。
 */
export function lintDict(dict, source = null) {
  const out = [];
  if (typeof dict[NAME_KEY] !== 'string' || !dict[NAME_KEY].trim()) out.push({ rule: 'value', text: NAME_KEY, hint: '言語名（当該言語での自称）が未登録' });
  const keys = Object.keys(dict).filter((k) => k !== NAME_KEY);
  keys.forEach((k, i) => {
    const v = dict[k];
    if (!KEY.test(k)) out.push({ rule: 'value', text: k, hint: 'キーは `区分:名称` 形式（英小文字・数字・`-`・`_`）' });
    if (typeof v !== 'string' || !v.trim()) out.push({ rule: 'value', text: k, hint: '文言が空欄' });
    else if (source && KANA_KANJI.test(v)) out.push({ rule: 'value', text: k, hint: `日本語が混入：${v}` });
    else if (source && typeof source[k] === 'string' && placeholders(source[k]) !== placeholders(v)) out.push({ rule: 'value', text: k, hint: `{name} が ${SOURCE}.json と不一致：${v}` });
    if (i > 0 && byCodePoint(keys[i - 1], k) > 0) out.push({ rule: 'order', text: k, hint: '符号位置順へ整列（--fix で自動整列）' });
  });
  return out;
}

function walkFiles(p, out) {
  const rel = path.relative(ROOT, p).split(path.sep).join('/');
  const st = fs.statSync(p);
  if (st.isDirectory()) for (const f of fs.readdirSync(p).sort()) walkFiles(path.join(p, f), out);
  else if (/\.(svelte|ts|rs)$/.test(rel) && !rel.endsWith('.d.ts') && !SKIP.some((s) => s.test(rel))) out.push(rel);
}

/** 言語ファイルの一覧（相対パス、基準の言語が先頭）。 */
export function localeFiles() {
  return fs
    .readdirSync(path.join(ROOT, LOCALES_DIR))
    .filter((f) => f.endsWith('.json'))
    .sort((a, b) => (b === `${SOURCE}.json`) - (a === `${SOURCE}.json`) || byCodePoint(a, b))
    .map((f) => `${LOCALES_DIR}/${f}`);
}

/** 全ソースの走査。戻り値：キー ID の出現箇所（keys）、ソース単体の違反（found：raw／literal）。 */
export function scanProject() {
  const files = [];
  walkFiles(path.join(ROOT, 'src'), files);
  walkFiles(path.join(ROOT, 'src-tauri/src'), files);
  const keys = [];
  const found = [];
  const HINT = {
    raw: "言語ファイルへ登録し、`{t('区分:名称')}` で参照",
    literal: '言語ファイルへ登録し、キー ID で参照'
  };
  for (const rel of files) {
    const scanned = scanFile(fs.readFileSync(path.join(ROOT, rel), 'utf8'), rel);
    for (const k of scanned.keys) keys.push({ file: rel, line: k.line, text: k.text, args: k.args ?? null });
    for (const i of scanned.issues) found.push({ file: rel, line: i.line, rule: i.rule, text: i.text, hint: HINT[i.rule] });
  }
  return { keys, found };
}

/** ソース中のキー ID と基準の言語の照合（missing／args）。 */
export function lintKeys(keys, source) {
  const found = [];
  for (const k of keys) {
    if (!(k.text in source)) found.push({ file: k.file, line: k.line, rule: 'missing', text: k.text, hint: `${LOCALES_DIR}/${SOURCE}.json へ文言を登録` });
    else if (k.args && [...k.args].sort().join(',') !== placeholders(source[k.text]))
      found.push({ file: k.file, line: k.line, rule: 'args', text: k.text, hint: `\`tr!\` の引数名を文言の {name} と一致：${source[k.text]}` });
  }
  return found;
}

/**
 * 言語ファイル 1 件の検査（missing／unused／value／order）。
 * used：ソース中のキー ID の集合。source：基準の言語の辞書（基準の言語自体の検査では null）。
 */
export function lintLocale(file, dict, used, source = null) {
  const found = [];
  if (source) {
    for (const k of Object.keys(source)) {
      if (!(k in dict)) found.push({ file, line: 0, rule: 'missing', text: k, hint: `${SOURCE}.json の項目が未登録（文言：${source[k]}）` });
    }
  }
  for (const k of Object.keys(dict)) {
    if (k !== NAME_KEY && !used.has(k)) found.push({ file, line: 0, rule: 'unused', text: k, hint: 'ソース中に該当キーなし（--fix で削除）' });
  }
  for (const v of lintDict(dict, source)) found.push({ file, line: 0, ...v });
  return found;
}

function main() {
  const scan = scanProject();
  const used = new Set(scan.keys.map((k) => k.text));
  const found = [...scan.found];
  const files = localeFiles();
  let source = null;
  if (files[0] !== `${LOCALES_DIR}/${SOURCE}.json`) {
    found.push({ file: LOCALES_DIR, line: 0, rule: 'missing', text: `${SOURCE}.json`, hint: '基準の言語ファイルなし' });
  }
  for (const rel of files) {
    const file = path.join(ROOT, rel);
    let dict = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (process.argv.includes('--fix')) {
      const keys = Object.keys(dict).filter((k) => k !== NAME_KEY && used.has(k)).sort(byCodePoint);
      dict = Object.fromEntries([NAME_KEY, ...keys].filter((k) => k in dict).map((k) => [k, dict[k]]));
      fs.writeFileSync(file, JSON.stringify(dict, null, 2) + '\n');
    }
    const isSource = rel === `${LOCALES_DIR}/${SOURCE}.json`;
    if (isSource) {
      source = dict;
      found.push(...lintKeys(scan.keys, dict));
    }
    found.push(...lintLocale(rel, dict, used, isSource ? null : source));
  }
  if (process.argv.includes('--json')) {
    console.log(JSON.stringify(found, null, 2));
  } else {
    for (const f of found) console.log(`${f.file}:${f.line}  [${f.rule}] 「${f.text}」 ${f.hint}`);
    console.log(found.length === 0 ? '表示言語検査：違反なし' : `表示言語検査：違反 ${found.length} 件`);
  }
  process.exitCode = found.length === 0 ? 0 : 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
