#!/usr/bin/env node
// 4px 格子の監査：`vite build` の出力を IPC モック付きで表示し、余白・項目間隔・垂直中央を計測。
// 使用法：node scripts/ui-audit/grid-audit.mjs [buildDir] [--json out.json] [--only main|lens|subtitle|overlay] [--sizes medium,...]
// 違反が有れば終了コード 1。
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { HERE, TEXT_SIZES, args, collector, launch, serve, walkMain, walkWindows } from './harness.mjs';

const { opt, rest } = args();
const jsonOut = opt('--json');
const only = opt('--only');
const SIZES = (opt('--sizes') ?? TEXT_SIZES.join(',')).split(',');
const { url, close } = await serve(rest[0] ?? 'build');

const MEASURE = readFileSync(join(HERE, 'measure.js'), 'utf8');
const found = collector();
const browser = await launch();
const measure = async (pg, state) => found.record(state, await pg.evaluate(MEASURE));

if (!only || only === 'main') for (const ts of SIZES) await walkMain(browser, url, { textSize: ts }, (pg, name) => measure(pg, `${name}@${ts}`));
if (only !== 'main') await walkWindows(browser, url, { only }, measure);
await browser.close(); close();
process.exit(found.report(jsonOut) ? 1 : 0);
