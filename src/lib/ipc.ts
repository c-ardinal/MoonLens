/**
 * Typed wrappers around Tauri `invoke` calls.
 * Every IPC surface the frontend needs lives here so it is easy to mock
 * in tests and audit what the backend can be asked to do.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { AttachCandidate, AttachDock, Attachment } from './attach';
import { LAYOUT_SETS_EVENT, type LayoutSet, type LensView } from './layout-sets';
import { NO_STREAM, type StreamItem, type StreamState } from './stream';

export type Settings = Record<string, any>;

export type Intent = 'qr' | 'text' | 'photo' | 'mixed' | 'unknown';

export interface OcrLine {
  text: string;
  bbox: [number, number, number, number];
  confidence: number;
}

export type TaskOutput =
  | { Text: string }
  | { TextChunk: { delta: string; done: boolean } }
  | { Ocr: { lines: OcrLine[] } }
  | { Transcript: { segments: { start_s: number; end_s: number; text: string }[] } }
  | { OpenedExternalUrl: string }
  | { Intent: Intent }
  | 'Empty';

export interface TaskResult {
  task: string;
  output: TaskOutput;
  used_provider: string;
  via_cloud: boolean;
  latency_ms: number;
  fallback_reasons: string[];
}

export interface CaptureFlowRect {
  monitor_index: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** One recognised paragraph, positioned in the captured image. */
export interface LensBlock {
  /** [x, y, w, h] in captured-image pixels. */
  bbox: [number, number, number, number];
  /** Median source line height in pixels. */
  line_height: number;
  source: string;
  /** null while translating, or when translation failed for this block. */
  translation: string | null;
  colors: { background: [number, number, number]; foreground: [number, number, number] } | null;
  /** Already in the target language (or no letters): not translated. */
  same_language?: boolean;
  /** Translation came from the local translation cache (shown at once). */
  from_cache?: boolean;
  /** Horizontal alignment of the source lines. */
  align?: 'left' | 'center' | 'right';
  /** Source text looks bold; draw the translation bold too. */
  bold?: boolean;
  /** Source text rows `[x, y, w, h]` (captured-image px), top to bottom. */
  lines?: [number, number, number, number][];
  /**
   * Right edge (captured-image px) the text of each row may reach, set
   * when a picture stands beside some rows of the block.
   */
  row_limits?: number[];
  /**
   * Set when the text stands on a picture or gradient: the text pixels
   * repainted from their surroundings, drawn instead of the box.
   */
  erase?: ErasePatch | null;
}

/** Text-only erase picture of a block. */
export interface ErasePatch {
  /** [x, y, w, h] of the picture in the pixels the block is placed in (x / y negative beyond a window frame). */
  rect: [number, number, number, number];
  /** PNG data URL: text pixels opaque, the rest transparent. */
  image: string;
  /** The background varies: the translation needs an outline. */
  outline: boolean;
}

/** Sent before each translation request of a capture (`moonlens://image-progress`). */
export interface TranslationProgress {
  /** Blocks with the translations finished so far. */
  blocks: LensBlock[];
  /** Indices of the blocks the next request streams into, in order. */
  streaming: number[];
}

export interface CaptureFlowResult {
  intent: Intent;
  source_text: string | null;
  result: TaskResult;
  source_rect: CaptureFlowRect | null;
  /** Paragraph blocks for the in-place overlay (empty for QR / photos). */
  blocks?: LensBlock[];
  /** [width, height] of the captured image (coordinate space of blocks). */
  image_size?: [number, number] | null;
  /** Best guess at the source language ("ja", "en", …). */
  source_language?: string | null;
  /** Language the blocks were translated into. */
  target_language?: string | null;
}

export type OverlayEvent =
  | { kind: 'processing'; message: string }
  | { kind: 'chunk'; delta: string; done: boolean }
  | {
      kind: 'result';
      source_text: string | null;
      intent: Intent | null;
      result: TaskResult;
      source_rect: CaptureFlowRect | null;
    }
  | { kind: 'error'; message: string }
  | { kind: 'hidden' };

export interface PluginDescriptor {
  id: string;
  name: string;
  version: string;
  author: string;
  license: string;
  kind: 'wasm' | 'sidecar';
  enabled: boolean;
  provides: string[];
  permissions_summary: string[];
  path: string;
}

const hasTauri = () =>
  typeof window !== 'undefined' && typeof (window as any).__TAURI_INTERNALS__ !== 'undefined';

async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  if (!hasTauri()) {
    if (fallback !== undefined) return fallback;
    throw new Error(`Tauri not available: ${cmd}`);
  }
  return (await invoke(cmd, args)) as T;
}

