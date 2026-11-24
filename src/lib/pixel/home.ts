/**
 * ホーム画面の描画（UI 第5案のモック準拠）。
 * 左：ルーン魔法陣、右：斜めの見出し帯・説明窓・エンジン窓・最近の翻訳。
 * 右側は画面右端基準、最近の翻訳と案内行は下端基準。縦の余白は高さに応じて圧縮。
 */
import { Pix, TITLE_FONT_FAMILY } from './pix';
import { chip, drawDial, drawDialMotion, type DialItem } from './dial';
import type { EngineState } from '../engine-status';

const K = 0.3;

/** 配置に必要な最小の論理寸法。これ未満はドットを縮小（homeDot）。 */
export const HOME_MIN = { w: 560, h: 318 } as const;
/** 案内行を表示する最小の高さ。未満は案内行を省略し、最近の翻訳を下端へ移動。 */
const HINT_MIN_H = 344;

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
  /** 選択切替の演出：魔法陣の表示位置（小数）と見出し帯の進行度（0〜1）。 */
  turn?: number;
  slide?: number;
}

export type HomeRegion = 'go' | 'engines' | 'recent';
export type Rect = { x: number; y: number; w: number; h: number };

/** 情報欄の窓と右端の余白（見出し帯のみ右端まで延長する演出）。 */
const RIGHT_MARGIN = 14;

/** 右側の情報欄の縦位置。高さ不足時は上端の余白を圧縮。 */
function column(h: number) {
  const top = Math.max(6, Math.min(32, h - (h >= HINT_MIN_H ? 337 : 311)));
  return { top, chips: top + 74, desc: top + 102, engines: top + 180 };
}

/** 右側の情報欄の各領域（論理座標）。 */
export function homeRegions(w: number, h: number): Record<HomeRegion, Rect> {
  const mx = w - 640 - RIGHT_MARGIN;
  const c = column(h);
  return {
    go: { x: mx + 480, y: c.desc + 42, w: 146, h: 22 },
    engines: { x: mx + 342, y: c.engines, w: 284, h: 94 },
    recent: { x: mx + 330, y: h - (h >= HINT_MIN_H ? 52 : 26), w: 306, h: 22 }
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

  // 見出し帯：骨白の段影＋琥珀の帯＋斜体の機能名。切替時は右から滑込み
  const item = v.items[v.selected];
  const s = v.slide ?? 1;
  const off = Math.round((1 - s) * (1 - s) * 56);
  p.slab(dx + 300 + off, c.top + 8, 360, 62, K, 9);
  p.slab(dx + 312 + off * 2, c.top, 340, 62, K, 5);
  p.text(item?.label ?? '', dx + 342 + off * 2, c.top + 6, 0, { k: K, font: `700 44px ${TITLE_FONT_FAMILY}` });
  const n = String(v.selected + 1).padStart(2, '0'),
    total = String(v.items.length).padStart(2, '0');
  p.text(n, w - 12, c.top + 2, 0, { align: 'right' });
  p.hl(w - 40, c.top + 20, 28, 0);
  p.text(total, w - 12, c.top + 22, 2, { align: 'right' });

  // 名札：ショートカット・動作状態
  let x = mx + 324;
  if (v.hotkey) x += chip(p, x, c.chips, v.hotkey, 0, 9) + 4;
  if (v.running) chip(p, x, c.chips, '● 動作中', 8, 1);

  // 説明窓
  p.slantWin(mx + 330, c.desc, 296, 70, 0.2, { shadow: 2 });
  let ty = p.wrap(v.desc, mx + 352, c.desc + 10, 260, 6, 18, v.error || v.preview ? 1 : 2);
  if (v.error) ty = p.wrap(v.error, mx + 352, ty, 260, 7, 18, 1);
  else if (v.preview) p.wrap(v.preview, mx + 352, ty, 260, 3, 18, 1);
  const go = `▶ Enter で${v.running ? '停止' : '開始'}`;
  p.text(go, mx + 612, c.desc + 47, v.hot === 'go' ? 9 : 5, { align: 'right' });

  // エンジン窓
  const E = R.engines;
  p.slantWin(E.x, E.y + 8, 278, 86, 0.2, { shadow: 2 });
  chip(p, E.x + 14, E.y, 'エンジン', 0, v.hot === 'engines' ? 9 : 5);
  v.engines.slice(0, 4).forEach((r, i) => {
    const [label, col] = STATE_TEXT[r.state];
    const y = E.y + 20 + i * 17;
    const lw = p.text(r.label, E.x + 26, y, 4) + 8;
    p.text(p.fit(r.model, 184 - lw), E.x + 26 + lw, y, 5);
    p.text(label, E.x + 262, y, col, { align: 'right' });
  });

  // 最近の翻訳（下端基準）
  const Rr = R.recent;
  p.slab(Rr.x, Rr.y, 306, 21, K, 1);
  p.text('最近', Rr.x + 16, Rr.y + 2, 3);
  if (v.recent) {
    p.text(p.fit(v.recent.text, 142), Rr.x + 54, Rr.y + 2, 6);
    p.text(v.recent.time, Rr.x + 202, Rr.y + 2, 3);
  } else p.text('履歴なし', Rr.x + 54, Rr.y + 2, 3);
  p.text('履歴▶', Rr.x + 254, Rr.y + 2, v.hot === 'recent' ? 9 : 5);

  if (h >= HINT_MIN_H) p.text(p.fit(v.hint, 316), w - 4 - RIGHT_MARGIN, h - 20, 3, { align: 'right' });
}
