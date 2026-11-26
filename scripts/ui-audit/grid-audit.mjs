#!/usr/bin/env node
// 4px 格子の監査：`vite build` の出力を IPC モック付きで表示し、余白・項目間隔・垂直中央を計測。
// 使用法：node scripts/ui-audit/grid-audit.mjs [buildDir] [--json out.json] [--only main|lens|subtitle|overlay] [--sizes medium,...]
// 違反が有れば終了コード 1。
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { HERE, TEXT_SIZES, args, collector, init, launch, reach, serve, wait, walkMain } from './harness.mjs';

const { opt, rest } = args();
const jsonOut = opt('--json');
const only = opt('--only');
const SIZES = (opt('--sizes') ?? TEXT_SIZES.join(',')).split(',');
const { url, close } = await serve(rest[0] ?? 'build');

const MEASURE = readFileSync(join(HERE, 'measure.js'), 'utf8');
const found = collector();
const browser = await launch();
const measure = async (pg, state) => found.record(state, await pg.evaluate(MEASURE));

const BLOCKS = [
  { source: 'The North Wind and the Sun', translation: '北風と太陽', bbox: [20, 20, 300, 28], same_language: false },
  { source: 'The North Wind and the Sun were disputing which was the stronger.', translation: '北風と太陽が、どちらが強いかで言い争っていました。', bbox: [20, 64, 400, 44], same_language: false }
];
const LENS = JSON.stringify({
  get_settings: { subtitle: { target_language: 'ja', show_original: true, font_size_px: 22 }, hotkeys: { toggle_window_operate: 'Ctrl+Shift+O' } },
  list_lens_windows: [{ id: 'lens-1', target_language: 'ja' }], refresh_lens: true, subtitle_state: false
});
async function win(label, route, w, h) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 1, reducedMotion: 'reduce' });
  await ctx.addInitScript(init(label, '{}', LENS));
  const pg = await ctx.newPage(); await pg.goto(url(route)); await wait(pg, 700);
  return pg;
}
async function lensWindow() {
  for (const w of [700, 420, 300, 200]) {
    const pg = await win('lens-1', '/lens/?id=lens-1', w, 320);
    await measure(pg, `lens-${w}-idle`);
    await pg.evaluate((blocks) => window.__emit('moonlens://lens-live', { kind: 'blocks', blocks, translating: false, provider: 'PLaMo', via_cloud: false }), BLOCKS);
    await wait(pg, 300); await measure(pg, `lens-${w}`);
    // 縦メニューは収納項目が有る幅のみ表示
    if (await pg.locator('.more').count()) { await reach(`lens-${w}-menu`, async () => { await pg.click('.more', { timeout: 2000 }); await pg.locator('.menu').waitFor({ timeout: 2000 }); }); await measure(pg, `lens-${w}-menu`); }
    await pg.context().close();
  }
  for (const placement of ['outside', 'free']) {
    const pg = await win('lens-1-text', '/lens/text/?id=lens-1', 420, 300);
    await pg.evaluate(([blocks, placement]) => window.__emit('moonlens://lens-text', { blocks, translation: '', source: null, explanation: placement === 'free' ? '北風と太陽の寓話の冒頭。' : null, placement }), [BLOCKS, placement]);
    await wait(pg, 300); await measure(pg, `lens-text-${placement}`);
    await pg.context().close();
  }
}
async function subtitleWindow() {
  for (const [w, h] of [[900, 190], [480, 160]]) {
    const pg = await win('subtitle', '/subtitle/', w, h);
    const emit = (p) => pg.evaluate((p) => window.__emit('moonlens://subtitle', p), p);
    const m = async (n) => { await wait(pg, 200); await measure(pg, `subtitle-${w}-${n}`); };
    await m('idle');
    await emit({ kind: 'started', device: 'スピーカー (Realtek(R) Audio)' }); await m('started');
    await emit({ kind: 'status', phase: 'recognizing' }); await m('recognizing');
    await emit({ kind: 'status', phase: 'translating' });
    await emit({ kind: 'chunk', original: 'And so, my fellow Americans,', translated: 'そして、同胞であるアメリカ国民の皆さん、' }); await m('chunk');
    await pg.mouse.move(w / 2, 20); await m('hover');
    await emit({ kind: 'error', message: '音声認識の連続失敗により停止：モデル読込失敗' }); await emit({ kind: 'stopped' }); await m('error');
    await pg.context().close();
  }
}
async function overlayWindow() {
  const pg = await win('overlay', '/overlay/', 480, 320);
  const emit = (p) => pg.evaluate((p) => window.__emit('moonlens://overlay', p), p);
  await measure(pg, 'overlay-idle');
  await emit({ kind: 'processing', message: '翻訳中…' }); await wait(pg, 200); await measure(pg, 'overlay-processing');
  await emit({ kind: 'result', source_text: 'The North Wind and the Sun', result: { output: { Text: '北風と太陽' }, via_cloud: false, used_provider: 'PLaMo' }, intent: 'translate', source_rect: null });
  await wait(pg, 300); await measure(pg, 'overlay-result');
  await emit({ kind: 'error', message: '翻訳失敗：モデル未導入' }); await wait(pg, 200); await measure(pg, 'overlay-error');
  await pg.context().close();
}

if (!only || only === 'main') for (const ts of SIZES) await walkMain(browser, url, { textSize: ts }, (pg, name) => measure(pg, `${name}@${ts}`));
if (!only || only === 'lens') await lensWindow();
if (!only || only === 'subtitle') await subtitleWindow();
if (!only || only === 'overlay') await overlayWindow();
await browser.close(); close();
process.exit(found.report(jsonOut) ? 1 : 0);
