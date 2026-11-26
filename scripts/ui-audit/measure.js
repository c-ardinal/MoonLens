// ページ内で実行する計測関数。戻り値：違反の配列。
// 座標は表示上の px（CSS zoom 反映後）。4px 格子の許容誤差は 0.05px。
(() => {
  const EPS = 0.05;
  const on4 = (v) => Math.abs(v / 4 - Math.round(v / 4)) < EPS / 4;
  const r3 = (v) => Math.round(v * 1000) / 1000;
  const sig = (e) => {
    const c = typeof e.className === 'string' ? e.className.trim().split(/\s+/).filter((x) => x && !x.startsWith('svelte-')).slice(0, 5).join('.') : '';
    return e.tagName.toLowerCase() + (c ? '.' + c : '');
  };
  const path = (e) => { const p = []; for (let n = e, i = 0; n && n !== document.body && i < 3; n = n.parentElement, i++) p.unshift(sig(n)); return p.join(' > '); };
  const visible = (e) => {
    const s = getComputedStyle(e);
    if (s.display === 'none' || s.visibility === 'hidden' || e.classList.contains('sr-only')) return false;
    if (!e.offsetParent && s.position !== 'fixed') return false;
    const b = e.getBoundingClientRect();
    return b.width > 0 && b.height > 0;
  };
  const exempt = (e) => !!e.closest('[data-grid-exempt]');
  const out = [];
  const add = (rule, e, detail, value) => out.push({ rule, where: path(e), detail, value: r3(value) });
  const SYMBOL = /[←-⇿⌀-⏿■-◿☀-➿⬀-⯿×]/;
  const SPACING = ['paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft', 'marginTop', 'marginRight', 'marginBottom', 'marginLeft', 'rowGap', 'columnGap'];
  const kebab = (p) => p.replace(/[A-Z]/g, (c) => '-' + c.toLowerCase());
  const iconLike = (e) => /^(svg|canvas|img)$/i.test(e.tagName) || (!e.textContent.trim() && e.children.length === 0);

  for (const e of document.querySelectorAll('body *')) {
    if (e.closest('svg') && e.tagName.toLowerCase() !== 'svg') continue;
    if (!visible(e) || exempt(e)) continue;
    const s = getComputedStyle(e);
    const z = e.currentCSSZoom ?? 1;

    // 規則 1：余白（padding・margin・gap）と絶対配置の位置指定。疑似要素を含む
    for (const pseudo of [null, '::before', '::after']) {
      const ps = pseudo ? getComputedStyle(e, pseudo) : s;
      if (pseudo && (ps.content === 'none' || ps.content === 'normal' || ps.display === 'none')) continue;
      const tag = pseudo ?? '';
      for (const p of SPACING) {
        const v = parseFloat(ps[p]);
        if (!v || Number.isNaN(v)) continue;
        if (/Gap/.test(p) && !/(flex|grid)/.test(ps.display)) continue;
        if (!pseudo && /^margin/.test(p) && String(e.computedStyleMap().get(kebab(p))) === 'auto') continue;
        if (!on4(v * z)) add('spacing', e, tag + p, v * z);
      }
      if (ps.position === 'absolute' || ps.position === 'fixed') {
        for (const p of ['top', 'right', 'bottom', 'left']) {
          const raw = pseudo ? ps[p] : String(e.computedStyleMap().get(p));
          if (!/px$/.test(raw)) continue;
          const v = parseFloat(ps[p]);
          if (!pseudo && (e.style[p] || e.closest('[class*="absolute"]') === e && Math.abs(v) <= 2)) continue;
          if (Math.abs(v) <= 2) continue;
          if (v && !on4(v * z)) add('offset', e, tag + p, v * z);
        }
      }
      if (pseudo && SYMBOL.test(ps.content) && !e.closest('[data-grid-glyph]')) add('glyph-icon', e, tag + ' ' + ps.content, 0);
    }

    // 子要素・文字列を「項目」として収集
    const items = [];
    for (const n of e.childNodes) {
      if (n.nodeType === 3) {
        if (!n.textContent.trim()) continue;
        const rg = document.createRange(); rg.selectNodeContents(n);
        // 前後の空白を除外した範囲
        const t = n.textContent; const a = t.length - t.trimStart().length; const b = t.trimEnd().length;
        rg.setStart(n, a); rg.setEnd(n, b);
        for (const q of rg.getClientRects()) if (q.width > 0) items.push({ kind: 'text', text: t.trim(), l: q.left, r: q.right, t: q.top, b: q.bottom });
        if (SYMBOL.test(t) && !e.closest('[data-grid-glyph]')) add('glyph-icon', e, t.trim().slice(0, 24), 0);
      } else if (n.nodeType === 1 && visible(n) && !exempt(n)) {
        const cs = getComputedStyle(n);
        if (cs.position === 'absolute' || cs.position === 'fixed') continue;
        const rects = cs.display === 'inline' ? [...n.getClientRects()] : [n.getBoundingClientRect()];
        for (const q of rects) if (q.width > 0) items.push({ kind: iconLike(n) ? 'icon' : 'box', el: n, inline: cs.display.startsWith('inline'), l: q.left, r: q.right, t: q.top, b: q.bottom });
      }
    }
    if (items.length < 2) continue;
    const flex = /flex|grid/.test(s.display);
    const row = flex ? (/flex/.test(s.display) ? s.flexDirection.startsWith('row') : true) : true;
    // 行へ分割
    items.sort((a, b) => a.t - b.t || a.l - b.l);
    const lines = [];
    for (const it of items) {
      const ln = lines.find((L) => Math.min(L.b, it.b) - Math.max(L.t, it.t) > Math.min(L.b - L.t, it.b - it.t) * 0.5);
      if (ln) { ln.items.push(it); ln.t = Math.min(ln.t, it.t); ln.b = Math.max(ln.b, it.b); } else lines.push({ t: it.t, b: it.b, items: [it] });
    }
    // 規則 6：縦積みの要素間隔
    if (!flex || !row) {
      const blocks = lines.filter((L) => L.items.every((x) => x.kind !== 'text' && !x.inline));
      for (let i = 1; i < lines.length; i++) {
        const a = lines[i - 1], b = lines[i];
        if (!blocks.includes(a) || !blocks.includes(b)) continue;
        const gap = b.t - a.b;
        if (gap > -0.5 && !on4(gap)) add('block-gap', e, `${sig(a.items[0].el)} / ${sig(b.items[0].el)}`, gap);
      }
    }
    const name = (it) => (it.kind === 'text' ? `"${it.text.slice(0, 8)}"` : sig(it.el));
    for (const L of lines) {
      if (L.items.length < 2) continue;
      L.items.sort((a, b) => a.l - b.l);
      const inlineFlow = !flex;
      for (let i = 1; i < L.items.length; i++) {
        const a = L.items[i - 1], b = L.items[i];
        const gap = b.l - a.r;
        if (gap < -0.5) continue;
        const pair = `${name(a)} | ${name(b)}`;
        // 規則 2：行内配置の項目間隔（空白・字間由来を含む）
        if (inlineFlow && !(a.kind === 'text' && b.kind === 'text') && !on4(gap)) add('inline-gap', e, pair, gap);
        // 規則 3：図標と文字の密着
        if ((a.kind === 'icon') !== (b.kind === 'icon') && gap < 4 - EPS && (a.kind === 'text' || b.kind === 'text' || (a.el ?? b.el).textContent.trim())) add('icon-touch', e, pair, gap);
      }
      // 規則 4：行内の垂直中央
      const centred = flex ? (row && /center/.test(s.alignItems)) : true;
      if (centred) {
        const ref = L.items.find((x) => x.kind === 'text') ?? L.items.reduce((m, x) => (x.b - x.t > m.b - m.t ? x : m));
        const rc = (ref.t + ref.b) / 2;
        for (const it of L.items) {
          if (it === ref) continue;
          if (!flex && it.kind === 'box' && !it.inline) continue;
          const d = (it.t + it.b) / 2 - rc;
          if (Math.abs(d) > 1 + EPS) add('v-centre', e, `${name(it)} 対 ${name(ref)}`, d);
        }
      }
    }
  }

  // 規則 5：枠付き要素内の文字の上下偏り
  for (const e of document.querySelectorAll('body *')) {
    if (e.closest('svg') || !visible(e) || exempt(e)) continue;
    const s = getComputedStyle(e);
    const boxed = s.backgroundColor !== 'rgba(0, 0, 0, 0)' || parseFloat(s.borderTopWidth) > 0 || /^(BUTTON|SELECT|INPUT|A)$/.test(e.tagName);
    if (!boxed) continue;
    const tn = [...e.childNodes].filter((n) => n.nodeType === 3 && n.textContent.trim());
    if (!tn.length) continue;
    const rg = document.createRange(); rg.selectNodeContents(tn[0]);
    const rects = rg.getClientRects(); if (rects.length !== 1) continue;
    const t = rects[0], b = e.getBoundingClientRect();
    if (b.height > t.height * 2.2) continue;
    const d = (t.top - b.top) - (b.bottom - t.bottom);
    if (Math.abs(d) > 2 + EPS) add('box-centre', e, `"${tn[0].textContent.trim().slice(0, 8)}" 上下差`, d);
  }
  return out;
})()
