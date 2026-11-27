#!/usr/bin/env node
// 画面超過の監査：メインウィンドウの全状態を文字サイズ 5 段 × ウィンドウ寸法で表示し、
// 横スクロール・内容の欠落・ウィンドウ外への突出・要素の重複を計測。
// 使用法：node scripts/ui-audit/overflow-audit.mjs [buildDir] [--json out.json] [--sizes medium,...] [--langs ja,en] [--shots dir]
// 違反が有れば終了コード 1。
import { mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { HERE, TEXT_SIZES, args, collector, launch, serve, walkMain } from './harness.mjs';

const { opt, rest } = args();
const jsonOut = opt('--json');
const shots = opt('--shots');
const SIZES = (opt('--sizes') ?? TEXT_SIZES.join(',')).split(',');
/** 表示言語：英語は日本語より文言が長く、超過の発生箇所が相違。 */
const LANGS = (opt('--langs') ?? 'ja').split(',');
/** ウィンドウ寸法：既定・最小（tauri.conf.json の minWidth／minHeight）・フル HD。 */
const VIEWPORTS = [[1280, 800], [1120, 700], [1920, 1080]];
const { url, close } = await serve(rest[0] ?? 'build');

const MEASURE = readFileSync(join(HERE, 'overflow.js'), 'utf8');
const found = collector();
const browser = await launch();
if (shots) mkdirSync(shots, { recursive: true });
for (const lang of LANGS) for (const ts of SIZES) for (const [width, height] of VIEWPORTS)
  await walkMain(browser, url, { textSize: ts, width, height, lang }, async (pg, name) => {
    const state = `${name}@${ts}@${width}x${height}${lang === 'ja' ? '' : `@${lang}`}`;
    found.record(state, await pg.evaluate(MEASURE));
    if (shots) await pg.screenshot({ path: join(shots, `${state}.png`) });
  });
await browser.close(); close();
process.exit(found.report(jsonOut) ? 1 : 0);
