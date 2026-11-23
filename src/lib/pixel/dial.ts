/**
 * ホームの魔法陣メニュー：幾何計算（純関数）と描画。
 * 寸法は UI 第5案のモック（論理 640×368、外周ルーン環 半径 158）を基準に比例配分。
 * 選択中の項目は常に右上（-54 度）へ回転し、見出し帯と接続。
 */
import { type Color, Pix } from './pix';
import { ICONS, MINI_RUNES, RUNES, type IconName } from './glyphs';

export interface DialItem {
  label: string;
  icon: IconName;
  /** 実行不可。破線枠と赤い × 印で表示。 */
  locked?: boolean;
}

export interface DialGeometry {
  cx: number;
  cy: number;
  /** 外周ルーン環の半径。 */
  r: number;
  /** 各項目ノードの中心（items と同順）。 */
  nodes: { x: number; y: number; angle: number }[];
}

/** 選択項目の向き（度）。右上の見出し帯側。 */
export const SELECTED_ANGLE = -54;
/** 右側の情報欄の幅（論理画素）。 */
export const SIDE_WIDTH = 310;
export const NODE_HIT_RADIUS = 26;
const CENTER_HIT_RADIUS = 38;

/** 論理解像度 w×h の配置。魔法陣は左下へはみ出す前提の大きさ。 */
export function dialGeometry(w: number, h: number, count: number, selected: number, reserve = SIDE_WIDTH): DialGeometry {
  const free = Math.max(160, w - reserve);
  const r = Math.round(Math.min(h * 0.43, free * 0.48));
  const cx = Math.round(free * 0.53);
  const cy = Math.round(h * 0.6);
  const step = 360 / Math.max(1, count);
  const rn = r * 0.76;
  const nodes = Array.from({ length: count }, (_, i) => {
    const angle = SELECTED_ANGLE + (i - selected) * step;
    const t = (angle * Math.PI) / 180;
    return { x: Math.round(cx + rn * Math.cos(t)), y: Math.round(cy + rn * Math.sin(t)), angle };
  });
  return { cx, cy, r, nodes };
}

/** 座標に該当する項目番号。中心の決定ボタンは 'center'、該当なしは null。 */
export function hitTest(geo: DialGeometry, x: number, y: number): number | 'center' | null {
  let best: number | null = null;
  let bestD = NODE_HIT_RADIUS;
  geo.nodes.forEach((n, i) => {
    const d = Math.hypot(n.x - x, n.y - y);
    if (d <= bestD) {
      best = i;
      bestD = d;
    }
  });
  if (best !== null) return best;
  return Math.hypot(geo.cx - x, geo.cy - y) <= CENTER_HIT_RADIUS ? 'center' : null;
}

/** 項目名の配置：上部ノードは上、下部ノードは横、他は下。 */
export function labelPlacement(angle: number): 'above' | 'below' | 'left' | 'right' {
  const t = (angle * Math.PI) / 180;
  const s = Math.sin(t);
  if (s < -0.5) return 'above';
  if (s > 0.5) return Math.cos(t) >= 0 ? 'right' : 'left';
  return 'below';
}

function runeRing(p: Pix, cx: number, cy: number, r: number, n: number, set: readonly (readonly string[])[], c: Color, offDeg = 0) {
  const w = set[0][0].length,
    h = set[0].length;
  for (let i = 0; i < n; i++) {
    const t = ((i / n) * 360 - 90 + offDeg) * (Math.PI / 180);
    p.icon(set[i % set.length], Math.round(cx + r * Math.cos(t) - w / 2), Math.round(cy + r * Math.sin(t) - h / 2), c);
  }
}

/** 斜めの名札（背景つき）。戻り値は幅。 */
export function chip(p: Pix, x: number, y: number, s: string, fg: Color, bg: Color, pad = 8, k = 0.3) {
  const w = p.textW(s) + pad * 2;
  p.slab(x, y, w, 19, k, bg);
  p.text(s, x + pad + 3, y + 1, fg);
  return w;
}

export interface DialState {
  items: DialItem[];
  selected: number;
  hover: number | 'center' | null;
  /** 見出し帯の左端（論理座標）。選択ノードから接続線を描画。 */
  anchor?: { x: number; y: number };
}

