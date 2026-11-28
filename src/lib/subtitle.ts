import type {
  AudioApp,
  AudioSource,
  Speaker,
  SubtitleEvent,
  SubtitlePhase,
  SubtitleSession
} from '$lib/ipc';
import type { InputState } from '$lib/subtitle-level';
import { t } from './i18n';

/** 字幕 1 件（認識単位、補正後は文単位）。 */
export type SubtitleLine = {
  id: number;
  original: string;
  translated: string;
  /** 文単位の再翻訳で確定済み。 */
  revised?: boolean;
  /** 対面会話の話者。字幕では未設定。 */
  speaker?: Speaker;
};

/** 音声翻訳ウィンドウの表示状態。 */
export type SubtitleView = {
  running: boolean;
  phase: SubtitlePhase;
  /** 音声取得中のデバイス名、またはアプリ名。 */
  device: string;
  /** 音源：PC 全体、またはアプリ単位。 */
  source: AudioSource;
  lines: SubtitleLine[];
  /** 遮断リストへの追加候補（利用者の判断待ち、検出順）。 */
  candidates: string[];
  error: string | null;
  /**
   * 行 ID の加算値。バックエンドは開始（音源変更・再開）単位で 1 から採番する一方、
   * 表示済みの字幕は保持するため、開始時点の最大 ID を加算して重複を回避。
   */
  idBase: number;
  /** 採番済みの最大 ID（加算後）。 */
  lastId: number;
};

/** 保持件数の上限（超過分は旧字幕から破棄）。 */
export const MAX_LINES = 100;

export const initialView = (): SubtitleView => ({
  running: false,
  phase: 'waiting',
  device: '',
  source: { kind: 'system' },
  lines: [],
  candidates: [],
  error: null,
  idBase: 0,
  lastId: 0
});

