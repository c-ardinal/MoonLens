#!/usr/bin/env node
/**
 * 表示言語の検査：日本語文言と言語ファイル（locales/<言語コード>.json）の整合を確認。
 * 言語ファイルは画面側（src/）と Rust 側（src-tauri/src/）の共用。全言語ファイルが検査対象。
 *
 *   node scripts/i18n-lint.mjs            検査（違反時は終了コード 1）
 *   node scripts/i18n-lint.mjs --fix      全言語ファイルを整列し、未使用の項目を削除
 *   node scripts/i18n-lint.mjs --json     違反一覧を JSON で出力
 *
 * 規則
 *   raw       Svelte のマークアップ（本文・属性値）へ日本語を直接記述不可。`{t('…')}` で包囲。
 *   template  日本語を含むテンプレート文字列へ `${}` を埋込み不可。`t('{n} 件', { n })` へ変更。
 *   missing   日本語の文字列リテラル（Rust：`tr!`・`Text::new`・`text` の原文）は
 *             全言語ファイルへ訳文の登録が必須。
 *   unused    言語ファイルの項目はソース中の文字列リテラルと一致が必須。
 *   value     訳文は空欄・日本語混入不可、`{name}` は原文と同一。`@language`（言語名）は必須。
 *   order     言語ファイルの項目は符号位置順（並行作業の競合を抑制）。
 *   args      Rust の `tr!` は原文の `{name}` と引数名（`name = 値`）が一致。
 * 例外：直前行または同一行の `i18n-ignore`（言語名の自称・判定用の文字集合等）、SKIP、PENDING。
 */
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
/** 言語ファイルの置場（1 言語 1 ファイル、ファイル名：言語コード）。 */
export const LOCALES_DIR = 'locales';
/** 言語名（当該言語での自称）の項目。設定画面の選択肢に表示。 */
export const NAME_KEY = '@language';

/** 検査対象外：テスト・辞書本体・ルーン字形・LLM プロンプト。 */
const SKIP = [/\.test\.[a-z]+$/, /^src\/lib\/pixel\/glyphs/, /prompt/i];

/** 英語化の未着手ファイル（着手時に削除。空が完了状態）。 */
export const PENDING = [];

/** 日本語の判定：仮名・漢字・全角記号。 */
export const JA = /[　-〿ぁ-ゖァ-ヺー一-鿿！-ﾟ]/;
/** 訳文への混入検査：仮名・漢字のみ（全角記号は検出対象外）。 */
const KANA_KANJI = /[ぁ-ゖァ-ヺ一-鿿]/;

const lineOf = (text, pos) => text.slice(0, pos).split('\n').length;

/**
 * Svelte ファイルの走査。
 * 戻り値：literals（辞書照合の対象）、issues（raw／template）。
 */
export function scanSvelte(text) {
  const { parse } = require('svelte/compiler');
  const ast = parse(text, { modern: true });
  const literals = [];
  const issues = [];
  const walk = (node, inScript) => {
    if (!node || typeof node !== 'object') return;
    if (Array.isArray(node)) return node.forEach((n) => walk(n, inScript));
    switch (node.type) {
      case 'Comment':
      case 'StyleSheet':
        return;
      case 'Text':
        if (JA.test(node.data)) issues.push({ rule: 'raw', pos: node.start + node.raw.search(/\S/), text: node.data.trim() });
        return;
      case 'Literal':
        if (typeof node.value === 'string' && JA.test(node.value)) literals.push({ pos: node.start, text: node.value });
        return;
      case 'TemplateLiteral':
        scanTemplate(node.quasis.map((q) => q.value.cooked ?? ''), node.start, literals, issues);
        return walk(node.expressions, inScript);
    }
    for (const [k, v] of Object.entries(node)) {
      if (k === 'css' || k === 'loc' || k === 'comments' || k === 'leadingComments' || k === 'trailingComments') continue;
      if (v && typeof v === 'object') walk(v, inScript);
    }
  };
  walk(ast.fragment);
  walk(ast.instance);
  walk(ast.module);
  return { literals, issues };
}

function scanTemplate(parts, pos, literals, issues) {
  if (!parts.some((p) => JA.test(p))) return;
  if (parts.length === 1) literals.push({ pos, text: parts[0] });
  else issues.push({ rule: 'template', pos, text: parts.join('${…}') });
}

/** TypeScript ファイルの走査。 */
export function scanTs(text, file = 'a.ts') {
  const ts = require('typescript');
  const src = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const literals = [];
  const issues = [];
  const walk = (node) => {
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
      if (JA.test(node.text)) literals.push({ pos: node.getStart(), text: node.text });
    } else if (ts.isTemplateExpression(node)) {
      scanTemplate([node.head.text, ...node.templateSpans.map((s) => s.literal.text)], node.getStart(), literals, issues);
    }
    ts.forEachChild(node, walk);
  };
  walk(src);
  return { literals, issues };
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
 * Rust ファイルの走査：`tr!("原文", name = 値…)`・`Text::new("原文")`・`text("原文")`。
 * 原文は言語ファイルの検索キー（日本語を含まない原文も対象）。
 */
