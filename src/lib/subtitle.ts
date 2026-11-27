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
  /** 遮断リストへの追加候補（利用者の判断待ち、検出順）。 */
  candidates: string[];
  error: string | null;
};

/** 保持件数の上限（超過分は旧字幕から破棄）。 */
export const MAX_LINES = 100;

export const initialView = (): SubtitleView => ({
  running: false,
  phase: 'waiting',
  device: '',
  lines: [],
  candidates: [],
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
    case 'candidate':
      return view.candidates.includes(ev.phrase)
        ? view
        : { ...view, candidates: [...view.candidates, ev.phrase] };
    case 'error':
      return { ...view, error: ev.message };
  }
}

/**
 * 候補への判断を反映。遮断（`blocked`）の場合、表示済みの同一文の字幕も除去
 * （誤認識と判断済みのため）。無視の場合、字幕は維持。
 */
export function settle(view: SubtitleView, phrase: string, blocked: boolean): SubtitleView {
  const candidates = view.candidates.filter((c) => c !== phrase);
  if (!blocked) return { ...view, candidates };
  const key = phraseKey(phrase);
  return { ...view, candidates, lines: view.lines.filter((l) => !saysOnly(l.original, key)) };
}

/** 表記差（大小文字・空白・約物）を除去した照合用の文字列。 */
export function phraseKey(text: string): string {
  return text.normalize('NFKC').toLowerCase().replace(/[^\p{L}\p{N}]/gu, '');
}

/** `text` の内容が `key` の文の反復のみか。 */
function saysOnly(text: string, key: string): boolean {
  const said = phraseKey(text);
  return key !== '' && said !== '' && said.length % key.length === 0 && said === key.repeat(said.length / key.length);
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
