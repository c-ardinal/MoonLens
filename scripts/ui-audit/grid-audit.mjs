#!/usr/bin/env node
// 4px 格子の監査：`vite build` の出力を IPC モック付きで表示し、余白・項目間隔・垂直中央を計測。
// 使用法：node scripts/ui-audit/grid-audit.mjs [buildDir] [--json out.json] [--only main|lens|subtitle|overlay] [--sizes medium,...]
// 違反が有れば終了コード 1。
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const { chromium } = createRequire(import.meta.url)('playwright');
const HERE = dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const opt = (k) => { const i = argv.indexOf(k); return i < 0 ? null : argv.splice(i, 2)[1]; };
const jsonOut = opt('--json');
const only = opt('--only');
const SIZES = (opt('--sizes') ?? 'xsmall,small,medium,large,xlarge').split(',');
const root = argv[0] ?? 'build';

const MIME = { '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html', '.woff2': 'font/woff2', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png' };
const srv = createServer((q, r) => {
  let f = join(root, decodeURIComponent(q.url.split('?')[0]));
  if (existsSync(f) && statSync(f).isDirectory()) f = join(f, 'index.html');
  if (!existsSync(f)) f = join(root, 'index.html');
  r.writeHead(200, { 'content-type': MIME[extname(f)] ?? 'application/octet-stream' });
  r.end(readFileSync(f));
}).listen(0);
await new Promise((ok) => srv.once('listening', ok));
const url = (p) => `http://127.0.0.1:${srv.address().port}${p}`;

const MEASURE = readFileSync(join(HERE, 'measure.js'), 'utf8');
const MOCK = readFileSync(join(HERE, 'mock.json'), 'utf8');
/** IPC モック：invoke は表引き、listen は window.__emit で発火可能。 */
const init = (label, mock, extra = '{}') => `(() => {
  const M = Object.assign(${mock}, ${extra});
  const cbs = {}; let n = 1; const listeners = [];
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: '${label}' }, currentWebview: { label: '${label}', windowLabel: '${label}' } },
    transformCallback(cb) { const id = n++; cbs[id] = cb; return id; },
    unregisterCallback(id) { delete cbs[id]; },
    convertFileSrc: (p) => p,
    async invoke(cmd, args) {
      if (cmd === 'plugin:event|listen') { listeners.push([args.event, args.handler]); return n++; }
      if (cmd.startsWith('plugin:')) return null;
      if (cmd in M) return JSON.parse(JSON.stringify(M[cmd]));
      return null;
    }
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__emit = (event, payload) => { for (const [e, h] of listeners) if (e === event) cbs[h]({ event, payload, id: 0 }); };
})();`;

const found = new Map();
const record = (state, rows) => {
  for (const v of rows) {
    const k = `${v.rule}|${v.where}|${v.detail}|${v.value}`;
    const hit = found.get(k) ?? { ...v, states: new Set() };
    hit.states.add(state); found.set(k, hit);
  }
};
const browser = await chromium.launch();
const wait = (pg, ms) => pg.waitForTimeout(ms);
const measure = async (pg, state) => record(state, await pg.evaluate(MEASURE));
/** 状態への到達操作。失敗時は監査を中断（未到達の状態を計測済みと誤認しないため）。 */
const reach = async (state, f) => { try { await f(); } catch (e) { throw new Error(`状態「${state}」へ未到達：${String(e.message).split('\n')[0]}`); } };

const TRANSLATED = JSON.stringify({
  run_translate: { task: 'translate', output: { Text: '素早い茶色の狐。\n2 行目。' }, used_provider: 'PLaMo', via_cloud: false, latency_ms: 120, fallback_reasons: [] }
});
async function mainWindow(ts) {
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1, reducedMotion: 'reduce' });
  await ctx.addInitScript(init('main', MOCK.replace(/"text_size":\s*"medium"/g, `"text_size": "${ts}"`), TRANSLATED));
  const open = async (route) => { const pg = await ctx.newPage(); await pg.goto(url(route)); await wait(pg, 1200); return pg; };
  const m = (pg, name) => measure(pg, `${name}@${ts}`);
  let pg = await open('/'); await m(pg, 'home');
  await pg.keyboard.press('Control+k'); await wait(pg, 400); await m(pg, 'palette'); await pg.close();
  pg = await open('/translate/'); await m(pg, 'translate');
  await pg.locator('textarea').first().fill('The quick brown fox.\nSecond line.'); await pg.keyboard.press('Control+Enter');
  await reach('translate-result', () => pg.getByText('素早い茶色の狐。').first().waitFor({ timeout: 5000 })); await m(pg, 'translate-result');
  await reach('translate-toast', async () => { await pg.getByRole('button', { name: '訳文をコピー' }).click({ timeout: 2000 }); await pg.locator('.toast').first().waitFor({ timeout: 2000 }); }); await m(pg, 'translate-toast');
  await reach('context-menu', async () => { await pg.locator('textarea').first().click({ button: 'right', timeout: 2000 }); await pg.locator('.ctx-menu').waitFor({ timeout: 2000 }); }); await m(pg, 'context-menu');
  await pg.close();
  pg = await open('/image/'); await m(pg, 'image'); await pg.close();
  pg = await open('/history/'); await m(pg, 'history');
  await reach('history-toast', async () => { await pg.getByRole('button', { name: 'この履歴を削除' }).first().click({ timeout: 2000 }); await pg.locator('.toast').first().waitFor({ timeout: 2000 }); }); await m(pg, 'history-toast');
  await reach('history-confirm', async () => { await pg.getByText('全消去').click({ timeout: 2000 }); await pg.getByRole('dialog').waitFor({ timeout: 2000 }); }); await m(pg, 'history-confirm');
  await pg.close();
  pg = await open('/setup/'); await m(pg, 'setup'); await pg.close();
  pg = await open('/settings/');
  const tabs = await pg.locator('.tab').count();
  for (let i = 0; i < tabs; i++) {
    await pg.locator('.tab').nth(i).click(); await wait(pg, 250);
    const subs = Math.max(1, await pg.locator('.subtab').count());
    for (let j = 0; j < subs; j++) {
      if (await pg.locator('.subtab').count()) await pg.locator('.subtab').nth(j).click();
      await wait(pg, 350); await m(pg, `settings-${i}-${j}`);
    }
  }
  await ctx.close();
}

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

if (!only || only === 'main') for (const ts of SIZES) await mainWindow(ts);
if (!only || only === 'lens') await lensWindow();
if (!only || only === 'subtitle') await subtitleWindow();
if (!only || only === 'overlay') await overlayWindow();
await browser.close(); srv.close();

const rows = [...found.values()].map((v) => ({ ...v, states: [...v.states] }));
if (jsonOut) writeFileSync(jsonOut, JSON.stringify(rows, null, 1));
const byRule = {};
for (const v of rows) (byRule[v.rule] ??= []).push(v);
for (const [rule, list] of Object.entries(byRule)) {
  console.log(`\n== ${rule}：${list.length} 件`);
  for (const v of list.sort((a, b) => a.where.localeCompare(b.where))) console.log(`  ${v.where} ｜ ${v.detail} ｜ ${v.value}px ｜ ${v.states.slice(0, 3).join(', ')}${v.states.length > 3 ? ` 他 ${v.states.length - 3}` : ''}`);
}
console.log(`\n違反：${rows.length} 件`);
process.exit(rows.length ? 1 : 0);
