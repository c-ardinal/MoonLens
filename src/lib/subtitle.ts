import type { SubtitleEvent, SubtitlePhase } from '$lib/ipc';
import type { InputState } from '$lib/subtitle-level';

/** 字幕 1 件（認識単位）。 */
export type SubtitleLine = { id: number; original: string; translated: string };

/** 音声翻訳ウィンドウの表示状態。 */
export type SubtitleView = {
  running: boolean;
  phase: SubtitlePhase;
  /** 音声取得中のデバイス名。 */
  device: string;
  lines: SubtitleLine[];
  error: string | null;
};

/** 保持件数の上限（超過分は旧字幕から破棄）。 */
export const MAX_LINES = 100;

export const initialView = (): SubtitleView => ({
  running: false,
  phase: 'waiting',
  device: '',
  lines: [],
  error: null
});

/** バックエンドのイベント（`moonlens://subtitle`）を表示状態へ反映。 */
export function reduce(view: SubtitleView, ev: SubtitleEvent): SubtitleView {
  switch (ev.kind) {
    case 'started':
      return { ...initialView(), running: true, device: ev.device };
    case 'stopped':
      // 停止理由（エラー）は表示を継続。
      return { ...view, running: false, phase: 'waiting' };
    case 'status':
      return { ...view, phase: ev.phase };
    case 'chunk': {
      const id = (view.lines.at(-1)?.id ?? 0) + 1;
      const line = { id, original: ev.original, translated: ev.translated };
      return { ...view, error: null, lines: [...view.lines, line].slice(-MAX_LINES) };
    }
    case 'error':
      return { ...view, error: ev.message };
  }
}

/** タイトルバーの状態表記。音声待機中は入力状態を表示。 */
export function statusLabel(view: SubtitleView, input: InputState = 'none'): string {
  if (!view.running) return view.error ? 'エラー停止' : '停止中';
  if (view.phase === 'recognizing') return '認識中…';
  if (view.phase === 'translating') return '翻訳中…';
  if (input === 'ok') return '音声検出';
  return input === 'low' ? '閾値未満' : '入力未検出';
}

/** 字幕表示前の案内文：入力状態と対処。 */
export function inputHint(input: InputState): string {
  if (input === 'ok') return '音声検出。認識結果を字幕表示。';
  if (input === 'low') return '入力音量が認識閾値未満。再生音量の調整が必要。';
  return '音声入力未検出。PC の再生音声が認識対象。';
}
