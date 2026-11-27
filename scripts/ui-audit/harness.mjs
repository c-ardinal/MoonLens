// UI 監査の共通基盤：`vite build` の出力の配信、IPC モック、ヘッドレス Chromium、違反の集計。
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const { chromium } = createRequire(import.meta.url)('playwright');
export const HERE = dirname(fileURLToPath(import.meta.url));
export const MOCK = readFileSync(join(HERE, 'mock.json'), 'utf8');
/** 英語辞書（日本語の原文 → 英訳）。 */
/** 言語ファイル（locales/<言語コード>.json）。 */
export const dict = (lang) => JSON.parse(readFileSync(join(HERE, `../../locales/${lang}.json`), 'utf8'));
/** 表示言語の文言：操作対象の名称（ボタン名等）の指定に使用。 */
/** キー ID から指定言語の文言を取得する関数。 */
export const label = (lang) => { const d = dict(lang); return (key) => d[key] ?? key; };
export const TEXT_SIZES = ['xsmall', 'small', 'medium', 'large', 'xlarge'];

/** 引数の解析：`--key value` を取出し、残余を位置引数として返却。 */
export function args(argv = process.argv.slice(2)) {
  const rest = [...argv];
  const opt = (k) => { const i = rest.indexOf(k); return i < 0 ? null : rest.splice(i, 2)[1]; };
  return { opt, rest };
}

