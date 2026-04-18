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

export interface CaptureFlowResult {
  intent: Intent;
  source_text: string | null;
  result: TaskResult;
  source_rect: CaptureFlowRect | null;
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
export const listPlugins = () =>
  safeInvoke<PluginDescriptor[]>('list_plugins', undefined, []);
export const setPluginEnabled = (id: string, enabled: boolean) =>
  safeInvoke<void>('set_plugin_enabled', { id, enabled });
export const installPluginFromPath = (archivePath: string) =>
  safeInvoke<string>('install_plugin_from_path', { archivePath });
export const pickAndInstallPlugin = () =>
  safeInvoke<string | null>('pick_and_install_plugin');
export const installOcrModels = () => safeInvoke<string>('install_ocr_models');
export const showMain = () => safeInvoke<void>('show_main');
export const hideOverlay = () => safeInvoke<void>('hide_overlay');

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

export const runReverseImageSearch = (imageUrl: string) =>
  safeInvoke<TaskResult>('run_reverse_image_search', { imageUrl });

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

export interface OnboardingStatus {
  platform: 'macos' | 'windows' | 'linux' | 'other';
  needs_macos_permissions_review: boolean;
}

export const getOnboardingStatus = () =>
  safeInvoke<OnboardingStatus>('get_onboarding_status', undefined, {
    platform: 'other',
    needs_macos_permissions_review: false
  });

export const markMacosPermissionsReviewed = () =>
  safeInvoke<void>('mark_macos_permissions_reviewed');

export const openMacosPrivacyPane = (
  pane: 'screen_capture' | 'accessibility' | 'microphone' | 'input_monitoring'
) => safeInvoke<void>('open_macos_privacy_pane', { pane });

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