export const getSettings = () => safeInvoke<Settings>('get_settings', undefined, {});
export const setSettings = (next: Settings) => safeInvoke<void>('set_settings', { new: next });
export const getSettingsSchema = () => safeInvoke<any>('get_settings_schema', undefined, null);
/** The bundled defaults, for per-field "初期値に復元". */
export const getDefaultSettings = () => safeInvoke<Settings>('get_default_settings', undefined, {});
export const listPlugins = () =>
  safeInvoke<PluginDescriptor[]>('list_plugins', undefined, []);
export const setPluginEnabled = (id: string, enabled: boolean) =>
  safeInvoke<void>('set_plugin_enabled', { id, enabled });
export const installPluginFromPath = (archivePath: string) =>
  safeInvoke<string>('install_plugin_from_path', { archivePath });
export const pickAndInstallPlugin = () =>
  safeInvoke<string | null>('pick_and_install_plugin');
export const installOcrModels = () => safeInvoke<string>('install_ocr_models');

export interface OcrModelsStatus {
  /** PP-OCR (Japanese / Chinese / English), the default engine. */
  ppocr: boolean;
  /** PP-OCR が現行世代（PP-OCRv6）。false：旧版で取得した PP-OCRv5 で動作中。 */
  current: boolean;
  /** ocrs (Latin only), the fallback engine. */
  ocrs: boolean;
  /** PP-OCR の高速実行部品（ONNX Runtime）が取得済み、または当該環境で対象外。 */
  fast: boolean;
  dir: string;
}
export const ocrModelsStatus = () =>
  safeInvoke<OcrModelsStatus>('ocr_models_status', undefined, {
    ppocr: false,
    current: false,
    ocrs: false,
    fast: false,
    dir: ''
  });

/** 標準モデルの対象外の文字体系（韓国語・キリル文字等）と、その認識モデルの取得状態。 */
export interface OcrScript {
  id: string;
  /** 取得量（バイト）。 */
  size: number;
  installed: boolean;
}
export const ocrScriptsStatus = () => safeInvoke<OcrScript[]>('ocr_scripts_status', undefined, []);
/** 進捗は onModelProgress（filename：ocrScriptFile(id)）で通知。 */
export const installOcrScript = (id: string) => safeInvoke<void>('install_ocr_script', { id });
export const deleteOcrScript = (id: string) => safeInvoke<void>('delete_ocr_script', { id });
/** 文字体系の認識モデルのファイル名（進捗通知の filename）。 */
export const ocrScriptFile = (id: string) => `ppocrv5-rec-${id}.onnx`;

/** Fired after settings are saved, so every window can apply them at once. */
export const onSettingsChanged = (handler: (s: Settings) => void): Promise<UnlistenFn> =>
  listen<Settings>('moonlens://settings-changed', (e) => handler(e.payload));

/** A GGUF model the built-in translation engine can run. */
export interface LlmModel {
  id: string;
  label: string;
  /** 文言のキー ID（表示時に `t()` で取得）。 */
  note: string;
  file: string;
  url: string;
  size: number;
  sha256: string;
  license: string;
  license_url: string;
  style: 'chat_template' | 'pfml3';
  /** Licence id the user must accept before download (see `LICENSES`); null when none. */
  consent: string | null;
  installed: boolean;
  partial_bytes: number;
  downloading: boolean;
}

export type LlmRole = 'translate' | 'assistant';

export interface LlmModels {
  /** Model used for translation. */
  selected: string;
  /** Model used for explain / summarize. */
  assistant: string;
  dir: string;
  models: LlmModel[];
  /** Licence ids the user already accepted. */
  accepted_licenses: string[];
}

export interface LlmProgress {
  id: string;
  downloaded: number;
  total: number;
  fraction: number;
}

export const listLlmModels = () =>
  safeInvoke<LlmModels>('list_llm_models', undefined, {
    selected: '',
    assistant: '',
    dir: '',
    models: [],
    accepted_licenses: []
  });
export const installLlmModel = (id: string) => safeInvoke<string>('install_llm_model', { id });
/** Record acceptance of a model licence (idempotent). */
export const acceptLicense = (id: string) => safeInvoke<void>('accept_license', { id });
export const cancelLlmDownload = (id: string) => safeInvoke<boolean>('cancel_llm_download', { id });
export const deleteLlmModel = (id: string) => safeInvoke<void>('delete_llm_model', { id });
export const selectLlmModel = (id: string, role: LlmRole = 'translate') =>
  safeInvoke<void>('select_llm_model', { id, role });
