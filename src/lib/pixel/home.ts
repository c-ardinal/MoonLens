/**
 * ホーム画面の描画（UI 第5案のモック準拠）。
 * 左：ルーン魔法陣、右：斜めの見出し帯・説明欄・エンジン欄・最近の翻訳。
 * 右側は画面右端基準、最近の翻訳と案内行は下端基準。縦の余白は高さに応じて圧縮。
 */
import { Pix, TITLE_FONT_FAMILY, TRI_H, TRI_W } from './pix';
import { chip, drawDial, drawDialMotion, type DialItem } from './dial';
import type { EngineState } from '../engine-status';

const K = 0.3;
/** 文字と三角の間隔（論理 4 ドット＝表示 8px）。 */
const TRI_GAP = 4;
/** 文字 s（描画原点 y）の字面の垂直中心へ三角を揃える描画位置。 */
const triY = (p: Pix, s: string, y: number) => Math.round(y + p.inkMidY(s) - TRI_H / 2);

/** 配置に必要な最小の論理寸法。これ未満はドットを縮小（homeDot）。 */
export const HOME_MIN = { w: 560, h: 318 } as const;
/** 案内行を表示する最小の高さ。未満は案内行を省略し、最近の翻訳を下端へ移動。 */
const HINT_MIN_H = 344;
/** 見出し帯の機能名の最大幅（論理画素）。右端の番号表示と非重複。 */
const TITLE_ROOM = 244;
/** 機能名の縦送りの移動量（論理画素）。帯の高さと同値：退場側の消失と入場側の出現が帯の端で一致。 */
const TITLE_ROLL = 62;
/** 右端の番号表示（分数）の占有幅（論理画素）。機能名の切抜き範囲から除外。 */
const NUMBER_ROOM = 44;

/**
 * 表示領域（CSS px）に対するドット 1 個の CSS 寸法。
 * 端末画素の整数倍（最大 2 倍表示）から、最小の論理寸法を満たす最大値を選択。
 * 例：125% 表示は 2 端末画素 = 1.6 CSS px、低い画面は 1 段階縮小。
 */
export function homeDot(cssW: number, cssH: number, dpr: number): number {
  const max = Math.max(1, Math.floor(2 * dpr + 1e-6));
  for (let d = max; d > 1; d--)
    if (Math.floor((cssW * dpr) / d) >= HOME_MIN.w && Math.floor((cssH * dpr) / d) >= HOME_MIN.h) return d / dpr;
  return 1 / dpr;
}

export interface HomeView {
  items: DialItem[];
  selected: number;
  hover: number | null;
  hotkey: string;
  running: boolean;
  desc: string;
  preview: string | null;
  error: string | null;
  engines: { label: string; model: string; state: EngineState }[];
  recent: { text: string; time: string } | null;
  hint: string;
  /** 押下中・ホバー中の領域。 */
  hot: HomeRegion | null;
  /** 選択切替の演出：魔法陣の表示位置（小数）と機能名の縦送りの進行度（0〜1）。 */
  turn?: number;
  slide?: number;
  /** 退場側の機能名：項目番号と開始時の縦位置（論理画素、定位置は 0）。 */
  leave?: { index: number; from: number };
}

/**
 * 機能名の縦送りの位置（論理画素、定位置は 0）。dir は魔法陣の回転方向（1：下から入場、-1：上から入場）。
 * 入場側と退場側は同方向へ同時に等速移動（12 fps の各コマで退場側を視認可能）。退場側は leaveFrom から帯の外まで。
 */
export function titleOffsets(slide: number, dir: 1 | -1, leaveFrom?: number): { enter: number; leave: number | null } {
  const rest = 1 - slide;
  const enter = Math.round(rest * TITLE_ROLL) * dir;
  if (leaveFrom === undefined) return { enter, leave: null };
  const out = -dir * TITLE_ROLL;
  // 端数処理は方向に対し対称
  return { enter, leave: Math.round((out + (leaveFrom - out) * rest) * dir) * dir };
}

export type HomeRegion = 'go' | 'engines' | 'recent';
export type Rect = { x: number; y: number; w: number; h: number };