/** バックエンドのイベント（`moonlens://subtitle`）を表示状態へ反映。 */
export function reduce(view: SubtitleView, ev: SubtitleEvent): SubtitleView {
  switch (ev.kind) {
    case 'started':
      // 音源変更・再開：表示済みの字幕は保持。
      return {
        ...view,
        running: true,
        phase: 'waiting',
        device: ev.device,
        source: ev.source,
        // 反復検出はバックエンド側で開始単位に初期化。
        candidates: [],
        error: null,
        idBase: view.lastId
      };
    case 'stopped':
      // 停止理由（エラー）は表示を継続。
      return { ...view, running: false, phase: 'waiting' };
    case 'status':
      return { ...view, phase: ev.phase };
    case 'chunk': {
      const id = view.idBase + ev.id;
      const line: SubtitleLine = { id, original: ev.original, translated: ev.translated };
      if (ev.speaker) line.speaker = ev.speaker;
      return {
        ...view,
        error: null,
        lines: [...view.lines, line].slice(-MAX_LINES),
        lastId: Math.max(view.lastId, id)
      };
    }
    case 'revised': {
      const base = view.idBase;
      const replaces = ev.replaces.map((id) => base + id);
      const lastId = Math.max(view.lastId, base + ev.id, base + (ev.rest?.id ?? 0));
      // 置換対象が保持範囲外（破棄済み）の場合、行は不変。
      const at = view.lines.findIndex((l) => replaces.includes(l.id));
      if (at < 0) return lastId === view.lastId ? view : { ...view, lastId };
      // 対面会話：文は 1 話者の発話（話者交替で文を終了）のため、置換対象の話者を継承。
      const speaker = view.lines[at].speaker;
      const spoken = speaker ? { speaker } : {};
      // 原文が空：補正前に遮断済みとなった文。置換対象の行は除去、次文の冒頭のみ残置。
      const next: SubtitleLine[] = ev.original
        ? [{ id: base + ev.id, original: ev.original, translated: ev.translated, revised: true, ...spoken }]
        : [];
      if (ev.rest) next.push({ ...ev.rest, id: base + ev.rest.id, ...spoken });
      const lines = view.lines.filter((l) => !replaces.includes(l.id));
      lines.splice(at, 0, ...next);
      return { ...view, lines, lastId };
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
  if (!view.running) return view.error ? t('subtitle:stopped_error') : t('common:stopped');
  if (view.phase === 'recognizing') return t('subtitle:listening');
  if (view.phase === 'translating') return t('common:translating');
  if (view.phase === 'revising') return t('subtitle:correcting');
  if (input === 'ok') return t('subtitle:audio_detected');
  return input === 'low' ? t('subtitle:below_threshold') : t('subtitle:no_input');
}

/**
 * 字幕表示前の案内文：入力状態と対処。`app`：アプリ単位の音源名（PC 全体は null）。
 * `microphone`：対面会話（マイク入力）。
 */
export function inputHint(input: InputState, app: string | null = null, microphone = false): string {
  if (microphone) {
    return input === 'none'
      ? t('subtitle:no_microphone_input_detected')
      : t('subtitle:speak_in_either_language');
  }
  if (input === 'ok') return t('subtitle:audio_detected_subtitles_will_appear_here');
  if (input === 'low') return t('subtitle:input_is_below_the_recognition_threshold');
  return app
    ? t('subtitle:no_audio_input_detected_listening_to_audio', { app })
    : t('subtitle:no_audio_detected_moonlens_listens_to_what');
}

/** 再読込時：照会した現況を表示状態へ反映。 */
export function restore(view: SubtitleView, session: SubtitleSession | null): SubtitleView {
  if (!session) return view;
  return { ...view, running: session.running, device: session.device, source: session.source };
}

/** 音源の表記（タイトルバー）。 */
export function sourceLabel(view: Pick<SubtitleView, 'source' | 'device'>): string {
  if (view.source.kind === 'system') return t('subtitle:whole_pc');
  if (view.source.kind === 'microphone') return t('subtitle:microphone');
  return view.device || `PID ${view.source.pid}`;
}

export const sameSource = (a: AudioSource, b: AudioSource): boolean =>
  a.kind === 'app' ? b.kind === 'app' && a.pid === b.pid : a.kind === b.kind;

/**
 * 音源変更に伴う画面切替の案内文。対面会話（マイク）と字幕表示の間の変更のみ対象、
 * 同一画面内の変更（PC 全体とアプリの間）は null。
 */
export function modeSwitchNotice(from: AudioSource, to: AudioSource): string | null {
  const before = from.kind === 'microphone';
  const after = to.kind === 'microphone';
  if (before === after) return null;
  return after ? t('subtitle:switched_to_conversation_view') : t('subtitle:switched_to_subtitle_view');
}

/** 音源メニューの行の識別子。 */
export const sourceKey = (source: AudioSource): string | number =>
  source.kind === 'app' ? source.pid : source.kind;

/** 対面会話の言語対：本人（音声翻訳の翻訳先言語）と相手。 */
export type ConversationPair = { own: string; partner: string };

const spoken = (code: string) => code.split(/[-_]/)[0].trim().toLowerCase();

/**
 * 設定値から言語対を決定（バックエンド `conversation::Pair::from_settings` と同一規則）。
 * 相手の言語が未設定、または本人と同一の場合：英語（本人が英語の場合は日本語）。
 */
export function conversationPair(own: string, partner: string): ConversationPair {
  const same = !partner.trim() || spoken(partner) === spoken(own);
  return { own, partner: same ? (spoken(own) === 'en' ? 'ja' : 'en') : partner.trim() };
}

/** 相手の言語の選択肢：本人と同一の言語は対象外。 */
export function partnerLanguages<T extends { code: string }>(languages: T[], own: string): T[] {
  return languages.filter((l) => spoken(l.code) !== spoken(own));
}

/** 音源メニューの 1 行。 */
export type SourceOption = {
  source: AudioSource;
  label: string;
  /** 補足（アプリのウィンドウ題名）。 */
  detail: string;
  /** 音声再生中。null：再生状況の対象外（マイク）。 */
  playing: boolean | null;
  selected: boolean;
};

/**
 * 音源メニューの選択肢：先頭は PC 全体とマイク（対面会話）、以降はアプリ一覧。選択中のアプリが
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
      label: t('subtitle:whole_pc'),
      detail: t('subtitle:audio_from_all_apps'),
      playing: apps.some((a) => a.playing),
      selected: current.kind === 'system'
    },
    {
      source: { kind: 'microphone' },
      label: t('subtitle:conversation'),
      detail: t('subtitle:microphone_switches_to_conversation_view'),
      playing: null,
      selected: current.kind === 'microphone'
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
    options.push({ source: current, label: sourceLabel(view), detail: t('subtitle:no_audio_output'), playing: false, selected: true });
  }
  return options;
}
