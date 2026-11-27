/**
 * ホームの魔法陣メニュー：幾何計算（純関数）と描画。
 * 寸法は UI 第5案のモック（論理 640×368、外周ルーン環 半径 158）を基準に比例配分。
 * 選択中の項目は常に右上（-54 度）へ回転し、見出し帯と接続。
 * 描画は 3 種：静止層（drawDial：中心の月を含む）、動作層（drawDialMotion：光点・波紋・中心核の円）、
 * 回転体（drawRotor：ルーン環・中心核の正方形。CSS で等速回転）。
 * 動作層・回転体は静止層の下へ重ね、再描画しても文字の 2 値化処理を伴わない軽量構成。
 */
import { type Color, Pix } from './pix';
import { ICONS, MINI_RUNES, RUNES, type IconName } from './glyphs';

export interface DialItem {
  /** 魔法陣上の名札。 */
  label: string;
  /** 見出し帯の機能名。省略時は label。 */
  title?: string;
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
export const SIDE_WIDTH = 324;
export const NODE_HIT_RADIUS = 26;

/**
 * 論理解像度 w×h の配置。魔法陣は下端へはみ出す前提の大きさ。
 * selected は小数も可（回転演出の途中経過）。外周は情報欄（右端から reserve）の手前で完結。
 */
export function dialGeometry(w: number, h: number, count: number, selected: number, reserve = SIDE_WIDTH): DialGeometry {
  const free = Math.max(160, w - reserve);
  const r = Math.round(Math.min(h * 0.43, free * 0.44));
  const cx = Math.round(free * 0.5);
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

/** 座標に該当する項目番号。該当なしは null。 */
export function hitTest(geo: DialGeometry, x: number, y: number): number | null {
  let best: number | null = null;
  let bestD = NODE_HIT_RADIUS;
  geo.nodes.forEach((n, i) => {
    const d = Math.hypot(n.x - x, n.y - y);
    if (d <= bestD) {
      best = i;
      bestD = d;
    }
  });
  return best;
}

/** 項目名の配置：上部ノードは上、下部ノードは横、他は下。 */
export function labelPlacement(angle: number): 'above' | 'below' | 'left' | 'right' {
  const t = (angle * Math.PI) / 180;
  const s = Math.sin(t);
  if (s < -0.5) return 'above';
  if (s > 0.5) return Math.cos(t) >= 0 ? 'right' : 'left';
  return 'below';
}

type Box = { x: number; y: number; w: number; h: number };
const overlap = (a: Box, b: Box) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

function runeRing(p: Pix, cx: number, cy: number, r: number, n: number, set: readonly (readonly string[])[], c: Color) {
  const w = set[0][0].length,
    h = set[0].length;
  for (let i = 0; i < n; i++) {
    const t = ((i / n) * 360 - 90) * (Math.PI / 180);
    p.icon(set[i % set.length], Math.round(cx + r * Math.cos(t) - w / 2), Math.round(cy + r * Math.sin(t) - h / 2), c);
  }
}

/** 名札の高さ（論理画素）：文字 16 ＋上下 2（表示 4px）。 */
export const CHIP_H = 20;
/** 名札の標識（正方形）の一辺と、文字との間隔（論理画素。表示 16px・8px）。 */
const MARK = 8;
const MARK_GAP = 4;

/** 斜めの名札（背景つき）。戻り値は幅。mark 指定時は文字の左へ正方形の標識を描画。 */
export function chip(p: Pix, x: number, y: number, s: string, fg: Color, bg: Color, mark = false, pad = 8, k = 0.3) {
  const lead = mark ? MARK + MARK_GAP : 0;
  const w = p.textW(s) + lead + pad * 2;
  p.slab(x, y, w, CHIP_H, k, bg);
  if (mark) p.rect(x + pad + 3, y + (CHIP_H - MARK) / 2, MARK, MARK, fg);
  p.text(s, x + pad + 3 + lead, y + 2, fg);
  return w;
}

export interface DialState {
  items: DialItem[];
  selected: number;
  hover: number | null;
  /** 回転演出中の表示位置（小数可）。省略時は selected。 */
  turn?: number;
  /** 見出し帯の左端（論理座標）。選択ノードから接続線を描画。 */
  anchor?: { x: number; y: number };
}

const geometryOf = (p: Pix, s: DialState, reserve: number) =>
  dialGeometry(p.w, p.h, s.items.length, s.turn ?? s.selected, reserve);

/**
 * 回転体：外周・内周のルーン環と中心核の 2 枚の正方形。
 * 各回転体は専用の小型 Canvas へ 1 度だけ描画し、表示側の CSS 回転（線形・等速）で回す。
 * 毎フレームのドット再配置（ルーン単位の段階的な移動）を避け、滑らかな回転を実現。
 * degPerSec：回転速度（負値は反時計回り）、start：初期角度。
 */
export const ROTORS = [
  { id: 'outer', degPerSec: 3, start: 0 },
  { id: 'inner', degPerSec: -8, start: 6 },
  { id: 'core-a', degPerSec: 24, start: 0 },
  { id: 'core-b', degPerSec: -36, start: 45 }
] as const;
export type RotorId = (typeof ROTORS)[number]['id'];

const coreRadius = (q: number) => Math.round(34 * Math.min(1, q + 0.15));

/** 中心の月の半径（論理画素）。回転体（正方形）の頂点が枠環の外側へ残る寸法。 */
export const moonRadius = (r: number) => Math.round(coreRadius(r / 158) * 0.47);

/**
 * 月面の区分（アプリアイコンと同形）：左側が明部の三日月、残余は影。円外は null。
 * x・y は月の中心基準。
 */
export function moonPart(x: number, y: number, r: number): 'lit' | 'shade' | null {
  if (x * x + y * y > r * r + r) return null;
  const k = 0.42;
  return (x - r * k) ** 2 + (y + r * k * 0.55) ** 2 > (r * 0.92) ** 2 ? 'lit' : 'shade';
}

/** 中心の月：黒地の円盤と枠環、淡琥珀の三日月、網点の影。背後の星形の線は円盤で遮蔽。 */
function drawMoon(p: Pix, cx: number, cy: number, r: number) {
  p.fcircle(cx, cy, r + 3, 0);
  p.circle(cx, cy, r + 3, 5);
  for (let y = -r; y <= r; y++)
    for (let x = -r; x <= r; x++) {
      const part = moonPart(x, y, r);
      if (part === 'lit') p.px(cx + x, cy + y, 6);
      else if (part === 'shade') p.dither(cx + x, cy + y, 1, 1, 0, 3, 5 / 16);
    }
}


/** 回転体の Canvas 寸法（論理画素、奇数：中心画素を回転中心と一致）。 */
export function rotorSize(id: RotorId, r: number): number {
  const q = r / 158;
  const half = id === 'outer' ? r + 6 : id === 'inner' ? Math.round(r * 0.456) - 4 : coreRadius(q);
  return half * 2 + 1;
}

/** 回転体の描画（角度 0、Canvas 中心が回転中心）。p は rotorSize 四方。 */
export function drawRotor(p: Pix, id: RotorId, r: number) {
  const q = r / 158;
  const c = (p.w - 1) / 2;
  p.clear();
  if (id === 'outer') runeRing(p, c, c, r, Math.round(52 * q), RUNES, 5);
  else if (id === 'inner') runeRing(p, c, c, Math.round(r * 0.456) - 11, 30, MINI_RUNES, 4);
  else {
    const cr = coreRadius(q);
    const rad = id === 'core-a' ? cr - 6 : cr - 12;
    const pts = [0, 1, 2, 3].map((i) => {
      const a = (i * 90 * Math.PI) / 180;
      return [c + rad * Math.cos(a), c + rad * Math.sin(a)] as const;
    });
    pts.forEach((a, i) => p.line(a[0], a[1], pts[(i + 1) % 4][0], pts[(i + 1) % 4][1], id === 'core-a' ? 5 : 4));
  }
}

/**
 * 動作層：背景円・外周の光点・選択ノードの波紋・中心核の円と核。
 * ルーン環と正方形は回転体（drawRotor）として別層で回転。t は経過秒（静止表示は 0）。
 */
export function drawDialMotion(p: Pix, s: DialState, t: number, reserve = SIDE_WIDTH) {
  const geo = geometryOf(p, s, reserve);
  const { cx, cy, r } = geo;
  const q = r / 158;
  p.clear();
  p.fcircle(cx, cy, Math.round(r + 20 * q), 0);

  // 外周の光点：二重円の隙間を周回し、尾を引く
  const ro = r + 16 * q;
  const TAIL = [9, 6, 6, 5, 5, 4, 4, 3, 3];
  const head = t * 40;
  TAIL.forEach((c, k) => {
    const a = ((head - k * 1.4 - 90) * Math.PI) / 180;
    p.px(Math.round(cx + ro * Math.cos(a)), Math.round(cy + ro * Math.sin(a)), c);
  });

  // 中心核：円（正方形は回転体、月は静止層）
  const cr = coreRadius(q);
  p.circle(cx, cy, cr, 3);
  p.circle(cx, cy, cr - 3, 5, [1, 3]);

  // 選択ノードの波紋：菱形が外側へ広がり減光（回転演出中は省略）
  const sel = geo.nodes[s.selected];
  if (sel && s.turn === undefined) {
    const f = (t / 1.2) % 1;
    p.diamond(sel.x, sel.y, 31 + Math.round(f * 12), f < 0.5 ? 5 : 3, null, f > 0.66 ? [2, 2] : undefined);
  }
  return geo;
}

/** 静止層：目盛り・星形・中心の月・ノード・名札・接続線。動作層の上へ重ねる（背景は透明）。 */
export function drawDial(p: Pix, s: DialState, reserve = SIDE_WIDTH) {
  const geo = geometryOf(p, s, reserve);
  const { cx, cy, r } = geo;
  const q = r / 158; // モック比
  p.clear();

  // 外周：ルーン環の枠と目盛り（ルーン本体は動作層）
  p.circle(cx, cy, Math.round(r + 19 * q), 3, [1, 2]);
  p.circle(cx, cy, Math.round(r + 13 * q), 3);
  p.circle(cx, cy, Math.round(r + 10 * q), 5);
  p.circle(cx, cy, Math.round(r - 10 * q), 5);
  const tick = Math.round(r - 13 * q);
  p.circle(cx, cy, tick, 3);
  for (let i = 0; i < 72; i++) {
    const t = ((i * 5 - 90) * Math.PI) / 180,
      l = i % 6 === 0 ? 6 : 2;
    p.line(cx + tick * Math.cos(t), cy + tick * Math.sin(t), cx + (tick - l) * Math.cos(t), cy + (tick - l) * Math.sin(t), i % 6 === 0 ? 5 : 3);
  }

  // 四角形と対角線（項目数 4）・五芒星（5）・六芒星（6）・多角形（その他）
  const N = geo.nodes;
  const path = (ids: number[]) => ids.reduce((a, b) => (p.line(N[a].x, N[a].y, N[b].x, N[b].y, 4), b));
  if (N.length === 4) (path([0, 1, 2, 3, 0]), path([0, 2]), path([1, 3]));
  else if (N.length === 5) path([0, 2, 4, 1, 3, 0]);
  else if (N.length === 6) (path([0, 2, 4, 0]), path([1, 3, 5, 1]));
  else N.forEach((n, i) => p.line(n.x, n.y, N[(i + 1) % N.length].x, N[(i + 1) % N.length].y, 4));

  // 内周（小ルーンは動作層）
  const ri = Math.round(r * 0.456);
  p.circle(cx, cy, ri, 5);
  p.circle(cx, cy, ri - 3, 3);
  p.circle(cx, cy, ri - 19, 3);
  // 中心の月：星形の線と回転体（正方形）の上へ描画
  drawMoon(p, cx, cy, moonRadius(r));
  // 星の谷間の紋章
  N.forEach((n, i) => {
    const t = ((n.angle + 180 / N.length) * Math.PI) / 180;
    p.icon(RUNES[(i * 3) % RUNES.length], Math.round(cx + r * 0.595 * Math.cos(t) - 3), Math.round(cy + r * 0.595 * Math.sin(t) - 4), 4);
  });

  // 選択線：見出し帯へ 3 本の平行線、中心核から破線
  const sel = N[s.selected];
  if (sel && s.turn === undefined) {
    if (s.anchor) for (let d = -2; d <= 2; d += 2) p.line(sel.x + d, sel.y - 4 + d, s.anchor.x + d, s.anchor.y + d, d === 0 ? 9 : 5);
    const a = Math.atan2(sel.y - cy, sel.x - cx);
    p.line(cx + 40 * Math.cos(a), cy + 40 * Math.sin(a), sel.x - 34 * Math.cos(a), sel.y - 34 * Math.sin(a), 5, [3, 2]);
  }

  // ノード（本体を先に全描画、名札は重複回避の配置探索後に描画）
  const boxes: Box[] = N.map((n, i) => {
    const r = i === s.selected ? 31 : 22;
    return { x: n.x - r, y: n.y - r, w: r * 2, h: r * 2 };
  });
  boxes.push({ x: cx - ri, y: cy - ri, w: ri * 2, h: ri * 2 }); // 内周・中心核
  N.forEach((n, i) => i !== s.selected && boxes.push({ x: n.x + 13, y: n.y - 30, w: 10, h: 16 })); // 番号
  s.items.forEach((item, i) => {
    const { x, y } = N[i];
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
  });
  const limit = p.w - reserve + 6; // 情報欄へ食込まない右端
  s.items.forEach((item, i) => {
    if (i === s.selected) return;
    const { x, y, angle } = N[i];
    const hov = s.hover === i,
      lock = !!item.locked;
    p.text(String(i + 1), x + 13, y - 30, hov ? 6 : 3);
    const w = p.textW(item.label) + 16 + (lock ? 16 : 0);
    const at = (place: string): Box => {
      const lx = place === 'left' ? x - 24 - w : place === 'right' ? x + 24 : Math.round(x - w / 2);
      const ly = place === 'above' ? y - 46 : place === 'below' ? y + 26 : y - CHIP_H / 2;
      return { x: Math.max(2, Math.min(limit - w, lx)), y: Math.max(2, Math.min(p.h - CHIP_H - 2, ly)), w, h: CHIP_H };
    };
    // 既定の配置→下→上→左→右の順に、ノード・既存の名札と重ならない位置を採用
    const order = [labelPlacement(angle), 'below', 'above', 'left', 'right'];
    const hit = (b: Box) => boxes.some((o) => overlap(b, o));
    const box = order.map(at).find((b) => !hit(b)) ?? at(order[0]);
    boxes.push(box);
    chip(p, box.x, box.y, item.label, lock ? 3 : hov ? 0 : 5, hov ? 9 : 1);
    if (lock) {
      const ex = box.x + w - 12;
      p.rect(ex - 2, box.y + 4, 13, 13, 0);
      p.line(ex, box.y + 6, ex + 8, box.y + 14, 7);
      p.line(ex + 8, box.y + 6, ex, box.y + 14, 7);
    }
  });
  return geo;
}
