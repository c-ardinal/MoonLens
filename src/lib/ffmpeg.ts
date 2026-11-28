/**
 * 字幕合成用 ffmpeg（利用者が配置）の判定結果の文言。
 */
import type { FfmpegStatus } from './ipc';

/** 利用不可の理由のキー ID。利用可能な場合は null。 */
export function ffmpegProblem(status: FfmpegStatus): string | null {
  if (!status.path) return 'file-burn:no_ffmpeg';
  if (status.check === 'no_libass') return 'file-burn:no_libass';
  if (status.check !== 'ok') return 'file-burn:not_ffmpeg';
  return null;
}