export const onLlmProgress = (handler: (p: LlmProgress) => void): Promise<UnlistenFn> =>
  listen<LlmProgress>('moonlens://llm-model-progress', (e) => handler(e.payload));
/** A whisper.cpp model the built-in speech recognition can run. */
export interface AsrModel {
  id: string;
  label: string;
  /** 文言のキー ID（表示時に `t()` で取得）。 */
  note: string;
  file: string;
  url: string;
  size: number;
  sha256: string;
  license: string;
  license_url: string;
  installed: boolean;
  partial_bytes: number;
  downloading: boolean;
}

export interface AsrModels {
  /** Model used for live subtitles. */
  selected: string;
  dir: string;
  models: AsrModel[];
}

export const listAsrModels = () =>
  safeInvoke<AsrModels>('list_asr_models', undefined, { selected: '', dir: '', models: [] });
export const installAsrModel = (id: string) => safeInvoke<string>('install_asr_model', { id });
export const cancelAsrDownload = (id: string) => safeInvoke<boolean>('cancel_asr_download', { id });
export const deleteAsrModel = (id: string) => safeInvoke<void>('delete_asr_model', { id });
export const selectAsrModel = (id: string) => safeInvoke<void>('select_asr_model', { id });
export const onAsrProgress = (handler: (p: LlmProgress) => void): Promise<UnlistenFn> =>
  listen<LlmProgress>('moonlens://asr-model-progress', (e) => handler(e.payload));
export const showMain = () => safeInvoke<void>('show_main');
export const hideOverlay = () => safeInvoke<void>('hide_overlay');

/** Events of one image translation run carry the caller's run id. */
export type ImageRunEvent<T> = T & { run: number };

/**
 * Translate an image file (PNG / JPEG / WebP). The bytes go as the raw
 * request body; `moonlens://image-analysis` and `moonlens://image-progress`
 * stream the blocks to this window while it runs.
 */
export async function translateImage(
  bytes: Uint8Array,
  run: number,
  targetLanguage?: string
): Promise<CaptureFlowResult> {
  if (!hasTauri()) throw new Error('Tauri not available: translate_image');
  const headers: Record<string, string> = { run: String(run) };
  if (targetLanguage) headers.target = targetLanguage;
  return (await invoke('translate_image', bytes, { headers })) as CaptureFlowResult;
}
/** Bytes of an image file dropped on the window (the webview only gets its path). */
export const readImageFile = (path: string) => safeInvoke<ArrayBuffer>('read_image_file', { path });
export const onImageAnalysis = (handler: (e: ImageRunEvent<CaptureFlowResult>) => void) =>
  listen<ImageRunEvent<CaptureFlowResult>>('moonlens://image-analysis', (e) => handler(e.payload));
export const onImageProgress = (handler: (e: ImageRunEvent<TranslationProgress>) => void) =>
  listen<ImageRunEvent<TranslationProgress>>('moonlens://image-progress', (e) => handler(e.payload));
/** Open a window-translation window whose body covers `rect` (or under
 * the mouse pointer); `targetLanguage` defaults to the configured target.
 * Resolves to the window id. */
export const openLens = (rect: CaptureFlowRect | null = null, targetLanguage?: string | null) =>
  safeInvoke<string>('open_lens', { rect, targetLanguage: targetLanguage ?? null });
/** Open one more window next to window `id` (same size and language). */
export const openLensBeside = (id: string) => safeInvoke<string>('open_lens', { beside: id });
export const setLensLanguage = (id: string, targetLanguage: string | null) =>
  safeInvoke<void>('set_lens_language', { id, targetLanguage });
/** ウィンドウ翻訳の読取方式。字幕：逐次表示の完了後に前文脈付きで翻訳。UI：読取単位で翻訳し、前文脈の継承なし。 */
export type LensReadMode = 'subtitle' | 'ui';
/** 読取方式の変更（次回の読取から適用）。 */
export const setLensReadMode = (id: string, mode: LensReadMode) =>
  safeInvoke<void>('set_lens_read_mode', { id, mode });
export const setLensOperate = (id: string, on: boolean) => safeInvoke<void>('set_lens_operate', { id, on });
export const setLensInteractive = (id: string, on: boolean) =>
  safeInvoke<void>('set_lens_interactive', { id, on });
/** テキスト表示をウィンドウ外へ配置（`null`：ウィンドウ外表示を終了）。 */
export const setLensTextDock = (
  id: string,
  dock: 'top' | 'bottom' | 'left' | 'right' | 'free' | null
) => safeInvoke<void>('set_lens_text_dock', { id, dock });

