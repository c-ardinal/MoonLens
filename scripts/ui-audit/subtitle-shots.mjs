// 音声翻訳の記録の画面保存：詳細・再翻訳中・完了（取消付き通知）を 2 寸法で保存。
// 使用：node scripts/ui-audit/subtitle-shots.mjs build [--out .shots]
import { args, serve, init, MOCK, launch, wait } from './harness.mjs';

const { opt, rest } = args();
const out = opt('--out') ?? '.shots';
const root = rest[0] ?? 'build';
const srv = await serve(root);
const browser = await launch();
// run_translate を遅延付きの別応答へ差替え（再翻訳の進行を可視化）
const slow = `(() => { const o = window.__TAURI_INTERNALS__.invoke; let n = 0;
  window.__TAURI_INTERNALS__.invoke = async (cmd, a) => {
    if (cmd === 'run_translate') { await new Promise((r) => setTimeout(r, 700)); n++;
      return { task: 'translate', output: { Text: '再翻訳 ' + n + '：' + a.args.text.slice(0, 40) }, used_provider: 'PLaMo', via_cloud: false, latency_ms: 1, fallback_reasons: [] }; }
    if (cmd === 'replace_subtitle_lines') return a.revision + 1;
    return o(cmd, a);
  }; })();`;
for (const [w, h] of [[1280, 800], [720, 480]]) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 2, reducedMotion: 'reduce' });
  await ctx.addInitScript(init('main', MOCK));
  await ctx.addInitScript(slow);
  const pg = await ctx.newPage();
  await pg.goto(srv.url('/history/')); await wait(pg, 1200);
  await pg.getByRole('button', { name: /音声翻訳/ }).first().click();
  await pg.getByText('音源：').first().waitFor(); await wait(pg, 300);
  await pg.screenshot({ path: `${out}/record-${w}x${h}.png` });
  await pg.getByRole('button', { name: '再翻訳' }).click(); await wait(pg, 1700);
  await pg.screenshot({ path: `${out}/running-${w}x${h}.png` });
  await pg.getByText('再翻訳完了').first().waitFor({ timeout: 10000 }); await wait(pg, 300);
  await pg.screenshot({ path: `${out}/done-${w}x${h}.png` });
  await ctx.close();
}
await browser.close(); srv.close();