const MIME = { '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html', '.woff2': 'font/woff2', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png' };
/** ビルド出力の配信。戻り値：URL 生成関数と停止関数。 */
export async function serve(root) {
  const srv = createServer((q, r) => {
    let f = join(root, decodeURIComponent(q.url.split('?')[0]));
    if (existsSync(f) && statSync(f).isDirectory()) f = join(f, 'index.html');
    if (!existsSync(f)) f = join(root, 'index.html');
    r.writeHead(200, { 'content-type': MIME[extname(f)] ?? 'application/octet-stream' });
    r.end(readFileSync(f));
  }).listen(0);
  await new Promise((ok) => srv.once('listening', ok));
  return { url: (p) => `http://127.0.0.1:${srv.address().port}${p}`, close: () => srv.close() };
}

/** IPC モック：invoke は表引き、listen は window.__emit で発火可能。 */
export const init = (label, mock, extra = '{}') => `(() => {
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

/** 文字サイズ・表示言語を差替えた設定モック。 */
export const mockWith = (ts, lang = 'ja') =>
  MOCK.replace(/"text_size":\s*"medium"/g, `"text_size": "${ts}"`).replace(/"ui_language":\s*"ja"/g, `"ui_language": "${lang}"`);
/** 翻訳モデル（PLaMo 2B）を未取得とした設定モック（初期設定の許諾確認の到達用）。 */
const mockPlamoMissing = (ts, lang) => {
  const m = JSON.parse(mockWith(ts, lang));
  for (const x of m.list_llm_models.models) if (x.id === 'plamo-3-translate-2b') x.installed = false;
  m.get_system_profile = {
    profile: { os: 'windows', cpu_name: 'Ryzen 7 7840U', cpu_cores: 16, ram_total: 32 * 2 ** 30, gpus: [], disk_free: 400 * 2 ** 30, use_gpu: true },
    accelerator: null,
    recommended: {
      translate: { model: 'plamo-3-translate-2b', reason: 'setup:high_quality_and_fast_runs_comfortably_on' },
      assistant: { model: 'gemma-4-e4b', reason: 'setup:high_quality_and_fast_runs_comfortably_on' },
      ocr: { model: 'ppocr', reason: '' },
      asr: { model: 'whisper-small', reason: '' }
    }
  };
  return JSON.stringify(m);
};

export const launch = () => chromium.launch();
export const wait = (pg, ms) => pg.waitForTimeout(ms);
/** 状態への到達操作。失敗時は監査を中断（未到達の状態を計測済みと誤認しないため）。 */
export const reach = async (state, f) => { try { await f(); } catch (e) { throw new Error(`状態「${state}」へ未到達：${String(e.message).split('\n')[0]}`); } };

/** 違反の集計：同一の違反は状態名を束ねて 1 件へ統合。 */
export function collector() {
  const found = new Map();
  return {
    record(state, rows) {
      for (const v of rows) {
        const k = `${v.rule}|${v.where}|${v.detail}|${v.value}`;
        const hit = found.get(k) ?? { ...v, states: new Set() };
        hit.states.add(state); found.set(k, hit);
      }
    },
    /** 結果を出力し、違反件数を返却。 */
    report(jsonOut) {
      const rows = [...found.values()].map((v) => ({ ...v, states: [...v.states] }));
      if (jsonOut) writeFileSync(jsonOut, JSON.stringify(rows, null, 1));
      const byRule = {};
      for (const v of rows) (byRule[v.rule] ??= []).push(v);
      for (const [rule, list] of Object.entries(byRule)) {
        console.log(`\n== ${rule}：${list.length} 件`);
        for (const v of list.sort((a, b) => a.where.localeCompare(b.where))) console.log(`  ${v.where} ｜ ${v.detail} ｜ ${v.value}px ｜ ${v.states.slice(0, 3).join(', ')}${v.states.length > 3 ? ` 他 ${v.states.length - 3}` : ''}`);
      }
      console.log(`\n違反：${rows.length} 件`);
      return rows.length;
    }
  };
}

const TRANSLATED = {
  run_translate: { task: 'translate', output: { Text: '素早い茶色の狐。\n2 行目。' }, used_provider: 'PLaMo', via_cloud: false, latency_ms: 120, fallback_reasons: [] }
};
/** 画像翻訳の応答：900×700 の画像に段落 7 件（英訳、長文を含む）。 */
const IMAGE_BLOCKS = [
  ['大規模なリニューアルを経て、「グリーンガーデンレストラン」をリニューアルオープンいたしましたので、お知らせいたします。', 'We are excited to announce the reopening of "Green Garden Restaurant" after extensive renovations. The renewed restaurant features a modern design with an expanded dining space.'],
  ['11月1日より、新たなファーム・トゥ・テーブル（地産地消）コンセプトを導入いたします。', 'Starting November 1st, we will introduce a new farm-to-table concept.'],
  ['リニューアルオープン特別特典（11月1日〜30日）', 'Grand reopening special offers (November 1st to 30th)'],
  ['新サービス：個室ダイニング利用可能（最低8名様、5日前までのご予約必須）', 'New service: private dining rooms available (minimum 8 guests, reservation required 5 days in advance)'],
  ['週末ブランチ（10:00〜15:00、要予約）', 'Weekend brunch (10:00-15:00, reservation required)'],
  ['電話：555-0123', 'Phone: 555-0123'],
  ['メール：info@greengarden.example ※料理教室参加者の方は、次回のお食事ご利用時に10%割引となります。', 'Email: info@greengarden.example *Cooking class participants receive a 10% discount on their next meal.']
].map(([source, translation], i) => ({
  source, translation, bbox: [40, 40 + i * 90, 820, 64], line_height: 28, colors: null, same_language: false, align: 'left',
  lines: [[40, 40 + i * 90, 820, 30], [40, 74 + i * 90, 600, 30]]
}));
const IMAGE_RESULT = {
  translate_image: {
    intent: 'translate', source_text: null, source_rect: null, blocks: IMAGE_BLOCKS, image_size: [900, 700], source_language: 'ja', target_language: 'en',
    result: { task: 'translate', output: { Text: IMAGE_BLOCKS.map((b) => b.translation).join('\n') }, used_provider: 'PLaMo', via_cloud: false, latency_ms: 2140, fallback_reasons: [] }
  }
};
/** 画像翻訳の入力画像（900×700、白地に罫線）。ページ内の Canvas で生成。 */
const imageFile = async (pg) => ({
  name: 'image.png', mimeType: 'image/png',
  buffer: Buffer.from((await pg.evaluate(() => {
    const c = Object.assign(document.createElement('canvas'), { width: 900, height: 700 });
    const g = c.getContext('2d'); g.fillStyle = '#fff'; g.fillRect(0, 0, 900, 700);
    g.fillStyle = '#222'; for (let i = 0; i < 7; i++) { g.fillRect(40, 52 + i * 90, 820, 3); g.fillRect(40, 86 + i * 90, 600, 3); }
    return c.toDataURL('image/png').split(',')[1];
  })), 'base64')
});

/**
 * メインウィンドウの全状態を巡回し、各状態で visit(page, 状態名) を実行。
 * 状態：ホーム・パレット・テキスト翻訳（結果・通知・右クリックメニュー）・画像翻訳（空・結果）・
 * 履歴（音声翻訳の記録・通知・確認）・初期設定（許諾確認を含む）・設定の全分類（許諾確認を含む）。
 */
export async function walkMain(browser, url, { textSize = 'medium', width = 1280, height = 800, scale = 1, lang = 'ja' } = {}, visit) {
  const l = label(lang);
  const ctx = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: scale, reducedMotion: 'reduce' });
  await ctx.addInitScript(init('main', mockWith(textSize, lang), JSON.stringify({ ...TRANSLATED, ...IMAGE_RESULT })));
  const open = async (route) => { const pg = await ctx.newPage(); await pg.goto(url(route)); await wait(pg, 1200); return pg; };
  let pg = await open('/'); await visit(pg, 'home');
  await pg.keyboard.press('Control+k'); await wait(pg, 400); await visit(pg, 'palette'); await pg.close();
  pg = await open('/translate/'); await visit(pg, 'translate');
  await pg.locator('textarea').first().fill('The quick brown fox.\nSecond line.'); await pg.keyboard.press('Control+Enter');
  await reach('translate-result', () => pg.getByText('素早い茶色の狐。').first().waitFor({ timeout: 5000 })); await visit(pg, 'translate-result');
  await reach('translate-toast', async () => { await pg.getByRole('button', { name: l('common:copy_translation') }).click({ timeout: 2000 }); await pg.locator('.toast').first().waitFor({ timeout: 2000 }); }); await visit(pg, 'translate-toast');
  await reach('context-menu', async () => { await pg.locator('textarea').first().click({ button: 'right', timeout: 2000 }); await pg.locator('.ctx-menu').waitFor({ timeout: 2000 }); }); await visit(pg, 'context-menu');
  await pg.close();
  pg = await open('/image/'); await visit(pg, 'image');
  await reach('image-result', async () => { await pg.locator('input[type=file]').setInputFiles(await imageFile(pg)); await pg.getByText('Phone: 555-0123').first().waitFor({ timeout: 5000 }); await wait(pg, 300); }); await visit(pg, 'image-result');
  await pg.close();
  pg = await open('/history/'); await visit(pg, 'history');
  await reach('history-subtitle', async () => { await pg.locator('li button').filter({ hasText: l('common:audio_translation') }).first().click({ timeout: 2000 }); await pg.getByText(l('subtitle-record-detail:audio_source').split('{')[0]).first().waitFor({ timeout: 2000 }); await wait(pg, 300); }); await visit(pg, 'history-subtitle');
  await reach('history-toast', async () => { await pg.getByRole('button', { name: l('common:delete_this_item') }).first().click({ timeout: 2000 }); await pg.locator('.toast').first().waitFor({ timeout: 2000 }); }); await visit(pg, 'history-toast');
  await reach('history-confirm', async () => { await pg.getByText(l('common:clear_all')).click({ timeout: 2000 }); await pg.getByRole('dialog').waitFor({ timeout: 2000 }); }); await visit(pg, 'history-confirm');
  await pg.close();
  pg = await open('/setup/'); await visit(pg, 'setup'); await pg.close();
  // 初期設定：許諾確認（取得対象に同意が必要なモデルを含む場合のみ表示される手順）
  const ctx2 = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: scale, reducedMotion: 'reduce' });
  await ctx2.addInitScript(init('main', mockPlamoMissing(textSize, lang)));
  pg = await ctx2.newPage(); await pg.goto(url('/setup/')); await wait(pg, 1200);
  await reach('setup-select', async () => { await pg.getByRole('button', { name: l('setup:next') }).click({ timeout: 3000 }); await pg.getByRole('radiogroup').first().waitFor({ timeout: 3000 }); }); await visit(pg, 'setup-select');
  await reach('setup-consent', async () => { await pg.getByRole('button', { name: l('setup:accept_and_download') }).click({ timeout: 3000 }); await pg.getByRole('checkbox').waitFor({ timeout: 3000 }); }); await visit(pg, 'setup-consent');
  await pg.getByRole('checkbox').check(); await wait(pg, 200); await visit(pg, 'setup-consent-checked');
  await ctx2.close();
  pg = await open('/settings/');
  const tabs = await pg.locator('.tab').count();
  for (let i = 0; i < tabs; i++) {
    await pg.locator('.tab').nth(i).click(); await wait(pg, 250);
    const subs = Math.max(1, await pg.locator('.subtab').count());
    for (let j = 0; j < subs; j++) {
      if (await pg.locator('.subtab').count()) await pg.locator('.subtab').nth(j).click();
      await wait(pg, 350); await visit(pg, `settings-${i}-${j}`);
    }
  }
  // 設定のモデル一覧：未取得モデルの取得前に表示する許諾確認
  pg = await open('/settings/#section-models');
  await reach('settings-consent', async () => {
    await pg.getByRole('radio', { name: /^PLaMo 3 Translate 0\.8B/ }).first().click({ timeout: 3000 });
    await pg.getByRole('button', { name: l('model-choice:download_and_switch') }).click({ timeout: 3000 });
    await pg.getByRole('checkbox').waitFor({ timeout: 3000 });
  }); await visit(pg, 'settings-consent'); await pg.close();
  await ctx.close();
}

const BLOCKS = [
  { source: 'The North Wind and the Sun', translation: '北風と太陽', bbox: [20, 20, 300, 28], same_language: false },
  { source: 'The North Wind and the Sun were disputing which was the stronger.', translation: '北風と太陽が、どちらが強いかで言い争っていました。', bbox: [20, 64, 400, 44], same_language: false }
];
const lensMock = (lang) => JSON.stringify({
  get_settings: { appearance: { ui_language: lang }, subtitle: { target_language: 'ja', show_original: true, font_size_px: 22 }, hotkeys: { toggle_window_operate: 'Ctrl+Shift+O', toggle_window_pause: 'Ctrl+Shift+X' } },
  list_lens_windows: [{ id: 'lens-1', target_language: 'ja' }], refresh_lens: true, subtitle_state: false,
  window_attachment: { target: null, dock: null },
  stream_state: { supported: true, active: false, title: null },
  set_stream: { supported: true, active: true, title: label(lang)('stream:moonlens_stream_name').replace('{name}', 'Chapter 1 — The Lighthouse') },
  list_attach_targets: [
    { id: 11, pid: 100, app: 'Game.exe', title: 'Chapter 1 — The Lighthouse', rect: { x: 0, y: 0, w: 800, h: 600 }, overlap: 0.8 },
    { id: 12, pid: 101, app: 'firefox', title: 'MoonLens docs — Mozilla Firefox', rect: { x: 0, y: 0, w: 800, h: 600 }, overlap: 0 }
  ]
});
const ATTACHED = { target: { id: 11, pid: 100, app: 'Game.exe', title: 'Chapter 1 — The Lighthouse' }, dock: 'cover' };
/**
 * メインウィンドウ以外（ウィンドウ翻訳・字幕・訳文オーバーレイ）の全状態を巡回し、各状態で visit(page, 状態名) を実行。
 * only：lens／subtitle／overlay の単独指定。lang：表示言語。
 */
export async function walkWindows(browser, url, { only = null, lang = 'ja' } = {}, visit) {
  async function win(label, route, w, h) {
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 1, reducedMotion: 'reduce' });
    await ctx.addInitScript(init(label, '{}', lensMock(lang)));
    const pg = await ctx.newPage(); await pg.goto(url(route)); await wait(pg, 700);
    return pg;
  }
  async function lensWindow() {
    for (const w of [700, 420, 300, 200]) {
      const pg = await win('lens-1', '/lens/?id=lens-1', w, 320);
      await visit(pg, `lens-${w}-idle`);
      await pg.evaluate((blocks) => window.__emit('moonlens://lens-live', { kind: 'blocks', blocks, translating: false, provider: 'PLaMo', via_cloud: false }), BLOCKS);
      await wait(pg, 300); await visit(pg, `lens-${w}`);
      await pg.evaluate(() => window.__emit('moonlens://lens-pause', true)); await wait(pg, 100); await visit(pg, `lens-${w}-paused`);
      await pg.evaluate(() => window.__emit('moonlens://lens-pause', false)); await wait(pg, 100);
      // 縦メニューは収納項目が有る幅のみ表示
      if (await pg.locator('.more').count()) { await reach(`lens-${w}-menu`, async () => { await pg.click('.more', { timeout: 2000 }); await pg.locator('.menu').waitFor({ timeout: 2000 }); }); await visit(pg, `lens-${w}-menu`); }
      // 吸着メニュー：吸着なし（候補一覧）・吸着中（配置・解除）。縦メニューへ収納中の幅は縦メニュー経由
      const attachMenu = async (state) => {
        await reach(state, async () => {
          if (await pg.locator('.am').count()) return;
          if (await pg.locator('.tb[data-action="attach"]:not(.off)').count()) await pg.click('.tb[data-action="attach"]:not(.off)', { timeout: 2000 });
          else { if (!(await pg.locator('.menu').count())) await pg.click('.more', { timeout: 2000 }); await pg.click('.mi[data-action="attach"]', { timeout: 2000 }); }
          await pg.locator('.am .row').first().waitFor({ timeout: 2000 });
        });
        await visit(pg, state);
      };
      await attachMenu(`lens-${w}-attach`);
      await pg.evaluate((a) => window.__emit('moonlens://attach', a), ATTACHED);
      await wait(pg, 200); await attachMenu(`lens-${w}-attached`);
      // 配信用ウィンドウ：稼働中（切替行・取込対象の表示）
      await reach(`lens-${w}-stream`, async () => { await pg.click('.am .sw', { timeout: 2000 }); await pg.locator('.am .sw[aria-checked="true"]').waitFor({ timeout: 2000 }); });
      await visit(pg, `lens-${w}-stream`);
      await pg.context().close();
    }
    for (const placement of ['outside', 'free']) {
      const pg = await win('lens-1-text', '/lens/text/?id=lens-1', 420, 300);
      await pg.evaluate(([blocks, placement]) => window.__emit('moonlens://lens-text', { blocks, translation: '', source: null, explanation: placement === 'free' ? { status: 'done', text: '北風と太陽の寓話の冒頭。' } : null, placement }), [BLOCKS, placement]);
      await wait(pg, 300); await visit(pg, `lens-text-${placement}`);
      await pg.context().close();
    }
  }
  async function subtitleWindow() {
    for (const [w, h] of [[900, 190], [480, 160]]) {
      const pg = await win('subtitle', '/subtitle/', w, h);
      const emit = (p) => pg.evaluate((p) => window.__emit('moonlens://subtitle', { window: 'subtitle', ...p }), p);
      const m = async (n) => { await wait(pg, 200); await visit(pg, `subtitle-${w}-${n}`); };
      await m('idle');
      await emit({ kind: 'started', device: 'スピーカー (Realtek(R) Audio)', source: { kind: 'system' } }); await m('started');
      // 入力レベル：閾値未満 → 音声検出（波形・メーター・状態表記）
      const level = (bins) => pg.evaluate((bins) => window.__emit('moonlens://subtitle-level', { bins, gate_db: -50.3 }), bins);
      await level(Array(40).fill(-64)); await m('low');
      await level(Array.from({ length: 40 }, (_, i) => -34 + 12 * Math.sin(i / 3))); await m('detected');
      await emit({ kind: 'status', phase: 'recognizing' }); await m('recognizing');
      await emit({ kind: 'status', phase: 'translating' });
      await emit({ kind: 'chunk', id: 1, original: 'And so, my fellow Americans,', translated: 'そして、同胞であるアメリカ国民の皆さん、' }); await m('chunk');
      await pg.mouse.move(w / 2, 20); await m('hover');
      await emit({ kind: 'candidate', phrase: 'Thank you.' }); await m('candidate');
      // 吸着メニュー：吸着なし（候補一覧）・吸着中（配置 6 種・解除）
      const attachMenu = async (n) => {
        await reach(`subtitle-${w}-${n}`, async () => {
          if (!(await pg.locator('.am').count())) await pg.click('[data-action="attach"]', { timeout: 2000 });
          await pg.locator('.am .row').first().waitFor({ timeout: 2000 });
        });
        await m(n);
      };
      await attachMenu('attach');
      await pg.evaluate((a) => window.__emit('moonlens://attach', a), { ...ATTACHED, dock: 'inside' });
      await attachMenu('attached');
      await pg.keyboard.press('Escape');
      await emit({ kind: 'error', message: label(lang)('subtitle:stopped_after_repeated_speech_recognition').replace('{detail}', 'model load failed') }); await emit({ kind: 'stopped' }); await m('error');
      await pg.context().close();
    }
  }
  async function overlayWindow() {
    const pg = await win('overlay', '/overlay/', 480, 320);
    const emit = (p) => pg.evaluate((p) => window.__emit('moonlens://overlay', p), p);
    await visit(pg, 'overlay-idle');
    await emit({ kind: 'processing', message: label(lang)('common:translating') }); await wait(pg, 200); await visit(pg, 'overlay-processing');
    await emit({ kind: 'result', source_text: 'The North Wind and the Sun', result: { output: { Text: '北風と太陽' }, via_cloud: false, used_provider: 'PLaMo' }, intent: 'translate', source_rect: null });
    await wait(pg, 300); await visit(pg, 'overlay-result');
    await emit({ kind: 'error', message: '翻訳失敗：モデル未導入' }); await wait(pg, 200); await visit(pg, 'overlay-error');
    await pg.context().close();
  }
  if (!only || only === 'lens') await lensWindow();
  if (!only || only === 'subtitle') await subtitleWindow();
  if (!only || only === 'overlay') await overlayWindow();
}