export const runTranslate = (args: {
  text: string;
  source_language: string;
  target_language: string;
  /** Earlier `[source, translation]` pairs of the same document. */
  history?: [string, string][];
}) => safeInvoke<TaskResult>('run_translate', { args });

/** Save a text-translation page result to history (respects the
 * keep-history setting on the backend). */
export const recordTranslation = (args: {
  source_text: string;
  result_text: string;
  provider: string | null;
  via_cloud: boolean;
}) => safeInvoke<void>('record_translation', { args });

export const runIntent = (imagePng: Uint8Array) =>
  safeInvoke<string>('run_intent', { imagePng: Array.from(imagePng) });

/** Open an http(s) link (e.g. a decoded QR code) in the default browser. */
export const openExternalUrl = (url: string) => safeInvoke<void>('open_external_url', { url });

export const runExplain = (prompt: string, imagePng?: Uint8Array) =>
  safeInvoke<TaskResult>('run_explain', {
    args: { prompt, image_png: imagePng ? Array.from(imagePng) : null }
  });

export const captureAndProcess = (region?: {
  monitor_index: number;
  x: number;
  y: number;
  w: number;
  h: number;
}) => safeInvoke<CaptureFlowResult>('capture_and_process', { args: region ?? null });

export const onOverlay = (handler: (event: OverlayEvent) => void): Promise<UnlistenFn> =>
  listen<OverlayEvent>('moonlens://overlay', (e) => handler(e.payload));

export const onHotkey = (handler: (action: string) => void): Promise<UnlistenFn> =>
  listen<string>('moonlens://hotkey', (e) => handler(e.payload));

/** 音声翻訳の処理段階：音声待機・音声認識・翻訳。 */
export type SubtitlePhase = 'waiting' | 'recognizing' | 'translating' | 'revising';

/** 音声翻訳ウィンドウの音源：PC 全体、アプリ単位（プロセスツリー）、マイク（対面会話）。 */
export type AudioSource = { kind: 'system' } | { kind: 'app'; pid: number } | { kind: 'microphone' };

/** 対面会話の話者：利用者本人、または相手。 */
export type Speaker = 'own' | 'partner';

/** 音声出力ストリームを保持するアプリ。 */
export type AudioApp = {
  /** 取得対象プロセス（子プロセス含む）。 */
  pid: number;
  /** 実行ファイル名（拡張子なし）。 */
  name: string;
  /** アプリのウィンドウ題名。 */
  title: string | null;
  /** 音声再生中。 */
  playing: boolean;
};

export type AudioApps = {
  /** アプリ単位の音声取得に対応する OS。 */
  supported: boolean;
  apps: AudioApp[];
};

/** 音声翻訳ウィンドウ 1 枚の音源と稼働状態。 */
export type SubtitleSession = { source: AudioSource; device: string; running: boolean };

export type SubtitleEvent =
  | { kind: 'started'; device: string; source: AudioSource }
  | { kind: 'stopped' }
  | { kind: 'status'; phase: SubtitlePhase }
  /** `speaker`：対面会話の話者（訳文は相手側の言語）。字幕では省略。 */
  | { kind: 'chunk'; id: number; original: string; translated: string; speaker?: Speaker }
  | {
      /** 文単位の再翻訳結果。`replaces` の各行を 1 行（＋次文の冒頭 `rest`）へ置換。`original` が空の場合は遮断済みの文：行の除去のみ。 */
      kind: 'revised';
      replaces: number[];
      id: number;
      original: string;
      translated: string;
      rest: { id: number; original: string; translated: string } | null;
    }
  /** 短時間に反復した文：遮断リストへの追加候補。 */
  | { kind: 'candidate'; phrase: string }
  | { kind: 'error'; message: string };

/** `window`：イベントの対象ウィンドウ（ラベル）。イベント固有のキー（行の `id` 等）と重複不可。 */
export type SubtitleMessage = SubtitleEvent & { window: string };

export const toggleSubtitle = () => safeInvoke<boolean>('toggle_subtitle');
/** 稼働中の音声翻訳ウィンドウの有無。 */
export const subtitleState = () => safeInvoke<boolean>('subtitle_state', undefined, false);
/** 音声翻訳ウィンドウを全停止・全閉鎖。 */
export const stopSubtitle = () => safeInvoke<void>('stop_subtitle');
export const startSubtitle = () => safeInvoke<void>('start_subtitle');
/** 対面会話を開始：マイク入力のウィンドウを表示（表示済みの場合は前面化・再開）。 */
export const startConversation = () => safeInvoke<void>('start_conversation');
/** 音声翻訳の遮断リスト（`subtitle.blocked_phrases`）へ追加。 */
export const blockSubtitlePhrase = (phrase: string) =>
  safeInvoke<void>('block_subtitle_phrase', { phrase });
