/**
 * ファイル字幕：進行状況の表示用の純粋関数（Svelte・IPC へ非依存）。
 */
import { timeLabel } from "./subtitle-record";

export type FileSubtitlePhase =
  | "idle"
  | "running"
  | "done"
  | "cancelled"
  | "failed";

/** ジョブの状態（`file_subtitle::Status`）。 */
export interface FileSubtitleStatus {
  phase: FileSubtitlePhase;
  /** フォルダーを除外したファイル名。 */
  name: string;
  /** 音声トラックの再生時間。コンテナーに記載の無い場合は null。 */
  duration_ms: number | null;
  /** 処理済みの位置。 */
  done_ms: number;
  /** 生成済みの字幕数。 */
  cues: number;
  error: string | null;
}

/** 開始済みのジョブ（生成中・完了・中止・失敗）。 */
export type FileSubtitleJob = FileSubtitleStatus & {
  phase: Exclude<FileSubtitlePhase, "idle">;
};

export const isJob = (status: FileSubtitleStatus): status is FileSubtitleJob =>
  status.phase !== "idle";

export interface FileSubtitleCue {
  start_ms: number;
  end_ms: number;
  original: string;
  /** 翻訳失敗時は空文字。 */
  translated: string;
}

/** 状態の変化（`moonlens://file-subtitle`）。`cue` は当該変化で追加の字幕。 */
export interface FileSubtitleUpdate {
  status: FileSubtitleStatus;
  cue: FileSubtitleCue | null;
}

export const IDLE: FileSubtitleStatus = {
  phase: "idle",
  name: "",
  duration_ms: null,
  done_ms: 0,
  cues: 0,
  error: null,
};

/** 画面に保持する直近の字幕数（全件は本体側で保持、保存は全件が対象）。 */
export const RECENT_CUES = 100;

/** 進行率（0〜1）。再生時間不明の場合は null、完了時は 1。 */
export function progressRatio(status: FileSubtitleStatus): number | null {
  if (status.phase === "done") return 1;
  if (!status.duration_ms) return null;
  return Math.min(1, Math.max(0, status.done_ms / status.duration_ms));
}

/** 処理済みの位置と全体の再生時間（例：`12:34 / 1:02:03`）。再生時間不明の場合は位置のみ。 */
export function positionLabel(status: FileSubtitleStatus): string {
  const done = timeLabel(status.done_ms);
  return status.duration_ms
    ? `${done} / ${timeLabel(status.duration_ms)}`
    : done;
}

/** 直近の字幕へ 1 件追加（上限超過分は先頭から削除）。 */
export function withCue(
  recent: readonly FileSubtitleCue[],
  cue: FileSubtitleCue,
): FileSubtitleCue[] {
  return [...recent, cue].slice(-RECENT_CUES);
}

/** 状態の文言のキー ID。 */
export const PHASE_LABELS: Record<
  Exclude<FileSubtitlePhase, "idle">,
  string
> = {
  running: "file-subtitle:phase_running",
  done: "file-subtitle:phase_done",
  cancelled: "file-subtitle:phase_cancelled",
  failed: "file-subtitle:phase_failed",
};

/** 字幕合成の進行（`file_subtitle::BurnUpdate`）。 */
export interface FileBurnUpdate {
  phase: "running" | "done" | "cancelled" | "failed";
  /** 処理済みの映像の位置。 */
  done_ms: number;
  /** 出力ファイル名。 */
  file: string;
  error: string | null;
}
