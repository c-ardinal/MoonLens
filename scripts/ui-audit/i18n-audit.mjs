#!/usr/bin/env node
// 表示言語の監査：指定言語（既定：英語）の表示で全ウィンドウの全状態を巡回し、未翻訳の日本語文言（言語ファイルの原文）の残存を検出。
// 対象：表示中の本文、title／aria-label／placeholder／alt 属性。
// 対象外：利用者由来の文字列（履歴の本文、訳文等。言語ファイルの原文と不一致のため検出対象外）、
//         Canvas 描画（ホームの魔法陣・見出し。`--shots` の画像で目視確認）。
// 使用法：node scripts/ui-audit/i18n-audit.mjs [buildDir] [--lang en] [--json out.json] [--shots dir]
// 違反が有れば終了コード 1。
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { args, dict, collector, launch, serve, walkMain, walkWindows } from './harness.mjs';

const { opt, rest } = args();
const jsonOut = opt('--json');
const shots = opt('--shots');
const lang = opt('--lang') ?? 'en';
const DICT = dict(lang);
const { url, close } = await serve(rest[0] ?? 'build');

/** 辞書の原文：`{name}` は任意の文字列と一致。訳文が原文と同一の項目は対象外。 */
const KEYS = Object.keys(DICT).filter((k) => DICT[k] !== k && /[ぁ-ゖァ-ヺ一-鿿]/.test(k));

/** ページ内の計測：表示中の文言から、辞書の原文と一致するものを列挙。 */
const MEASURE = ([keys, lang]) => {
  const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const res = keys.map((k) => new RegExp('^' + k.split(/\{\w+\}/).map(esc).join('.+') + '$'));
  const leak = (s) => { const v = s.replace(/\s+/g, ' ').trim(); return v && res.some((r) => r.test(v)) ? v : null; };
  const where = (el) => {
    const parts = [];
    for (let e = el; e && e !== document.body && parts.length < 3; e = e.parentElement)
      parts.unshift(e.tagName.toLowerCase() + (e.classList[0] ? '.' + e.classList[0] : ''));
    return parts.join(' > ');
  };
  const out = [];
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const el = n.parentElement;
    if (!el || el.closest('script, style')) continue;
    const v = leak(n.nodeValue);
    if (v) out.push({ rule: 'untranslated', where: where(el), detail: v, value: 0 });
  }
  for (const el of document.body.querySelectorAll('[title], [aria-label], [placeholder], [alt]'))
    for (const a of ['title', 'aria-label', 'placeholder', 'alt']) {
      const v = el.hasAttribute(a) ? leak(el.getAttribute(a)) : null;
      if (v) out.push({ rule: 'untranslated', where: `${where(el)}[${a}]`, detail: v, value: 0 });
    }
  if (document.documentElement.lang !== lang) out.push({ rule: 'lang', where: 'html', detail: `lang="${document.documentElement.lang}"`, value: 0 });
  return out;
};

const found = collector();
const browser = await launch();
if (shots) mkdirSync(shots, { recursive: true });
const visit = async (pg, state) => {
  found.record(state, await pg.evaluate(MEASURE, [KEYS, lang]));
  if (shots) await pg.screenshot({ path: join(shots, `${state}.png`) });
};
await walkMain(browser, url, { lang }, visit);
await walkWindows(browser, url, { lang }, visit);
await browser.close(); close();
process.exit(found.report(jsonOut) ? 1 : 0);