/** 音声翻訳ウィンドウを追加（呼出元ウィンドウの隣へ配置）。 */
export const openSubtitleWindow = (source?: AudioSource) =>
  safeInvoke<string>('open_subtitle_window', { source });
/** 呼出元の音声翻訳ウィンドウを停止・閉鎖。 */
export const closeSubtitleWindow = () => safeInvoke<void>('close_subtitle_window');
/** 呼出元の音声翻訳ウィンドウの音源を変更（停止後は同一音源で再開）。 */
export const setSubtitleSource = (source: AudioSource) =>
  safeInvoke<void>('set_subtitle_source', { source });
export const subtitleSession = () =>
  safeInvoke<SubtitleSession | null>('subtitle_session', undefined, null);
export const listAudioApps = () =>
  safeInvoke<AudioApps>('list_audio_apps', undefined, { supported: false, apps: [] });
/** 全ウィンドウ分のイベントを受信（`id` で対象を判別）。 */
export const onSubtitle = (handler: (event: SubtitleMessage) => void): Promise<UnlistenFn> =>
  listen<SubtitleMessage>('moonlens://subtitle', (e) => handler(e.payload));

/**
 * 音声翻訳の入力レベル（波形表示用、50 ms 間隔）。送出先は対象ウィンドウ限定のため、
 * 呼出元ウィンドウ宛のイベントのみ受信。
 */
export const onSubtitleLevel = (
  handler: (event: import('./subtitle-level').LevelEvent) => void
): Promise<UnlistenFn> =>
  getCurrentWebviewWindow().listen<import('./subtitle-level').LevelEvent>(
    'moonlens://subtitle-level',
    (e) => handler(e.payload)
  );

export type PermissionStatus = 'granted' | 'denied' | 'unknown';

export interface PermissionReport {
  screen_capture: PermissionStatus;
  accessibility: PermissionStatus;
  microphone: PermissionStatus;
  input_monitoring: PermissionStatus;
}

const defaultPermissions: PermissionReport = {
  screen_capture: 'granted',
  accessibility: 'granted',
  microphone: 'granted',
  input_monitoring: 'granted'
};

export interface OnboardingStatus {
  platform: 'macos' | 'windows' | 'linux' | 'other';
  needs_macos_permissions_review: boolean;
  /** 初回セットアップ（構成診断・推奨モデル・一括取得）の表示要否。 */
  needs_setup: boolean;
  macos_permissions: PermissionReport;
}

export const getOnboardingStatus = () =>
  safeInvoke<OnboardingStatus>('get_onboarding_status', undefined, {
    platform: 'other',
    needs_macos_permissions_review: false,
    needs_setup: false,
    macos_permissions: defaultPermissions
  });

export interface GpuDevice {
  name: string;
  backend: string;
  memory_total: number;
  integrated: boolean;
}

export interface SystemProfile {
  os: string;
  cpu_name: string;
  cpu_cores: number;
  ram_total: number;
  gpus: GpuDevice[];
  disk_free: number | null;
  use_gpu: boolean;
}

export interface ModelPick {
  model: string;
  /** 文言のキー ID（表示時に `t()` で取得）。 */
  reason: string;
}

/** 機能別の推奨モデル（ocr は PP-OCRv5 固定で 'ppocr'）。 */
export interface Recommendation {
  translate: ModelPick;
  assistant: ModelPick;
  ocr: ModelPick;
  asr: ModelPick;
}

export interface SystemReport {
  profile: SystemProfile;
  /** 推論に使用する GPU（null：CPU）。 */
  accelerator: GpuDevice | null;
  recommended: Recommendation;
}

export const getSystemProfile = () => safeInvoke<SystemReport>('get_system_profile');
export const completeSetup = () => safeInvoke<void>('complete_setup');

export const markMacosPermissionsReviewed = () =>
  safeInvoke<void>('mark_macos_permissions_reviewed');

export const openMacosPrivacyPane = (
  pane: 'screen_capture' | 'accessibility' | 'microphone' | 'input_monitoring'
) => safeInvoke<void>('open_macos_privacy_pane', { pane });

export const macosPermissionsSnapshot = () =>
  safeInvoke<PermissionReport>('macos_permissions_snapshot', undefined, defaultPermissions);

export const macosRequestScreenCapture = () =>
  safeInvoke<PermissionStatus>('macos_request_screen_capture', undefined, 'granted');

export interface ModelProgress {
  filename: string;
  downloaded: number;
  total: number | null;
  fraction: number | null;
}

