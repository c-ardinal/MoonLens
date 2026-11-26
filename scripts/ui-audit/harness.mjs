// UI 監査の共通基盤：`vite build` の出力の配信、IPC モック、ヘッドレス Chromium、違反の集計。
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const { chromium } = createRequire(import.meta.url)('playwright');
export const HERE = dirname(fileURLToPath(import.meta.url));
export const MOCK = readFileSync(join(HERE, 'mock.json'), 'utf8');
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

/** 文字サイズを差替えた設定モック。 */
export const mockWithTextSize = (ts) => MOCK.replace(/"text_size":\s*"medium"/g, `"text_size": "${ts}"`);

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
 * 履歴（通知・確認）・初期設定・設定の全分類。
 */
export async function walkMain(browser, url, { textSize = 'medium', width = 1280, height = 800 } = {}, visit) {
  const ctx = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 1, reducedMotion: 'reduce' });
  await ctx.addInitScript(init('main', mockWithTextSize(textSize), JSON.stringify({ ...TRANSLATED, ...IMAGE_RESULT })));
  const open = async (route) => { const pg = await ctx.newPage(); await pg.goto(url(route)); await wait(pg, 1200); return pg; };
  let pg = await open('/'); await visit(pg, 'home');
  await pg.keyboard.press('Control+k'); await wait(pg, 400); await visit(pg, 'palette'); await pg.close();
  pg = await open('/translate/'); await visit(pg, 'translate');
  await pg.locator('textarea').first().fill('The quick brown fox.\nSecond line.'); await pg.keyboard.press('Control+Enter');
  await reach('translate-result', () => pg.getByText('素早い茶色の狐。').first().waitFor({ timeout: 5000 })); await visit(pg, 'translate-result');
  await reach('translate-toast', async () => { await pg.getByRole('button', { name: '訳文をコピー' }).click({ timeout: 2000 }); await pg.locator('.toast').first().waitFor({ timeout: 2000 }); }); await visit(pg, 'translate-toast');
  await reach('context-menu', async () => { await pg.locator('textarea').first().click({ button: 'right', timeout: 2000 }); await pg.locator('.ctx-menu').waitFor({ timeout: 2000 }); }); await visit(pg, 'context-menu');
  await pg.close();
  pg = await open('/image/'); await visit(pg, 'image');
  await reach('image-result', async () => { await pg.locator('input[type=file]').setInputFiles(await imageFile(pg)); await pg.getByText('Phone: 555-0123').first().waitFor({ timeout: 5000 }); await wait(pg, 300); }); await visit(pg, 'image-result');
  await pg.close();
  pg = await open('/history/'); await visit(pg, 'history');
  await reach('history-toast', async () => { await pg.getByRole('button', { name: 'この履歴を削除' }).first().click({ timeout: 2000 }); await pg.locator('.toast').first().waitFor({ timeout: 2000 }); }); await visit(pg, 'history-toast');
  await reach('history-confirm', async () => { await pg.getByText('全消去').click({ timeout: 2000 }); await pg.getByRole('dialog').waitFor({ timeout: 2000 }); }); await visit(pg, 'history-confirm');
  await pg.close();
  pg = await open('/setup/'); await visit(pg, 'setup'); await pg.close();
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
  await ctx.close();
}
