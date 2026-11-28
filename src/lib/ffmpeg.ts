/**
 * 字幕合成用 ffmpeg（利用者が配置）の判定結果の文言。
 */
import type { FfmpegStatus } from './ipc';

/** 入手先の案内ページ（利用者の操作時のみ既定のブラウザーで表示）。 */
export const FFMPEG_DOWNLOAD_URL = 'https://ffmpeg.org/download.html';

/** 利用不可の理由のキー ID。利用可能な場合は null。 */
export function ffmpegProblem(status: FfmpegStatus): string | null {
  if (!status.path) return 'file-burn:no_ffmpeg';
  if (status.check === 'no_libass') return 'file-burn:no_libass';
  if (status.check !== 'ok') return 'file-burn:not_ffmpeg';
  return null;
}
