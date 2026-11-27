/**
 * 他アプリのウィンドウへの吸着：型・イベント名・表示用の整形（バックエンド：`attach.rs`）。
 */

import { t } from './i18n';

/** 吸着先のウィンドウ。 */
export interface AttachTarget {
  /** ウィンドウシステムの識別子。 */
  id: number;
  pid: number;
  title: string;
  app: string;
}

/** 吸着先の候補。 */
export interface AttachCandidate extends AttachTarget {
  rect: { x: number; y: number; w: number; h: number };
  /** 自ウィンドウの面積中、候補上へ重畳する割合（0〜1）。 */
  overlap: number;
}

/**
 * 吸着先に対する配置。`cover`：全体へ重畳、`free`：相対位置を保持、
 * `inside`：内側下端、他：外側の各辺。
 */
export type AttachDock = 'cover' | 'free' | 'inside' | 'top' | 'bottom' | 'left' | 'right';

/** ウィンドウの吸着状態（`target: null`：吸着なし）。 */
export interface Attachment {
  target: AttachTarget | null;
  dock: AttachDock | null;
}

export const DETACHED: Attachment = { target: null, dock: null };

/** バックエンド → 当該ウィンドウ：吸着状態の変化（{@link Attachment}）。 */
export const ATTACH_EVENT = 'moonlens://attach';

/** 実行ファイル名の拡張子を除去したアプリ名。 */
function appName(app: string): string {
  return app.trim().replace(/\.exe$/i, '');
}

/** 吸着先の短縮名（アプリ名、不明時はウィンドウ名）。 */
export function targetName(target: AttachTarget): string {
  return appName(target.app) || target.title.trim() || t('ウィンドウ {id}', { id: target.id });
}

/** 一覧の表示内容：主（アプリ名）と副（ウィンドウ名、主と同一なら省略）。 */
export function targetLines(t: AttachTarget): { name: string; detail: string } {
  const name = targetName(t);
  const title = t.title.trim();
  return { name, detail: title && title !== name ? title : '' };
}

/** 自ウィンドウの過半が候補上：「直下」と表示する対象（一覧の先頭のみ）。 */
export function isUnder(candidates: AttachCandidate[], index: number): boolean {
  return index === 0 && (candidates[0]?.overlap ?? 0) >= 0.5;
}
