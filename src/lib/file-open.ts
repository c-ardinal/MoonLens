/**
 * ファイル翻訳：入力ファイルの種別判定と画面の選択（Svelte・IPC へ非依存）。
 */
import { isImageName } from './image-translate';
import { isJob, type FileSubtitleStatus } from './file-subtitle';

/** 処理の種別。`image`：画像翻訳、`media`：動画・音声の字幕生成。 */
export type FileKind = 'image' | 'media';

/** 表示中の画面。`idle`：ファイル未選択。 */
export type FileView = 'idle' | FileKind;

/**
 * ファイル名から処理を決定。画像の拡張子は画像翻訳、以外は字幕生成
 * （動画・音声の可否は内容で判定：本体側）。
 */
export function fileKind(name: string): FileKind {
  return isImageName(name) ? 'image' : 'media';
}

/** 画面を開いた時点の表示：字幕生成のジョブが存在する場合は進行状況。 */
export function initialView(status: FileSubtitleStatus): FileView {
  return isJob(status) ? 'media' : 'idle';
}