/** 情報欄のウィンドウと右端の余白（見出し帯のみ右端まで延長する演出）。 */
const RIGHT_MARGIN = 14;
/** 情報欄（名札・説明欄・エンジン欄・最近の翻訳）の共通の左端と幅（mx 基準）。 */
const COL_X = 330;
const COL_W = 296;

/** 右側の情報欄の縦位置。高さ不足時は上端の余白を圧縮。 */
function column(h: number) {
  // 偶数へ切捨て：各段の位置を表示 4px の格子へ整列
  const top = Math.max(6, Math.min(32, h - (h >= HINT_MIN_H ? 337 : 311))) & ~1;
  return { top, chips: top + 74, desc: top + 102, engines: top + 180 };
}

/** 右側の情報欄の各領域（論理座標）。 */
export function homeRegions(w: number, h: number): Record<HomeRegion, Rect> {
  const mx = w - 640 - RIGHT_MARGIN;
  const c = column(h);
  return {
    go: { x: mx + 480, y: c.desc + 43, w: 146, h: 22 },
    engines: { x: mx + COL_X, y: c.engines, w: COL_W, h: 94 },
    recent: { x: mx + COL_X, y: h - (h >= HINT_MIN_H ? 52 : 26), w: COL_W, h: 20 }
  };
}

const STATE_TEXT: Record<EngineState, [string, number]> = {
  ready: ['準備完了', 8],
  missing: ['未導入', 7],
  downloading: ['取得中', 4],
  unset: ['未設定', 3]
};

/** 動作層（魔法陣の回転要素）。t は経過秒。 */
export function drawHomeMotion(p: Pix, v: Pick<HomeView, 'items' | 'selected' | 'turn'>, t: number) {
  drawDialMotion(p, { items: v.items, selected: v.selected, hover: null, turn: v.turn }, t);
}

