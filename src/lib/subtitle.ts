import type { AudioApp, AudioSource, SubtitleEvent, SubtitlePhase, SubtitleSession } from '$lib/ipc';
import type { InputState } from '$lib/subtitle-level';
import { t } from './i18n';

/** 字幕 1 件（認識単位）。 */
export type SubtitleLine = { id: number; original: string; translated: string };

/** 音声翻訳ウィンドウの表示状態。 */
export type SubtitleView = {
  running: boolean;
  phase: SubtitlePhase;
  /** 音声取得中のデバイス名、またはアプリ名。 */
  device: string;
  /** 音源：PC 全体、またはアプリ単位。 */
  source: AudioSource;
  lines: SubtitleLine[];
  error: string | null;
};

/** 保持件数の上限（超過分は旧字幕から破棄）。 */
export const MAX_LINES = 100;

export const initialView = (): SubtitleView => ({
  running: false,
  phase: 'waiting',
  device: '',
  source: { kind: 'system' },
  lines: [],
  error: null
});

/** バックエンドのイベント（`moonlens://subtitle`）を表示状態へ反映。 */
export function reduce(view: SubtitleView, ev: SubtitleEvent): SubtitleView {
  switch (ev.kind) {
    case 'started':
      // 音源変更・再開：表示済みの字幕は保持。
      return { ...view, running: true, phase: 'waiting', device: ev.device, source: ev.source, error: null };
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
  if (!view.running) return view.error ? t('エラー停止') : t('停止中');
  if (view.phase === 'recognizing') return t('認識中…');
  if (view.phase === 'translating') return t('翻訳中…');
  if (input === 'ok') return t('音声検出');
  return input === 'low' ? t('閾値未満') : t('入力未検出');
}

/** 字幕表示前の案内文：入力状態と対処。`app`：アプリ単位の音源名（PC 全体は null）。 */
export function inputHint(input: InputState, app: string | null = null): string {
  if (input === 'ok') return t('音声検出。認識結果を字幕表示。');
  if (input === 'low') return t('入力音量が認識閾値未満。再生音量の調整が必要。');
  return app
    ? t('音声入力未検出。アプリ「{app}」の再生音声が認識対象。', { app })
    : t('音声入力未検出。PC の再生音声が認識対象。');
}

/** 再読込時：照会した現況を表示状態へ反映。 */
export function restore(view: SubtitleView, session: SubtitleSession | null): SubtitleView {
  if (!session) return view;
  return { ...view, running: session.running, device: session.device, source: session.source };
}

/** 音源の表記（タイトルバー）。 */
export function sourceLabel(view: Pick<SubtitleView, 'source' | 'device'>): string {
  if (view.source.kind === 'system') return t('PC 全体');
  return view.device || `PID ${view.source.pid}`;
}

export const sameSource = (a: AudioSource, b: AudioSource): boolean =>
  a.kind === 'system' ? b.kind === 'system' : b.kind === 'app' && a.pid === b.pid;

/** 音源メニューの 1 行。 */
export type SourceOption = {
  source: AudioSource;
  label: string;
  /** 補足（アプリのウィンドウ題名）。 */
  detail: string;
  playing: boolean;
  selected: boolean;
};

/**
 * 音源メニューの選択肢：先頭は PC 全体、以降はアプリ一覧。選択中のアプリが
 * 一覧に不在（音声出力を終了）でも、選択肢として残置。
 */
export function sourceOptions(
  apps: AudioApp[],
  view: Pick<SubtitleView, 'source' | 'device'>
): SourceOption[] {
  const current = view.source;
  const options: SourceOption[] = [
    {
      source: { kind: 'system' },
      label: t('PC 全体'),
      detail: t('全アプリの再生音声'),
      playing: apps.some((a) => a.playing),
      selected: current.kind === 'system'
    },
    ...apps.map((a) => ({
      source: { kind: 'app', pid: a.pid } as AudioSource,
      label: a.name,
      detail: a.title ?? '',
      playing: a.playing,
      selected: sameSource(current, { kind: 'app', pid: a.pid })
    }))
  ];
  if (current.kind === 'app' && !options.some((o) => o.selected)) {
    options.push({ source: current, label: sourceLabel(view), detail: t('音声出力なし'), playing: false, selected: true });
  }
  return options;
}
