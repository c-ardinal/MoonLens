import type { SubtitleEvent, SubtitlePhase } from '$lib/ipc';

/** 字幕 1 件（認識単位、補正後は文単位）。 */
export type SubtitleLine = {
  id: number;
  original: string;
  translated: string;
  /** 文単位の再翻訳で確定済み。 */
  revised?: boolean;
};

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
      const line = { id: ev.id, original: ev.original, translated: ev.translated };
      return { ...view, error: null, lines: [...view.lines, line].slice(-MAX_LINES) };
    }
    case 'revised': {
      // 置換対象が保持範囲外（破棄済み）の場合は無視。
      const at = view.lines.findIndex((l) => ev.replaces.includes(l.id));
      if (at < 0) return view;
      const next: SubtitleLine[] = [
        { id: ev.id, original: ev.original, translated: ev.translated, revised: true }
      ];
      if (ev.rest) next.push({ ...ev.rest });
      const lines = view.lines.filter((l) => !ev.replaces.includes(l.id));
      lines.splice(at, 0, ...next);
      return { ...view, lines };
    }
    case 'error':
      return { ...view, error: ev.message };
  }
}

/** タイトルバーの状態表記。 */
export function statusLabel(view: SubtitleView): string {
  if (!view.running) return view.error ? 'エラー停止' : '停止中';
  if (view.phase === 'recognizing') return '認識中…';
  if (view.phase === 'translating') return '翻訳中…';
  if (view.phase === 'revising') return '補正中…';
  return '音声待機中';
}
