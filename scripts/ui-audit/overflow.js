// ページ内で実行する計測関数（画面超過の監査）。戻り値：違反の配列。座標は表示上の px。
(() => {
  const TOL = 1;
  const r1 = (v) => Math.round(v * 10) / 10;
  const sig = (e) => {
    const c = typeof e.className === 'string' ? e.className.trim().split(/\s+/).filter((x) => x && !x.startsWith('svelte-')).slice(0, 4).join('.') : '';
    return e.tagName.toLowerCase() + (c ? '.' + c : '');
  };
  const path = (e) => { const p = []; for (let n = e, i = 0; n && n !== document.body && i < 3; n = n.parentElement, i++) p.unshift(sig(n)); return p.join(' > '); };
  const visible = (e) => {
    const s = getComputedStyle(e);
    if (s.display === 'none' || s.visibility === 'hidden' || e.classList.contains('sr-only') || e.closest('.sr-only')) return false;
    if (!e.offsetParent && s.position !== 'fixed') return false;
    const b = e.getBoundingClientRect();
    return b.width > 0 && b.height > 0;
  };
  const out = [];
  const add = (rule, e, detail, value) => out.push({ rule, where: path(e), detail, value: r1(value) });
  const vw = window.innerWidth, vh = window.innerHeight;
  const clips = (v) => v !== 'visible';
  /** 祖先の切抜き・スクロール領域を反映した可視範囲。不可視（一覧のスクロール外の行等）は null。 */
  const shownRect = (e) => {
    const b = e.getBoundingClientRect();
    const v = { left: b.left, right: b.right, top: b.top, bottom: b.bottom };
    for (let n = e.parentElement; n && n !== document.documentElement; n = n.parentElement) {
      const s = getComputedStyle(n), q = n.getBoundingClientRect();
      if (clips(s.overflowX)) { v.left = Math.max(v.left, q.left); v.right = Math.min(v.right, q.right); }
      if (clips(s.overflowY)) { v.top = Math.max(v.top, q.top); v.bottom = Math.min(v.bottom, q.bottom); }
    }
    return v.right - v.left > TOL && v.bottom - v.top > TOL ? v : null;
  };
  /** 縦スクロール可能な祖先。 */
  const scroller = (e) => { for (let n = e.parentElement; n; n = n.parentElement) if (/auto|scroll/.test(getComputedStyle(n).overflowY)) return n; return null; };

  for (const e of document.querySelectorAll('body *')) {
    if (e.closest('svg') && e.tagName.toLowerCase() !== 'svg') continue;
    if (!visible(e) || e.closest('[data-overflow-exempt]')) continue;
    const s = getComputedStyle(e), b = e.getBoundingClientRect();

    // 規則 1：横スクロールの発生（スクロール領域の内容幅が表示幅を超過）
    if (/auto|scroll/.test(s.overflowX) && e.scrollWidth > e.clientWidth + TOL) add('h-scroll', e, '内容幅 − 表示幅', e.scrollWidth - e.clientWidth);

    // 規則 2：内容の欠落（切抜き指定の要素から内容が超過。省略記号・行数制限の指定は対象外）
    const ellipsis = s.textOverflow === 'ellipsis' || s.webkitLineClamp !== 'none';
    if (/hidden|clip/.test(s.overflowX) && !ellipsis && e.scrollWidth > e.clientWidth + TOL) add('clipped', e, '横の欠落幅', e.scrollWidth - e.clientWidth);
    if (/hidden|clip/.test(s.overflowY) && !ellipsis && e.scrollHeight > e.clientHeight + TOL && e.children.length === 0) add('clipped', e, '縦の欠落高', e.scrollHeight - e.clientHeight);

    const v = shownRect(e);
    if (!v) continue;
    // 規則 3：ウィンドウ外への突出（横方向。縦方向はスクロール領域外の要素のみ）
    if (v.right > vw + TOL) add('out-of-view', e, '右端の突出', v.right - vw);
    if (v.left < -TOL) add('out-of-view', e, '左端の突出', -v.left);
    if (!scroller(e) && v.bottom > vh + TOL) add('out-of-view', e, '下端の突出', v.bottom - vh);

    // 規則 4：同一階層の要素の重複（折返し・縮小による文字の重畳）
    const flow = [...e.children].filter((c) => visible(c) && !/absolute|fixed/.test(getComputedStyle(c).position) && c.textContent.trim());
    for (let i = 0; i < flow.length; i++) for (let j = i + 1; j < flow.length; j++) {
      const p = flow[i].getBoundingClientRect(), q = flow[j].getBoundingClientRect();
      const w = Math.min(p.right, q.right) - Math.max(p.left, q.left), h = Math.min(p.bottom, q.bottom) - Math.max(p.top, q.top);
      if (w > 2 && h > 2) add('overlap', e, `${sig(flow[i])} × ${sig(flow[j])}`, Math.min(w, h));
    }

    // 規則 5：枠内の文字の突出（文字列が親の枠より外へ描画）
    if (e.children.length === 0 && e.textContent.trim() && e.parentElement) {
      const rg = document.createRange(); rg.selectNodeContents(e);
      const t = rg.getBoundingClientRect(), q = e.parentElement.getBoundingClientRect();
      // 折返し行の行末の空白は枠外へ突出する仕様（hanging）：半角 1 字分を許容
      const tol = rg.getClientRects().length > 1 ? parseFloat(s.fontSize) * (e.currentCSSZoom ?? 1) * 0.5 + TOL : TOL;
      if (!ellipsis && s.overflowX === 'visible' && (t.right > q.right + tol || t.left < q.left - tol) && s.position !== 'absolute') add('text-out', e, `「${e.textContent.trim().slice(0, 10)}」`, Math.max(t.right - q.right, q.left - t.left));
    }
  }

  // 規則 6：画面全体を 1 画面へ収容する状態（data-fit-viewport）は縦スクロール不可
  for (const e of document.querySelectorAll('[data-fit-viewport]')) if (e.scrollHeight > e.clientHeight + TOL) add('v-scroll', e, '内容高 − 表示高', e.scrollHeight - e.clientHeight);
  return out;
})()
