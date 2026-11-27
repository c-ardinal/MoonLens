// 選択欄（<select>）の選択肢の可読性監査：文字色と背景色のコントラスト比を実測。
// 使用法：node scripts/ui-audit/select-contrast.mjs [buildDir]
//
// 選択肢の一覧は OS・WebView が描画し、背景色の指定なしでは配色設定（明／暗）の既定色を使用。
// 文字色のみの指定は暗色設定で判読不可となるため、明・暗の両設定で比 4.5 以上を要求。
import { MOCK, args, init, launch, serve, wait } from './harness.mjs';

const { rest } = args();
const site = await serve(rest[0] ?? 'build');
const browser = await launch();
const MIN_RATIO = 4.5;

const PAGES = [
  { label: 'lens-1', path: '/lens/?id=lens-1', width: 700, height: 320 },
  { label: 'lens-1-text', path: '/lens/text/?id=lens-1', width: 420, height: 300 },
  { label: 'main', path: '/translate/', width: 1280, height: 800 },
  { label: 'main', path: '/image/', width: 1280, height: 800 },
  { label: 'main', path: '/settings/', width: 1280, height: 800 }
];
const EXTRA = JSON.stringify({ list_lens_windows: [{ id: 'lens-1', target_language: 'ja' }], refresh_lens: true });

/** ページ内計測：各選択肢の文字色・実効背景色・コントラスト比。 */
function measure() {
  const parse = (c) => {
    const m = c.match(/[\d.]+/g).map(Number);
    return { rgb: m.slice(0, 3), a: m.length > 3 ? m[3] : 1 };
  };
  const lum = ([r, g, b]) => {
    const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
  };
  const rows = [];
  for (const sel of document.querySelectorAll('select')) {
    // 配色設定の既定背景：選択欄と同一の color-scheme を継承する計測用要素で解決
    const probe = document.createElement('span');
    probe.style.backgroundColor = 'Field';
    sel.parentElement.appendChild(probe);
    const field = parse(getComputedStyle(probe).backgroundColor);
    probe.remove();
    const own = parse(getComputedStyle(sel).backgroundColor);
    for (const opt of sel.options) {
      const cs = getComputedStyle(opt);
      const bg = parse(cs.backgroundColor);
      const back = bg.a > 0 ? bg : own.a > 0 ? own : field;
      const fore = parse(cs.color);
      const [hi, lo] = [lum(fore.rgb), lum(back.rgb)].sort((a, b) => b - a);
      rows.push({
        select: sel.getAttribute('aria-label') ?? sel.name ?? sel.className,
        option: opt.textContent.trim(),
        color: cs.color,
        background: `rgb(${back.rgb.join(', ')})`,
        ratio: Math.round(((hi + 0.05) / (lo + 0.05)) * 100) / 100
      });
    }
  }
  return rows;
}

let bad = 0;
let total = 0;
for (const scheme of ['dark', 'light']) {
  for (const p of PAGES) {
    const ctx = await browser.newContext({ viewport: { width: p.width, height: p.height }, colorScheme: scheme });
    await ctx.addInitScript(init(p.label, MOCK, EXTRA));
    const pg = await ctx.newPage();
    await pg.goto(site.url(p.path));
    await wait(pg, 600);
    const rows = await pg.evaluate(measure);
    if (rows.length === 0) { console.log(`選択欄なし（状態へ未到達）：${p.path}`); bad += 1; }
    total += rows.length;
    const seen = new Set();
    for (const r of rows.filter((r) => r.ratio < MIN_RATIO)) {
      const key = `${r.select}|${r.color}|${r.background}`;
      if (seen.has(key)) continue;
      seen.add(key); bad += 1;
      console.log(`[${scheme}] ${p.path} 「${r.select}」：文字 ${r.color}／背景 ${r.background}／比 ${r.ratio}（例：${r.option}）`);
    }
    await ctx.close();
  }
}
await browser.close();
site.close();
console.log(bad === 0 ? `選択欄の可読性：違反なし（選択肢 ${total} 件）` : `選択欄の可読性：違反 ${bad} 件`);
process.exit(bad === 0 ? 0 : 1);
