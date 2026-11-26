/**
 * ウィンドウ翻訳のテキスト表示：表示位置（窓内・窓外）と、窓外表示用ウィンドウとの通信。
 */

import type { LensBlock } from './ipc';

/** テキスト表示の位置。`inside` 以外は窓外（別ウィンドウ、本体の移動へ追従）。 */
export type TextPlacement = 'inside' | 'top' | 'bottom' | 'left' | 'right' | 'free';

export const TEXT_PLACEMENTS: { value: TextPlacement; label: string }[] = [
  { value: 'inside', label: '窓内' },
  { value: 'top', label: '窓外：上' },
  { value: 'bottom', label: '窓外：下' },
  { value: 'left', label: '窓外：左' },
  { value: 'right', label: '窓外：右' },
  { value: 'free', label: '窓外：自由配置' }
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

/** 窓外表示から本体への操作。 */
export type TextCommand = { kind: 'placement'; value: TextPlacement } | { kind: 'close' };

/** 本体 → 窓外表示：表示内容（{@link TextView}）。 */
export const TEXT_VIEW_EVENT = 'moonlens://lens-text';
/** 窓外表示 → 本体：受信準備完了（本体は現在の内容を再送）。 */
export const TEXT_READY_EVENT = 'moonlens://lens-text-ready';
/** 窓外表示 → 本体：{@link TextCommand}。 */
export const TEXT_COMMAND_EVENT = 'moonlens://lens-text-command';

/** 窓外表示用ウィンドウのラベル（バックエンドの `lens_text::label` と同一規則）。 */
export const textWindowLabel = (id: string) => `${id}-text`;