export const onModelProgress = (
  handler: (p: ModelProgress) => void
): Promise<UnlistenFn> =>
  listen<ModelProgress>('moonlens://ocr-model-progress', (e) => handler(e.payload));

export const checkPluginUpdates = () =>
  safeInvoke<
    {
      plugin_id: string;
      installed_version: string;
      latest_version: string;
      download_url: string | null;
      notes: string | null;
    }[]
  >('check_plugin_updates', undefined, []);

export interface LensDescriptor {
  id: string;
  title: string;
  insets: { top: number; bottom: number; left: number; right: number };
  /** Reads the region continuously (re-translates on change). */
  continuous: boolean;
  /** Language it translates into; null = the configured target. */
  target_language: string | null;
  read_mode: LensReadMode;
  /** View state the window reported, or was opened with (`LensView`). */
  view: unknown;
}

export const closeLensWindow = (id: string) => safeInvoke<void>('close_lens_window', { id });
export const listLensWindows = () =>
  safeInvoke<LensDescriptor[]>('list_lens_windows', undefined, []);
/** Pause every lens window, or resume. Resolves to the new state. */
/** Whether extra triggers (mouse button, double tap, gamepad) work here. */
export const triggerSupported = () => safeInvoke<boolean>('trigger_supported', undefined, false);
/**
 * Wait for the user to perform a trigger; resolves with its text form
 * (`Mouse4`, `Double:Control`, `Pad:Back+Y`), or null when nothing came
 * in time or the wait was cancelled.
 */
export const captureTrigger = () => safeInvoke<string | null>('capture_trigger', undefined, null);
export const cancelTriggerCapture = () => safeInvoke<void>('cancel_trigger_capture', undefined, undefined as void);
export const toggleLensPause = () => safeInvoke<boolean>('toggle_lens_pause', undefined, false);
/** All-window controls as they are now (for a window that just opened). */
export const lensControls = () =>
  safeInvoke<{ paused: boolean; original: boolean }>('lens_controls', undefined, {
    paused: false,
    original: false
  });
export const captureLensNow = (id: string) =>
  safeInvoke<CaptureFlowResult>('capture_lens_now', { id });

/** Where a lens window was before `fitLens` (physical screen pixels). */
export interface LensBefore {
  rect: { x: number; y: number; w: number; h: number };
  maximized: boolean;
}
/** Shrink a lens window to `band` ([x, y, w, h] inside its observed range, physical px). */
export const fitLens = (id: string, band: [number, number, number, number]) =>
  safeInvoke<LensBefore>('fit_lens', { id, band });
export const restoreLens = (id: string, before: LensBefore) =>
  safeInvoke<void>('restore_lens', { id, before });

export interface HistoryEntry {
  id: number;
  created_at: number;
  kind: string;
  source_text: string | null;
  result_text: string | null;
  provider: string | null;
  via_cloud: boolean;
  image_path: string | null;
}

export const listHistory = (args: { limit?: number; query?: string } = {}) =>
  safeInvoke<HistoryEntry[]>(
    'list_history',
    { limit: args.limit ?? null, query: args.query ?? null },
    []
  );

/** 個別削除。`record` は音声翻訳の記録（削除時点の全行）：取消時に復元へ引渡し。 */
export const deleteHistoryEntry = (id: number) =>
  safeInvoke<{ deleted: number; record: SubtitleRecord | null }>(
    'delete_history_entry',
    { id },
    { deleted: 0, record: null }
  );

/** 個別削除の取消：削除済みの履歴を元の id・日時で復元。音声翻訳は記録（行）も復元。 */
export const restoreHistoryEntry = (entry: HistoryEntry, record?: SubtitleRecord | null) =>
  safeInvoke<number>('restore_history_entry', { entry, record: record ?? null }, 0);

/** 音声翻訳の 1 行（時刻付き）。 */
export interface SubtitleLine {
  /** 記録内の順序（0 始まり）。 */
  seq: number;
  /** 実行開始からの経過時間（ms）。 */
  start_ms: number;
  end_ms: number;
  original: string;
  /** 翻訳失敗・不要の場合は空文字。 */
  translated: string;
}

/** 音声翻訳 1 回分の記録。 */
export interface SubtitleRecord {
  history_id: number;
  /** 音源（取得対象のデバイス・アプリの名称）。 */
  source: string;
  /** 実行時の設定値（auto の場合あり）。 */
  source_language: string;
  target_language: string;
  /** 対面会話の記録：行ごとに翻訳方向が相違（上記の言語は相手側の発話のみ）。 */
  conversation?: boolean;
  /** 行の変更回数。全行の置換時に取得時点の値を指定（記録中の行の消失防止）。 */
  revision: number;
  lines: SubtitleLine[];
}

