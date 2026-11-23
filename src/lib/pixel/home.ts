/**
 * ホーム画面の描画（UI 第5案のモック準拠）。
 * 左：ルーン魔法陣、右：斜めの見出し帯・説明窓・エンジン窓・最近の翻訳。
 * 右側は画面右端基準、最近の翻訳と案内行は下端基準で配置。
 */
import { Pix, TITLE_FONT_FAMILY } from './pix';
import { chip, drawDial, type DialItem } from './dial';
import type { EngineState } from '../engine-status';

const K = 0.3;

export interface HomeView {
  items: DialItem[];
  selected: number;
  hover: number | 'center' | null;
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
}

export type HomeRegion = 'go' | 'engines' | 'recent';
export type Rect = { x: number; y: number; w: number; h: number };

/** 右側の情報欄の各領域（論理座標）。 */
export function homeRegions(w: number, h: number): Record<HomeRegion, Rect> {
  const dx = w - 640;
  return {
    go: { x: dx + 480, y: 176, w: 146, h: 22 },
    engines: { x: dx + 342, y: 212, w: 290, h: 86 },
    recent: { x: dx + 330, y: h - 52, w: 310, h: 22 }
  };
}

const STATE_TEXT: Record<EngineState, [string, number]> = {
  ready: ['準備完了', 8],
  missing: ['未導入', 7],
  downloading: ['取得中', 4],
  unset: ['未設定', 3]
};

export function drawHome(p: Pix, v: HomeView) {
  const { w, h } = p;
  const dx = w - 640;
  const R = homeRegions(w, h);
  p.clear();

  drawDial(p, { items: v.items, selected: v.selected, hover: v.hover, anchor: { x: dx + 330, y: 80 } });

  // 見出し帯：骨白の段影＋琥珀の帯＋斜体の機能名
  const item = v.items[v.selected];
  p.slab(dx + 300, 40, 360, 62, K, 9);
  p.slab(dx + 312, 32, 340, 62, K, 5);
  p.text(item?.label ?? '', dx + 342, 38, 0, { k: K, font: `700 44px ${TITLE_FONT_FAMILY}` });
  const n = String(v.selected + 1).padStart(2, '0'),
    total = String(v.items.length).padStart(2, '0');
  p.text(n, w - 12, 34, 0, { align: 'right' });
  p.hl(w - 40, 52, 28, 0);
  p.text(total, w - 12, 54, 2, { align: 'right' });

  // 名札：ショートカット・直接起動・動作状態
  let x = dx + 324;
  if (v.hotkey) x += chip(p, x, 106, v.hotkey, 0, 9) + 4;
  x += chip(p, x, 106, `直接 ${v.selected + 1}`, 5, 1) + 4;
  if (v.running) chip(p, x, 106, '● 動作中', 8, 1);

  // 説明窓
  p.slantWin(dx + 330, 134, 296, 70, 0.2, { shadow: 2 });
  let ty = p.wrap(v.desc, dx + 352, 144, 260, 6, 18, v.error || v.preview ? 1 : 2);
  if (v.error) ty = p.wrap(v.error, dx + 352, ty, 260, 7, 18, 1);
  else if (v.preview) p.wrap(v.preview, dx + 352, ty, 260, 3, 18, 1);
  const go = `▶ Enter で${v.running ? '停止' : '開始'}`;
  p.text(go, dx + 612, 181, v.hot === 'go' ? 9 : 5, { align: 'right' });

  // エンジン窓
  const E = R.engines;
  p.slantWin(E.x, E.y + 8, 284, 86, 0.2, { shadow: 2 });
  chip(p, E.x + 14, E.y, 'エンジン', 0, v.hot === 'engines' ? 9 : 5);
  v.engines.slice(0, 4).forEach((r, i) => {
    const [label, c] = STATE_TEXT[r.state];
    const y = E.y + 20 + i * 17;
    const lw = p.text(r.label, E.x + 26, y, 4) + 8;
    p.text(p.fit(r.model, 190 - lw), E.x + 26 + lw, y, 5);
    p.text(label, E.x + 268, y, c, { align: 'right' });
  });

  // 最近の翻訳（下端基準）
  const Rr = R.recent;
  p.slab(Rr.x, Rr.y, 320, 21, K, 1);
  p.text('最近', Rr.x + 16, Rr.y + 2, 3);
  if (v.recent) {
    p.text(p.fit(v.recent.text, 150), Rr.x + 54, Rr.y + 2, 6);
    p.text(v.recent.time, Rr.x + 210, Rr.y + 2, 3);
  } else p.text('履歴なし', Rr.x + 54, Rr.y + 2, 3);
  p.text('履歴▶', Rr.x + 266, Rr.y + 2, v.hot === 'recent' ? 9 : 5);

  p.text(v.hint, w - 14, h - 20, 3, { align: 'right' });
}
