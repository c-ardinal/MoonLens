/**
 * Typed wrappers around Tauri `invoke` calls.
 * Every IPC surface the frontend needs lives here so it is easy to mock
 * in tests and audit what the backend can be asked to do.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

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
}

/** Sent before each translation request of a capture (`moonlens://selector-progress`). */
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

/** Selection on the selector's frozen frame, in frame pixels. */
export interface FrameRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface FrameReady {
  width: number;
  height: number;
  auto_full: boolean;
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
/** The bundled defaults, for per-field "初期値に戻す". */
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
  /** PP-OCRv5 (Japanese / Chinese / English), the default engine. */
  ppocr: boolean;
  /** ocrs (Latin only), the fallback engine. */
  ocrs: boolean;
  dir: string;
}
export const ocrModelsStatus = () =>
  safeInvoke<OcrModelsStatus>('ocr_models_status', undefined, { ppocr: false, ocrs: false, dir: '' });

/** Fired after settings are saved, so every window can apply them at once. */
export const onSettingsChanged = (handler: (s: Settings) => void): Promise<UnlistenFn> =>
  listen<Settings>('moonlens://settings-changed', (e) => handler(e.payload));

/** Pin state changed from the pin hotkey. */
export const onPinState = (handler: (pinned: boolean) => void): Promise<UnlistenFn> =>
  listen<boolean>('moonlens://pin-state', (e) => handler(e.payload));

/** A GGUF model the built-in translation engine can run. */
export interface LlmModel {
  id: string;
  label: string;
  note: string;
  file: string;
  url: string;
  size: number;
  sha256: string;
  license: string;
  license_url: string;
  style: 'chat_template' | 'pfml3';
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
    models: []
  });
export const installLlmModel = (id: string) => safeInvoke<string>('install_llm_model', { id });
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

/** PNG bytes of the frame the selector froze when it opened. */
export const getFrozenFrame = () => safeInvoke<ArrayBuffer>('get_frozen_frame');
/** OCR + translate a selection of the frozen frame (null = whole frame). */
export const processFrozenSelection = (rect: FrameRect | null, targetLanguage?: string | null) =>
  safeInvoke<CaptureFlowResult>('process_frozen_selection', {
    rect,
    targetLanguage: targetLanguage ?? null
  });
export const retranslateSelection = (targetLanguage: string) =>
  safeInvoke<CaptureFlowResult>('retranslate_selection', { targetLanguage });
export const closeSelector = () => safeInvoke<void>('close_selector');
/** Open the Lens selector (hides the main window first). */
export const startRegionSelection = (full = false) =>
  safeInvoke<void>('start_region_selection', { full });

export const pinOverlay = (args: CaptureFlowRect) =>
  safeInvoke<void>('pin_overlay', { args });
export const unpinOverlay = () => safeInvoke<void>('unpin_overlay');

export const runTranslate = (args: {
  text: string;
  source_language: string;
  target_language: string;
}) => safeInvoke<TaskResult>('run_translate', { args });

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

export type SubtitleEvent =
  | { kind: 'started' }
  | { kind: 'stopped' }
  | { kind: 'chunk'; original: string; translated: string }
  | { kind: 'error'; message: string };

export const toggleSubtitle = () => safeInvoke<boolean>('toggle_subtitle');
export const subtitleState = () => safeInvoke<boolean>('subtitle_state', undefined, false);
export const stopSubtitle = () => safeInvoke<void>('stop_subtitle');
export const startSubtitle = () => safeInvoke<void>('start_subtitle');
export const onSubtitle = (handler: (event: SubtitleEvent) => void): Promise<UnlistenFn> =>
  listen<SubtitleEvent>('moonlens://subtitle', (e) => handler(e.payload));

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
  macos_permissions: PermissionReport;
}

export const getOnboardingStatus = () =>
  safeInvoke<OnboardingStatus>('get_onboarding_status', undefined, {
    platform: 'other',
    needs_macos_permissions_review: false,
    macos_permissions: defaultPermissions
  });

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
  auto_refresh_ms: number | null;
}

export const newLensWindow = () => safeInvoke<string>('new_lens_window');
export const closeLensWindow = (id: string) => safeInvoke<void>('close_lens_window', { id });
export const listLensWindows = () =>
  safeInvoke<LensDescriptor[]>('list_lens_windows', undefined, []);
export const captureLensNow = (id: string) =>
  safeInvoke<CaptureFlowResult>('capture_lens_now', { id });

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

export const deleteHistoryEntry = (id: number) =>
  safeInvoke<number>('delete_history_entry', { id }, 0);

export const clearHistory = () => safeInvoke<number>('clear_history', undefined, 0);

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

/** Show the main window on an in-app page (from the selector or a lens). */
export const openMainRoute = (path: string) => safeInvoke<void>('open_main_route', { path });
/** The backend asks the main window to navigate (tray, hotkeys). */
export const onNavigate = (handler: (path: string) => void): Promise<UnlistenFn> =>
  listen<string>('moonlens://navigate', (e) => handler(e.payload));
/** Hide the main window to the tray (its close button). */
export const hideMain = () => safeInvoke<void>('hide_main');
export const quitApp = () => safeInvoke<void>('quit_app');

export interface DataLocations {
  config_dir: string;
  data_dir: string;
  models_dir: string;
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