export type SubtitleExportFormat = 'srt' | 'txt';
export type SubtitleExportContent = 'translated' | 'original' | 'both';

/** 音声翻訳の記録の取得。記録のない履歴は null。 */
export const getSubtitleRecord = (id: number) =>
  safeInvoke<SubtitleRecord | null>('get_subtitle_record', { id }, null);

/**
 * 記録の全行の置換（再翻訳とその取消）。履歴の原文・訳文も再構築。
 * `revision` は行の取得時点の値：取得後に記録が変化した場合は失敗。戻り値は新しい値。
 */
export const replaceSubtitleLines = (id: number, revision: number, lines: SubtitleLine[]) =>
  safeInvoke<number>('replace_subtitle_lines', { id, revision, lines });

/** 記録の書出し。戻り値は保存先、保存先の選択を取消した場合は null。 */
export const exportSubtitleRecord = (
  id: number,
  format: SubtitleExportFormat,
  content: SubtitleExportContent
) => safeInvoke<string | null>('export_subtitle_record', { id, format, content }, null);

/** 字幕生成の対象ファイルを選択。取消は null。 */
export const pickMediaFile = () => safeInvoke<string | null>('pick_media_file', undefined, null);

/** ファイルの字幕生成を開始。進行は `onFileSubtitle` で受信。 */
export const startFileSubtitle = (path: string) =>
  safeInvoke<import('./file-subtitle').FileSubtitleStatus>('start_file_subtitle', { path });

export const cancelFileSubtitle = () =>
  safeInvoke<import('./file-subtitle').FileSubtitleStatus>('cancel_file_subtitle');

export const fileSubtitleStatus = () =>
  safeInvoke<import('./file-subtitle').FileSubtitleStatus | null>('file_subtitle_status', undefined, null);

/** 生成済みの字幕の書出し。戻り値は保存先、保存先の選択を取消した場合は null。 */
export const exportFileSubtitle = (format: SubtitleExportFormat, content: SubtitleExportContent) =>
  safeInvoke<string | null>('export_file_subtitle', { format, content }, null);

export const onFileSubtitle = (
  handler: (update: import('./file-subtitle').FileSubtitleUpdate) => void
): Promise<UnlistenFn> =>
  listen<import('./file-subtitle').FileSubtitleUpdate>('moonlens://file-subtitle', (e) => handler(e.payload));

export const clearHistory = () => safeInvoke<number>('clear_history', undefined, 0);

/** 訳文キャッシュの使用量（端末内保存分）。 */
export interface TranslationCacheUsage {
  /** キャッシュファイルの容量（バイト）。 */
  bytes: number;
  entries: number;
  /** アプリ単位の内訳。scope が空文字：追従先アプリ未指定の共通分。 */
  scopes: { scope: string; entries: number; bytes: number }[];
}
export const translationCacheUsage = () =>
  safeInvoke<TranslationCacheUsage>('translation_cache_usage', undefined, {
    bytes: 0,
    entries: 0,
    scopes: []
  });
/** 訳文キャッシュの消去。app 省略：全消去。戻り値：消去件数。 */
export const clearTranslationCache = (app?: string) =>
  safeInvoke<number>('clear_translation_cache', { app: app ?? null }, 0);

/* ------------------------------ lifecycle ------------------------------ */

/** A problem found outside a user action (start-up, config file, hotkeys). */
export interface Notice {
  id: string;
  message: string;
  /** In-app page that fixes it, e.g. "/settings/#section-hotkeys". */
  action: string | null;
}

export const listNotices = () => safeInvoke<Notice[]>('list_notices', undefined, []);
export const dismissNotice = (id: string) => safeInvoke<void>('dismiss_notice', { id });
export const onNotices = (handler: (n: Notice[]) => void): Promise<UnlistenFn> =>
  listen<Notice[]>('moonlens://notices', (e) => handler(e.payload));

/** A problem in config.toml with its proposed fix (settings::validate). */
export interface ConfigIssue {
  id: string;
  /** Location as written in config.toml, e.g. "providers.ocr.chain[0]". */
  path: string;
  problem: string;
  /** What the fix does; null when there is nothing safe to propose. */
  fix_label: string | null;
}

export const listConfigIssues = () => safeInvoke<ConfigIssue[]>('list_config_issues', undefined, []);
/** Apply the fixes of the given issues (all when empty); returns how many applied. */
export const applyConfigFixes = (ids: string[]) => safeInvoke<number>('apply_config_fixes', { ids });
export const onConfigIssues = (handler: (i: ConfigIssue[]) => void): Promise<UnlistenFn> =>
  listen<ConfigIssue[]>('moonlens://config-issues', (e) => handler(e.payload));

