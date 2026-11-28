// ファイル字幕の再生画面の検査：字幕の同期表示・生成待機・一覧からの移動・表示内容の切替。
// 違反時は終了コード 1。使用：node scripts/ui-audit/player-check.mjs build [--out .shots]
import { mkdirSync } from 'node:fs';
import { args, serve, init, MOCK, launch, wait, label } from './harness.mjs';

const l = label('ja');

const { opt, rest } = args();
const out = opt('--out');
const root = rest[0] ?? 'build';
if (out) mkdirSync(out, { recursive: true });
const srv = await serve(root);
const browser = await launch({ args: ['--autoplay-policy=no-user-gesture-required'] });
const failures = [];
const expect = (ok, what) => { console.log(`${ok ? 'OK  ' : 'NG  '}${what}`); if (!ok) failures.push(what); };

const NAME = 'keynote.webm';
const status = (over) => ({ phase: 'running', name: NAME, duration_ms: 12_000, done_ms: 0, ready_ms: 0, cues: 0, error: null, ...over });
const CUES = [
  { start_ms: 500, end_ms: 2500, original: 'Good evening, everyone.', translated: '皆さん、こんばんは。' },
  { start_ms: 3000, end_ms: 5200, original: 'Tonight we open the new hall.', translated: '今夜、新しいホールを開館します。' },
  { start_ms: 8000, end_ms: 10_500, original: 'Thank you for coming.', translated: 'ご来場に感謝します。' }
];
const emit = (pg, update) => pg.evaluate((u) => window.__emit('moonlens://file-subtitle', u), update);
const video = (pg) => pg.evaluate(() => {
  const v = document.querySelector('video');
  const cue = v.textTracks[0]?.activeCues?.[0];
  return { t: v.currentTime, paused: v.paused, cue: cue ? cue.text : null };
});

for (const [w, h] of [[1280, 800], [1120, 700]]) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 1, reducedMotion: 'reduce' });
  await ctx.addInitScript(init('main', MOCK, JSON.stringify({ file_subtitle_media: '/__fixtures/player.webm', file_subtitle_cues: [] })));
  const pg = await ctx.newPage();
  await pg.goto(srv.url('/file/')); await wait(pg, 1000);
  // 生成中：字幕 2 件、生成位置 6 秒
  await emit(pg, { status: status({}), cue: null });
  await pg.locator('video').waitFor({ timeout: 3000 });
  await emit(pg, { status: status({ cues: 1, done_ms: 2600, ready_ms: 2600 }), cue: CUES[0] });
  await emit(pg, { status: status({ cues: 2, done_ms: 6000, ready_ms: 6000 }), cue: CUES[1] });
  await pg.evaluate(() => document.querySelector('video').play());
  await pg.waitForFunction(() => document.querySelector('video').currentTime > 1, null, { timeout: 5000 });
  let v = await video(pg);
  expect(v.cue === CUES[0].translated, `${w}x${h} 再生位置の訳文を表示（${v.cue}）`);
  expect(await pg.locator('li.active').count() === 1, `${w}x${h} 一覧の該当字幕を強調`);
  if (out) await pg.screenshot({ path: `${out}/playing-${w}x${h}.png` });

  // 生成位置の手前で待機
  await pg.waitForFunction(() => document.querySelector('video').paused, null, { timeout: 8000 });
  v = await video(pg);
  expect(v.t < 6 && v.t > 3.5, `${w}x${h} 生成位置の手前で一時停止（${v.t.toFixed(2)} 秒）`);
  expect(await pg.getByRole('status').filter({ hasText: l('file-subtitle:waiting_for_subtitles') }).count() === 1, `${w}x${h} 待機中の表示`);
  if (out) await pg.screenshot({ path: `${out}/waiting-${w}x${h}.png` });

  // 待機中の巻戻し：移動先の字幕が確定済み（10 秒先行）で再開
  await emit(pg, { status: status({ cues: 2, done_ms: 12_000, ready_ms: 11_500 }), cue: null });
  await wait(pg, 300);
  expect((await video(pg)).paused, `${w}x${h} 先行不足の間は待機を継続`);
  await pg.evaluate(() => { document.querySelector('video').currentTime = 1; });
  await pg.waitForFunction(() => !document.querySelector('video').paused, null, { timeout: 3000 }).catch(() => {});
  v = await video(pg);
  expect(!v.paused, `${w}x${h} 待機中の巻戻しで再生を再開（${v.t.toFixed(2)} 秒）`);
  await pg.waitForFunction(() => document.querySelector('video').paused, null, { timeout: 15_000 });
  v = await video(pg);
  expect(v.t > 9 && v.t < 11.5, `${w}x${h} 確定位置の手前で再度待機（${v.t.toFixed(2)} 秒）`);

  // 生成の完了で再開
  await emit(pg, { status: status({ cues: 3, done_ms: 12_000, ready_ms: 12_000 }), cue: CUES[2] });
  await emit(pg, { status: status({ phase: 'done', cues: 3, done_ms: 12_000, ready_ms: 12_000 }), cue: null });
  await wait(pg, 600);
  v = await video(pg);
  expect(!v.paused, `${w}x${h} 生成の完了で再生を再開`);

  // 一覧からの移動
  await pg.locator('ol li').nth(2).locator('button').click();
  // 字幕の切替は再生位置の更新の直後（activeCues の更新待ち）
  await pg.waitForFunction((text) => { const v = document.querySelector('video'); return v.currentTime >= 8 && v.textTracks[0].activeCues[0]?.text === text; }, CUES[2].translated, { timeout: 3000 }).catch(() => {});
  v = await video(pg);
  expect(v.cue === CUES[2].translated, `${w}x${h} 一覧の選択で再生位置を移動（${v.t.toFixed(2)} 秒、${v.cue}）`);

  // 併記へ切替
  await pg.locator('select').selectOption('both'); await wait(pg, 300);
  v = await video(pg);
  expect(v.cue === `${CUES[2].translated}\n${CUES[2].original}`, `${w}x${h} 併記へ切替（${JSON.stringify(v.cue)}）`);
  await pg.evaluate(() => document.querySelector('video').pause());
  if (out) await pg.screenshot({ path: `${out}/both-${w}x${h}.png` });
  await ctx.close();
}

// 再生不可の形式：理由を表示し、字幕の一覧は継続
{
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, reducedMotion: 'reduce' });
  await ctx.addInitScript(init('main', MOCK, JSON.stringify({ file_subtitle_media: '/__fixtures/missing.mkv', file_subtitle_cues: CUES, file_subtitle_status: status({ phase: 'done', cues: 3, done_ms: 12_000, ready_ms: 12_000 }) })));
  const pg = await ctx.newPage();
  await pg.goto(srv.url('/file/')); await wait(pg, 1500);
  expect(await pg.getByRole('alert').filter({ hasText: l('file-subtitle:cannot_play') }).count() === 1, '再生不可の形式で理由を表示');
  expect(await pg.locator('ol li').count() === 3, '再生不可でも字幕の一覧を表示（既存の字幕の取得）');
  if (out) await pg.screenshot({ path: `${out}/unplayable.png` });
  await ctx.close();
}

await browser.close(); srv.close();
if (failures.length) { console.error(`違反 ${failures.length} 件`); process.exit(1); }
console.log('違反なし');
