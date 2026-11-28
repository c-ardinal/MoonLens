/**
 * ファイル翻訳の再生画面：再生位置と字幕の同期の純粋関数（Svelte・DOM へ非依存）。
 */
import type { FileSubtitleCue, FileSubtitleStatus } from './file-subtitle';

/** 字幕の表示内容。 */
export type CueContent = 'translated' | 'original' | 'both';

/** 再生位置の字幕の番号（字幕は開始時刻の昇順）。該当無しは -1。 */
export function cueIndexAt(cues: readonly FileSubtitleCue[], ms: number): number {
  let lo = 0;
  let hi = cues.length - 1;
  let found = -1;
  // 開始時刻が再生位置以前の最終の字幕を二分探索
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (cues[mid].start_ms <= ms) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return found >= 0 && ms < cues[found].end_ms ? found : -1;
}

/** 字幕の表示行。訳文の無い字幕（翻訳失敗）は原文で代替。 */
export function cueLines(cue: FileSubtitleCue, content: CueContent): string[] {
  const translated = cue.translated || cue.original;
  if (content === 'original') return [cue.original];
  if (content === 'translated' || !cue.translated) return [translated];
  return [cue.translated, cue.original];
}

/** 字幕の確定位置の直前で待機を開始する余裕。 */
export const WAIT_LEAD_MS = 1500;
/** 待機の解除に必要な生成済みの先行量（再開直後の再待機を抑止）。 */
export const RESUME_AHEAD_MS = 10_000;

/** 字幕の確定位置（`ready_ms`）へ再生が接近：再生を一時停止し、字幕の生成を待機。 */
export function mustWait(status: FileSubtitleStatus, positionMs: number): boolean {
  return status.phase === 'running' && positionMs + WAIT_LEAD_MS > status.ready_ms;
}

/** 待機の解除：生成が十分に先行、または生成の終了（完了・中止・失敗）。 */
export function mayResume(status: FileSubtitleStatus, positionMs: number): boolean {
  return status.phase !== 'running' || status.ready_ms - positionMs >= RESUME_AHEAD_MS;
}

/** 再生位置の字幕が確定前（生成中のみ）。 */
export function isAhead(status: FileSubtitleStatus, positionMs: number): boolean {
  return status.phase === 'running' && positionMs > status.ready_ms;
}
