#!/usr/bin/env node
/**
 * 文体検査：UI 文言・メッセージ中の和語を検出（規約：CLAUDE.md「文体」）。
 *
 *   node scripts/copy-lint.mjs            検査（違反時は終了コード 1）
 *   node scripts/copy-lint.mjs --json     違反一覧を JSON で出力
 *
 * 検査内容
 *   1. 禁止語（BANNED）：コメントを含む全文が対象。
 *   2. 送り仮名を伴う語（漢字＋平仮名の活用語尾）：コメント以外が対象。
 *   3. 平仮名の和語（HIRAGANA）：コメント以外が対象。
 * 例外は ALLOW（語単位）・SKIP（ファイル単位）・直前行の `copy-lint-ignore`（行単位）で宣言。
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

/** 検査対象（リポジトリ基準）。 */
const TARGETS = ['src', 'src-tauri/src', 'src-tauri/tauri.conf.json', 'static', 'plugin-sdk', 'locales/ja.json'];
const EXTENSIONS = /\.(svelte|ts|js|mjs|cjs|rs|css|html|json)$/;
const IGNORED_DIRS = /(^|\/)(node_modules|target|build|dist|\.svelte-kit|gen)(\/|$)/;

/** 検査対象外：テスト（テストデータ）・LLM プロンプト・翻訳用の語彙データ。 */
const SKIP = [/\.test\.[a-z]+$/, /(^|\/)tests?\//, /prompt/i, /^src\/lib\/pixel\/glyphs/];

/** 1. 禁止語：単独使用の和語。熟語の一部（画面・背面・側面等）は対象外。 */
const BANNED = [
  { re: /窓/g, hint: '「ウィンドウ」「欄」「ダイアログ」へ置換' },
  { re: /(?<![一-龥々])面(?![一-龥々])/g, hint: '「側」等へ置換' }
];

/** 2. 漢字に続く平仮名のうち、助詞・助動詞・サ変活用として許容する先頭。 */
const PARTICLE =
  /^(の|を|に|へ|と|が|は|で|も|や|か|な|だ|です|から|より|まで|のみ|など|なし|あり|および|または|かつ|ほど|ずつ)/;
/** サ変活用（2 字以上の熟語に限定）。 */
const SURU = /^(する|し|され|させ|せず|せよ|すれ|さ|せ)/;
/** 1 字でサ変動詞を構成する漢字（関する・対する等）。 */
const SINGLE_SURU = new Set('関対適有要接属反発達略課比期訳減増害制配称'.split(''));

/** 3. 平仮名の和語。 */
const HIRAGANA = [
  { re: /でき(る|ます|ません|ない|た)/g, hint: '「可能」「不可」へ置換' },
  { re: /(して|て)ください/g, hint: '体言止めへ変更' },
  { re: /(?<=[一-龥々ァ-ヶー])ごと/g, hint: '「単位」「毎」へ置換' },
  { re: /すべて/g, hint: '「全」へ置換' },
  { re: /(この|その|あの)(?=[一-龥々ァ-ヶ])/g, hint: '「本」「当該」へ置換' },
  { re: /ほか/g, hint: '「他」へ置換' }
];

/** 許容語（OS 慣例の 1 語ボタン等）。 */
const ALLOW = new Set(['閉じる', '開く']);

function walk(p, out) {
  const rel = path.relative(ROOT, p).split(path.sep).join('/');
  if (IGNORED_DIRS.test(rel)) return;
  const st = fs.statSync(p);
  if (st.isDirectory()) for (const f of fs.readdirSync(p).sort()) walk(path.join(p, f), out);
  else if (EXTENSIONS.test(rel) && !SKIP.some((s) => s.test(rel))) out.push(rel);
}

/** コメントを同数の空白へ置換（行番号・桁位置を維持）。 */
export function stripComments(text, rel) {
  const blank = (m) => m.replace(/[^\n]/g, ' ');
  let t = text;
  if (/\.(svelte|html)$/.test(rel)) t = t.replace(/<!--[\s\S]*?-->/g, blank);
  if (!/\.(json|html)$/.test(rel)) {
    t = t.replace(/\/\*[\s\S]*?\*\//g, blank);
    if (!/\.css$/.test(rel)) t = t.replace(/(?<![:'"`\\])\/\/[^\n]*/g, blank);
  }
  // Rust：ファイル末尾のテストモジュール（テストデータ）
  if (/\.rs$/.test(rel)) {
    const i = t.search(/^#\[cfg\(test\)\]\s*\n\s*mod /m);
    if (i >= 0) t = t.slice(0, i) + blank(t.slice(i));
  }
  return t;
}

/** 送り仮名を伴う語の検出。 */
function okurigana(line) {
  const hits = [];
  for (const m of line.matchAll(/([一-龥々]+)([ぁ-ん]+)/g)) {
    const [, kanji, kana] = m;
    if (PARTICLE.test(kana)) continue;
    if (SURU.test(kana) && (kanji.length >= 2 || SINGLE_SURU.has(kanji))) continue;
    const word = kanji.slice(-1) + kana;
    if ([...ALLOW].some((a) => (kanji + kana).includes(a))) continue;
    hits.push({ index: m.index + kanji.length - 1, word, hint: '熟語・カタカナ語へ置換' });
  }
  return hits;
}

export function lintText(text, rel) {
  const out = [];
  const full = text.split('\n');
  const code = stripComments(text, rel).split('\n');
  const add = (i, col, word, hint) => out.push({ file: rel, line: i + 1, col: col + 1, word, hint });
  full.forEach((line, i) => {
    for (const b of BANNED) for (const m of line.matchAll(b.re)) add(i, m.index, m[0], b.hint);
  });
  code.forEach((line, i) => {
    if (!/[ぁ-ん]/.test(line) || (i > 0 && full[i - 1].includes('copy-lint-ignore'))) return;
    for (const h of okurigana(line)) add(i, h.index, h.word, h.hint);
    for (const b of HIRAGANA) for (const m of line.matchAll(b.re)) add(i, m.index, m[0], b.hint);
  });
  return out;
}

function main() {
  const files = [];
  for (const t of TARGETS) {
    const p = path.join(ROOT, t);
    if (fs.existsSync(p)) walk(p, files);
  }
  const found = files.flatMap((rel) => lintText(fs.readFileSync(path.join(ROOT, rel), 'utf8'), rel));
  if (process.argv.includes('--json')) {
    console.log(JSON.stringify(found, null, 2));
  } else {
    for (const f of found) console.log(`${f.file}:${f.line}:${f.col}  「${f.word}」 ${f.hint}`);
    console.log(found.length === 0 ? '文体検査：違反なし' : `文体検査：違反 ${found.length} 件`);
  }
  process.exit(found.length === 0 ? 0 : 1);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