export function scanRust(text) {
  const literals = [];
  const issues = [];
  const call = /(?<![A-Za-z0-9_])(tr!|Text::new|text)\(\s*"((?:[^"\\]|\\[\s\S])*)"/g;
  for (const m of text.matchAll(call)) {
    const pos = m.index;
    // コメント行（文書中の使用例）は対象外
    if (/^\s*\/\//.test(text.slice(text.lastIndexOf('\n', pos) + 1, pos))) continue;
    const key = rustString(m[2]);
    literals.push({ pos, text: key });
    if (m[1] !== 'tr!') continue;
    const names = callArgs(text, m.index + m[1].length + 1)
      .slice(1)
      .map((a) => /^([A-Za-z_]\w*)\s*=(?!=)/.exec(a)?.[1] ?? '?');
    if ([...names].sort().join(',') !== placeholders(key)) issues.push({ rule: 'args', pos, text: key });
  }
  return { literals, issues };
}

/** 1 ファイルの走査結果へ行番号を付与し、`i18n-ignore` 指定行を除外。 */
export function scanFile(text, rel) {
  const { literals, issues } = rel.endsWith('.svelte') ? scanSvelte(text) : rel.endsWith('.rs') ? scanRust(text) : scanTs(text, rel);
  const lines = text.split('\n');
  const keep = (x) => {
    x.line = lineOf(text, x.pos);
    // 直前行の指定はコメントのみの行に限定（行末コメントの指定は同一行のみに適用）
    const prev = (lines[x.line - 2] ?? '').trim();
    const prevIgnores = /^(\/\/|\/\*|\*|<!--)/.test(prev) && prev.includes('i18n-ignore');
    return !lines[x.line - 1].includes('i18n-ignore') && !prevIgnores;
  };
  return { literals: literals.filter(keep), issues: issues.filter(keep) };
}

const placeholders = (s) => [...new Set([...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]))].sort().join(',');
/** 符号位置順の比較。 */
export const byCodePoint = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

/** 辞書単体の検査（value／order）。 */
export function lintDict(dict) {
  const out = [];
  if (typeof dict[NAME_KEY] !== 'string' || !dict[NAME_KEY].trim()) out.push({ rule: 'value', text: NAME_KEY, hint: '言語名（当該言語での自称）が未登録' });
  const keys = Object.keys(dict).filter((k) => k !== NAME_KEY);
  keys.forEach((k, i) => {
    const v = dict[k];
    if (typeof v !== 'string' || !v.trim()) out.push({ rule: 'value', text: k, hint: '訳文が空欄' });
    else if (KANA_KANJI.test(v)) out.push({ rule: 'value', text: k, hint: `訳文へ日本語が混入：${v}` });
    else if (placeholders(k) !== placeholders(v)) out.push({ rule: 'value', text: k, hint: `{name} が原文と不一致：${v}` });
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

/** 言語ファイルの一覧（言語コード → 相対パス）。 */
export function localeFiles() {
  return fs
    .readdirSync(path.join(ROOT, LOCALES_DIR))
    .filter((f) => f.endsWith('.json'))
    .sort()
    .map((f) => `${LOCALES_DIR}/${f}`);
}

/** 全ソースの走査。戻り値：違反以外の走査結果（原文の出現箇所、raw／template／args の違反）。 */
export function scanProject() {
  const files = [];
  walkFiles(path.join(ROOT, 'src'), files);
  walkFiles(path.join(ROOT, 'src-tauri/src'), files);
  const literals = [];
  const found = [];
  const HINT = {
    raw: "`{t('…')}` で包囲",
    template: "`t('… {name} …', { name })` へ変更",
    args: '`tr!` の引数名を原文の {name} と一致'
  };
  for (const rel of files) {
    if (PENDING.includes(rel)) continue;
    const scanned = scanFile(fs.readFileSync(path.join(ROOT, rel), 'utf8'), rel);
    for (const l of scanned.literals) literals.push({ file: rel, line: l.line, text: l.text });
    for (const i of scanned.issues) found.push({ file: rel, line: i.line, rule: i.rule, text: i.text, hint: HINT[i.rule] });
  }
  return { literals, found, used: new Set(literals.map((l) => l.text)) };
}

/** 言語ファイル 1 件と走査結果の照合（missing／unused／value／order）。 */
export function lintLocale(file, dict, scan) {
  const found = [];
  for (const l of scan.literals) {
    if (!(l.text in dict)) found.push({ file: l.file, line: l.line, rule: 'missing', text: l.text, hint: `${file} へ訳文を追加` });
  }
  for (const k of Object.keys(dict)) {
    if (k !== NAME_KEY && !scan.used.has(k)) found.push({ file, line: 0, rule: 'unused', text: k, hint: 'ソース中に該当文言なし（--fix で削除）' });
  }
  for (const v of lintDict(dict)) found.push({ file, line: 0, ...v });
  return found;
}

function main() {
  const scan = scanProject();
  const found = [...scan.found];
  const files = localeFiles();
  if (files.length === 0) found.push({ file: LOCALES_DIR, line: 0, rule: 'missing', text: '*.json', hint: '言語ファイルなし' });
  for (const rel of files) {
    const file = path.join(ROOT, rel);
    let dict = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (process.argv.includes('--fix')) {
      const keys = Object.keys(dict).filter((k) => k !== NAME_KEY && scan.used.has(k)).sort(byCodePoint);
      dict = Object.fromEntries([NAME_KEY, ...keys].filter((k) => k in dict).map((k) => [k, dict[k]]));
      fs.writeFileSync(file, JSON.stringify(dict, null, 2) + '\n');
    }
    found.push(...lintLocale(rel, dict, scan));
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