/** Show the main window on an in-app page (from a window-translation window). */
export const openMainRoute = (path: string) => safeInvoke<void>('open_main_route', { path });
/** The backend asks the main window to navigate (tray, hotkeys). */
export const onNavigate = (handler: (path: string) => void): Promise<UnlistenFn> =>
  listen<string>('moonlens://navigate', (e) => handler(e.payload));
/** Hide the main window to the tray (its close button). */
export const hideMain = () => safeInvoke<void>('hide_main');
export const quitApp = () => safeInvoke<void>('quit_app');

export interface DataLocations {
  /** App version and build commit. */
  version: string;
  config_dir: string;
  data_dir: string;
  models_dir: string;
  log_dir: string;
  models_bytes: number;
  data_bytes: number;
}
export const dataLocations = () => safeInvoke<DataLocations>('data_locations');

/** A model file no catalog entry owns (left by an earlier version). */
export interface OrphanFile {
  name: string;
  size: number;
}
export const listOrphanLlmFiles = () =>
  safeInvoke<OrphanFile[]>('list_orphan_llm_files', undefined, []);
export const deleteOrphanLlmFile = (name: string) =>
  safeInvoke<void>('delete_orphan_llm_file', { name });

// ---- 他アプリのウィンドウへの吸着（attach.rs、型は `$lib/attach`） ----
/** ウィンドウ `label` の吸着先候補（直下のウィンドウが先頭）。 */
export const listAttachTargets = (label: string) =>
  safeInvoke<AttachCandidate[]>('list_attach_targets', { label }, []);
/** ウィンドウを表示中のアプリ（識別キー順）。アプリ別用語集の追加候補。 */
export const listWindowApps = () =>
  safeInvoke<{ key: string; name: string }[]>('list_window_apps', undefined, []);
/** `label` を `target`（候補の `id`）へ吸着、または吸着中の配置を変更。 */
export const attachWindow = (label: string, target: number, dock: AttachDock) =>
  safeInvoke<void>('attach_window', { label, target, dock });
export const detachWindow = (label: string) => safeInvoke<void>('detach_window', { label });
export const windowAttachment = (label: string) =>
  safeInvoke<Attachment>('window_attachment', { label }, { target: null, dock: null });

// ---- 配置セット（layout_sets.rs、型は `$lib/layout-sets`） ----
export const listLayoutSets = () => safeInvoke<LayoutSet[]>('list_layout_sets', undefined, []);
/** 表示中のウィンドウ翻訳を保存。`id` 指定時は当該セットへ上書。 */
export const saveLayoutSet = (id?: string) =>
  safeInvoke<LayoutSet>('save_layout_set', { id: id ?? null });
export const updateLayoutSet = (id: string, patch: { name?: string; autoRestore?: boolean }) =>
  safeInvoke<void>('update_layout_set', {
    id,
    name: patch.name ?? null,
    autoRestore: patch.autoRestore ?? null
  });
/** 削除。戻り値（位置とセット）は {@link putLayoutSet} による復元用。 */
export const deleteLayoutSet = (id: string) =>
  safeInvoke<[number, LayoutSet] | null>('delete_layout_set', { id });
export const putLayoutSet = (set: LayoutSet, index: number) =>
  safeInvoke<void>('put_layout_set', { set, index });
/** セットのウィンドウを表示。戻り値：表示した個数。 */
export const restoreLayoutSet = (id: string) => safeInvoke<number>('restore_layout_set', { id });
export const onLayoutSets = (handler: () => void): Promise<UnlistenFn> =>
  listen(LAYOUT_SETS_EVENT, () => handler());
/** ウィンドウ翻訳の表示状態を報告（配置セットの保存対象）。 */
export const setLensView = (id: string, view: LensView) =>
  safeInvoke<void>('set_lens_view', { id, view });
// ---- 配信用ウィンドウ（stream.rs、型は `$lib/stream`） ----
export const streamState = (label: string) =>
  safeInvoke<StreamState>('stream_state', { label }, NO_STREAM);
/** `label` の吸着先の配信用ウィンドウを生成（`on`）・終了。 */
export const setStream = (label: string, on: boolean) =>
  safeInvoke<StreamState>('set_stream', { label, on }, NO_STREAM);
/** `label` が表示中の訳文（配置済み）を配信用ウィンドウへ送信。 */
export const setStreamPaint = (label: string, items: StreamItem[]) =>
  safeInvoke<void>('set_stream_paint', { label, items });
