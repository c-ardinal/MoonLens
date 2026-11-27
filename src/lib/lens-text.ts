/**
 * ウィンドウ翻訳のテキスト表示：表示位置（ウィンドウ内・ウィンドウ外）と、ウィンドウ外表示用ウィンドウとの通信。
 */

import type { LensBlock } from './ipc';

/** テキスト表示の位置。`inside` 以外はウィンドウ外（別ウィンドウ、本体の移動へ追従）。 */
export type TextPlacement = 'inside' | 'top' | 'bottom' | 'left' | 'right' | 'free';

/** label は日本語の原文（表示時は `t()` で翻訳）。 */
export const TEXT_PLACEMENTS: { value: TextPlacement; label: string }[] = [
  { value: 'inside', label: 'ウィンドウ内' },
  { value: 'top', label: 'ウィンドウ外上側' },
  { value: 'bottom', label: 'ウィンドウ外下側' },
  { value: 'left', label: 'ウィンドウ外左側' },
  { value: 'right', label: 'ウィンドウ外右側' },
  { value: 'free', label: 'ウィンドウ外自由配置' }
];

export function parsePlacement(v: unknown): TextPlacement {
  return TEXT_PLACEMENTS.some((p) => p.value === v) ? (v as TextPlacement) : 'inside';
}

const STORAGE_KEY = 'moonlens.lens.text-placement';

/** 前回選択の表示位置（全ウィンドウ共通、新規ウィンドウの初期値）。 */
export function loadPlacement(): TextPlacement {
  try {
    return parsePlacement(localStorage.getItem(STORAGE_KEY));
  } catch {
    return 'inside';
  }
}

export function savePlacement(p: TextPlacement) {
  try {
    localStorage.setItem(STORAGE_KEY, p);
  } catch {
    /* 保存不可の環境：当該ウィンドウ限定で適用 */
  }
}

/** テキスト表示の内容。 */
export interface TextView {
  blocks: LensBlock[];
  translation: string;
  source: string | null;
  explanation: string | null;
  placement: TextPlacement;
}

/** ウィンドウ外表示から本体への操作。 */
export type TextCommand = { kind: 'placement'; value: TextPlacement } | { kind: 'close' };

/** 本体 → ウィンドウ外表示：表示内容（{@link TextView}）。 */
export const TEXT_VIEW_EVENT = 'moonlens://lens-text';
/** ウィンドウ外表示 → 本体：受信準備完了（本体は現在の内容を再送）。 */
export const TEXT_READY_EVENT = 'moonlens://lens-text-ready';
/** ウィンドウ外表示 → 本体：{@link TextCommand}。 */
export const TEXT_COMMAND_EVENT = 'moonlens://lens-text-command';

/** ウィンドウ外表示用ウィンドウのラベル（バックエンドの `lens_text::label` と同一規則）。 */
export const textWindowLabel = (id: string) => `${id}-text`;
