/**
 * 配信用ウィンドウ：吸着先のウィンドウと訳文を合成した取込専用ウィンドウ
 * （バックエンド：`stream.rs`、描画：crate `moonlens-stream`）。
 * 翻訳ウィンドウは表示中の訳文を配置済みの状態で送信し、合成側は同一配置で描画。
 */

import type { FlowResult } from './flow-layout';
import { maskBands } from './lens-overlay';
import type { LiveBox } from './live';

/** 当該ウィンドウの配信用ウィンドウの状態。 */
export interface StreamState {
  /** 当該環境での対応可否（Windows のみ対応）。 */
  supported: boolean;
  /** 吸着先の配信用ウィンドウが稼働中。 */
  active: boolean;
  /** 配信用ウィンドウのタイトル（取込ツールでの選択対象）。 */
  title: string | null;
}

export const NO_STREAM: StreamState = { supported: false, active: false, title: null };

/** バックエンド → 当該ウィンドウ：状態の変化（{@link StreamState}）。 */
export const STREAM_EVENT = 'moonlens://stream';

type Rgb = [number, number, number];

/** 訳文 1 行。`rect` はクライアント領域左上からの物理ピクセル `[x, y, w, h]`。 */
export interface StreamLine {
  text: string;
  rect: [number, number, number, number];
  /** 翻訳ウィンドウ上での文字列幅（物理ピクセル）。 */
  text_width: number;
  /** 枠外へ延伸する行：文字の背後を枠の色で塗装。 */
  backed: boolean;
}

/** 訳文 1 段落：原文を隠蔽する枠と、枠上の各行。 */
export interface StreamItem {
  rect: [number, number, number, number];
  /** 枠の塗装範囲（行単位）。空：`rect` 全体。隣接する画像を隠蔽しない枠で使用。 */
  fills: [number, number, number, number][];
  background: Rgb;
  foreground: Rgb;
  bold: boolean;
  /** 文字サイズ（物理ピクセル）。 */
  font_px: number;
  align: 'left' | 'center' | 'right';
  lines: StreamLine[];
}

/** 色の指定がない段落の配色（翻訳ウィンドウの既定と同一）。 */
const PAPER: Rgb = [255, 255, 255];
const INK: Rgb = [17, 17, 17];

/**
 * 表示中の枠を合成用の描画内容へ変換。
 * `layouts`：枠のキー別の行配置（未配置・翻訳待機中の枠は塗装のみ）。
 * `top`：枠の基準（本体領域）のクライアント領域上端からの距離（CSS px）。
 */
export function streamItems(
  boxes: LiveBox[],
  layouts: ReadonlyMap<string, FlowResult>,
  dpr: number,
  top: number
): StreamItem[] {
  const px = (v: number) => Math.round(v * dpr * 100) / 100;
  return boxes.map((b) => {
    const layout = b.pending ? undefined : layouts.get(b.rect.key);
    const x = b.rect.left;
    const y = b.rect.top + top;
    return {
      rect: [px(x), px(y), px(b.rect.width), px(b.rect.height)],
      fills: maskBands(b.block, dpr).map((m) => [px(x), px(y + m.top), px(m.right), px(m.bottom - m.top)]),
      background: b.block.colors?.background ?? PAPER,
      foreground: b.block.colors?.foreground ?? INK,
      bold: !!b.block.bold,
      font_px: px(layout?.fontSize ?? 0),
      align: b.block.align ?? 'left',
      lines: (layout?.lines ?? []).map((l) => ({
        text: l.text,
        rect: [px(x + l.left), px(y + l.top), px(l.width), px(l.height)],
        text_width: px(l.textWidth),
        backed: l.textWidth > l.width
      }))
    };
  });
}