/** 静止層（文字・枠・ノード）。背景は透明、動作層の上へ重ねる。 */
export function drawHome(p: Pix, v: HomeView) {
  const { w, h } = p;
  const dx = w - 640;
  const mx = dx - RIGHT_MARGIN;
  const c = column(h);
  const R = homeRegions(w, h);

  drawDial(p, { items: v.items, selected: v.selected, hover: v.hover, turn: v.turn, anchor: { x: dx + 330, y: c.top + 48 } });

  // 見出し帯：骨白の段影＋琥珀の帯＋斜体の機能名。帯と番号表示は固定。
  // 切替時は機能名のみ帯の内側で縦送り（魔法陣の回転方向に一致、後方の項目は下から）。
  // 切替前の機能名は同方向へ退場。横移動なし：右端の番号表示（分数）との干渉を回避。
  const dir = v.turn !== undefined && v.selected < v.turn ? -1 : 1;
  const off = titleOffsets(v.slide ?? 1, dir, v.leave?.from);
  p.slab(dx + 300, c.top + 8, 360, 62, K, 9);
  p.slab(dx + 312, c.top, 340, 62, K, 5);
  // 機能名：番号表示（右端）の手前へ収まる寸法へ縮小（最大 44px）
  const drawTitle = (item: DialItem | undefined, dy: number) => {
    const title = item?.title ?? item?.label ?? '';
    const fit = Math.min(44, Math.floor((44 * TITLE_ROOM) / Math.max(1, p.textW(title, `700 44px ${TITLE_FONT_FAMILY}`))));
    const y = c.top + 6 + Math.round((44 - fit) * 0.6);
    p.text(title, dx + 342, y + dy, 0, { k: K, font: `700 ${fit}px ${TITLE_FONT_FAMILY}` });
  };
  if (off.enter === 0 && off.leave === null) drawTitle(v.items[v.selected], 0);
  else {
    // 帯の高さ・番号表示の手前で切抜き、帯の外へ文字を出さない
    const g = p.g;
    g.save();
    g.beginPath();
    g.rect(dx + 312, c.top, w - NUMBER_ROOM - (dx + 312), 62);
    g.clip();
    if (off.leave !== null && v.leave) drawTitle(v.items[v.leave.index], off.leave);
    drawTitle(v.items[v.selected], off.enter);
    g.restore();
  }
  const n = String(v.selected + 1).padStart(2, '0'),
    total = String(v.items.length).padStart(2, '0');
  p.text(n, w - 12, c.top + 2, 0, { align: 'right' });
  p.hl(w - 40, c.top + 20, 28, 0);
  p.text(total, w - 12, c.top + 22, 2, { align: 'right' });

  // 名札：ショートカット・動作状態
  let x = mx + COL_X;
  if (v.hotkey) x += chip(p, x, c.chips, v.hotkey, 0, 9) + 4;
  if (v.running) chip(p, x, c.chips, '動作中', 8, 1, true);

  // 説明欄
  p.slantWin(mx + COL_X, c.desc, COL_W, 70, 0.2, { shadow: 2 });
  let ty = p.wrap(v.desc, mx + 352, c.desc + 10, 260, 6, 18, v.error || v.preview ? 1 : 2);
  if (v.error) ty = p.wrap(v.error, mx + 352, ty, 260, 7, 18, 1);
  else if (v.preview) p.wrap(v.preview, mx + 352, ty, 260, 3, 18, 1);
  const go = `Enter で${v.running ? '停止' : '開始'}`;
  const goCol = v.hot === 'go' ? 9 : 5;
  const goW = p.text(go, mx + 612, c.desc + 48, goCol, { align: 'right' });
  p.tri(mx + 612 - goW - TRI_GAP - TRI_W, triY(p, go, c.desc + 48), goCol);

  // エンジン欄
  const E = R.engines;
  p.slantWin(E.x, E.y + 8, E.w, 86, 0.2, { shadow: 2 });
  chip(p, E.x + 14, E.y, 'エンジン', 0, v.hot === 'engines' ? 9 : 5);
  // 種別・モデル・状態の 3 列：列頭を行間で統一し、モデル名は状態列の手前で省略
  const rows = v.engines.slice(0, 4);
  const labelW = Math.max(0, ...rows.map((r) => p.textW(r.label)));
  const stateW = Math.max(0, ...rows.map((r) => p.textW(STATE_TEXT[r.state][0])));
  const modelX = E.x + 22 + labelW + 8;
  const stateR = E.x + E.w - 10;
  rows.forEach((r, i) => {
    const [label, col] = STATE_TEXT[r.state];
    const y = E.y + 20 + i * 18;
    p.text(r.label, E.x + 22, y, 4);
    p.text(p.fit(r.model, stateR - stateW - 8 - modelX), modelX, y, 5);
    p.text(label, stateR, y, col, { align: 'right' });
  });

  // 最近の翻訳（下端基準）
  const Rr = R.recent;
  p.slab(Rr.x, Rr.y, Rr.w, 20, K, 1);
  p.text('最近', Rr.x + 16, Rr.y + 2, 3);
  // 右端から「履歴」＋三角・時刻の順に右寄せ、本文は残りの幅で省略
  const linkR = Rr.x + Rr.w - 4;
  const linkCol = v.hot === 'recent' ? 9 : 5;
  p.tri(linkR - TRI_W, triY(p, '履歴', Rr.y + 2), linkCol);
  const linkW = p.text('履歴', linkR - TRI_W - TRI_GAP, Rr.y + 2, linkCol, { align: 'right' }) + TRI_GAP + TRI_W;
  if (v.recent) {
    const timeR = linkR - linkW - 12;
    const timeW = p.text(v.recent.time, timeR, Rr.y + 2, 3, { align: 'right' });
    p.text(p.fit(v.recent.text, timeR - timeW - 12 - (Rr.x + 54)), Rr.x + 54, Rr.y + 2, 6);
  } else p.text('履歴なし', Rr.x + 54, Rr.y + 2, 3);

  // 案内行：情報欄の右端（説明欄の右下角）へ揃える
  if (h >= HINT_MIN_H) p.text(p.fit(v.hint, COL_W + 20), mx + COL_X + COL_W, h - 20, 3, { align: 'right' });
}