export function drawDial(p: Pix, s: DialState, reserve = SIDE_WIDTH) {
  const geo = dialGeometry(p.w, p.h, s.items.length, s.selected, reserve);
  const { cx, cy, r } = geo;
  const q = r / 158; // モック比

  // 外周：ルーン環と目盛り
  p.fcircle(cx, cy, Math.round(r + 20 * q), 0);
  p.circle(cx, cy, Math.round(r + 19 * q), 3, [1, 2]);
  p.circle(cx, cy, Math.round(r + 13 * q), 3);
  p.circle(cx, cy, Math.round(r + 10 * q), 5);
  runeRing(p, cx, cy, r, Math.round(52 * q), RUNES, 5);
  p.circle(cx, cy, Math.round(r - 10 * q), 5);
  const tick = Math.round(r - 13 * q);
  p.circle(cx, cy, tick, 3);
  for (let i = 0; i < 72; i++) {
    const t = ((i * 5 - 90) * Math.PI) / 180,
      l = i % 6 === 0 ? 6 : 2;
    p.line(cx + tick * Math.cos(t), cy + tick * Math.sin(t), cx + (tick - l) * Math.cos(t), cy + (tick - l) * Math.sin(t), i % 6 === 0 ? 5 : 3);
  }

  // 五芒星（項目数 5）・多角形（その他）
  const N = geo.nodes;
  if (N.length === 5) [0, 2, 4, 1, 3, 0].reduce((a, b) => (p.line(N[a].x, N[a].y, N[b].x, N[b].y, 4), b));
  else N.forEach((n, i) => p.line(n.x, n.y, N[(i + 1) % N.length].x, N[(i + 1) % N.length].y, 4));

  // 内周と小ルーン
  const ri = Math.round(r * 0.456);
  p.circle(cx, cy, ri, 5);
  p.circle(cx, cy, ri - 3, 3);
  runeRing(p, cx, cy, ri - 11, 30, MINI_RUNES, 4, 6);
  p.circle(cx, cy, ri - 19, 3);
  // 星の谷間の紋章
  N.forEach((n, i) => {
    const t = ((n.angle + 180 / N.length) * Math.PI) / 180;
    p.icon(RUNES[(i * 3) % RUNES.length], Math.round(cx + r * 0.595 * Math.cos(t) - 3), Math.round(cy + r * 0.595 * Math.sin(t) - 4), 4);
  });

  // 中心の決定
  const hc = s.hover === 'center';
  p.diamond(cx, cy, 38, hc ? 9 : 5, 0);
  p.diamond(cx, cy, 34, 3);
  p.text('決定', cx, cy - 22, 9, { align: 'center' });
  p.text('Enter', cx, cy + 3, 4, { align: 'center' });

  // 選択線：見出し帯へ 3 本の平行線、中心から破線
  const sel = N[s.selected];
  if (sel) {
    if (s.anchor) for (let d = -2; d <= 2; d += 2) p.line(sel.x + d, sel.y - 4 + d, s.anchor.x + d, s.anchor.y + d, d === 0 ? 9 : 5);
    p.line(cx + 26, cy - 26, sel.x - 14, sel.y + 14, 5, [3, 2]);
  }

  // ノード
  s.items.forEach((item, i) => {
    const { x, y, angle } = N[i];
    const on = i === s.selected,
      hov = s.hover === i,
      lock = !!item.locked;
    if (on) {
      p.diamond(x, y, 30, 9, 1);
      p.diamond(x, y, 25, 5, 5);
    } else {
      p.diamond(x, y, 21, hov ? 9 : lock ? 3 : 5, hov ? 2 : 0, lock ? [2, 2] : undefined);
      if (!lock) p.diamond(x, y, 17, 3);
    }
    p.icon(ICONS[item.icon], x - 8, y - 8, on ? 0 : lock ? 3 : hov ? 9 : 5, 2);
    if (on) return;
    p.text(String(i + 1), x + 13, y - 30, hov ? 6 : 3);
    const w = p.textW(item.label) + 16;
    const place = labelPlacement(angle);
    let lx = place === 'left' ? x - 24 - w : place === 'right' ? x + 24 : Math.round(x - w / 2);
    const ly = place === 'above' ? y - 46 : place === 'below' ? y + 26 : y - 9;
    lx = Math.max(2, Math.min(p.w - w - 14, lx));
    chip(p, lx, ly, item.label, lock ? 3 : hov ? 0 : 5, hov ? 9 : 1);
    if (lock) {
      const ex = lx + w + 4;
      p.rect(ex - 2, ly + 3, 13, 13, 0);
      p.line(ex, ly + 5, ex + 8, ly + 13, 7);
      p.line(ex + 8, ly + 5, ex, ly + 13, 7);
    }
  });
  return geo;
}
