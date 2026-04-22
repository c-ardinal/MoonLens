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
/** Close the region-selection overlay. Backend-side path, always
 *  resolves even if the webview's `window.hide()` call path is
 *  permission-blocked or the window is in a partly-initialised
 *  state during show/hide transitions. */
export const cancelRegionSelection = () =>
  safeInvoke<void>('cancel_region_selection');

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

/**
 * One hotkey the backend could not bind on startup or on a settings
 * reload. Mirror of the Rust struct in `src-tauri/src/hotkey.rs`.
 */
export interface HotkeyFailure {
  /** Logical action id, e.g. "capture_region" or a custom shortcut id. */
  id: string;
  /** The accelerator string that failed, e.g. "Ctrl+Shift+Space". */
  accelerator: string;
  /** Error message from `tauri_plugin_global_shortcut`. */
  error: string;
}

export interface HotkeyRegistrationReport {
  failed: HotkeyFailure[];
}

export const onHotkeyFailure = (
  handler: (report: HotkeyRegistrationReport) => void
): Promise<UnlistenFn> =>
  listen<HotkeyRegistrationReport>('moonlens://hotkey-failure', (e) => handler(e.payload));

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

/** Start-of-flow signal emitted by the backend lens watcher. Clear the
 * lens UI's streaming buffer when this arrives for the current lens. */
export interface LensFlowStart {
  id: string;
  flow_id: number;
}

/** OCR result + axis-aligned bboxes for each detected line. Emitted
 * once per flow, immediately after OCR completes and before any
 * translation arrives. The UI renders a positioned card per bbox so
 * tokens can stream into the right place. */
/** Optional linear gradient detected on the 2px rim of the bbox.
 * When present, the lens UI renders
 * `background: linear-gradient(<angle>deg, rgb(from), rgb(to))` so
 * the card blends into gradient / photo source instead of posting a
 * flat solid block. `bg_color` is still the safe fallback. */
export interface LensBgGradient {
  /** CSS `linear-gradient` angle in degrees (0 = to top, clockwise). */
  angle: number;
  /** Alias of `stops[0]`. Kept for legacy frontends that render a
   *  two-colour gradient only. */
  from: [number, number, number];
  /** Alias of `stops[n-1]`. */
  to: [number, number, number];
  /** Equally-spaced colour stops along the axis (2..=5). Render
   *  `linear-gradient(<angle>deg, rgb(stops[0]) 0%, rgb(stops[1]) X%,
   *  …, rgb(stops[n-1]) 100%)` to capture multi-stop fades like
   *  vignettes. */
  stops?: [number, number, number][];
}

export interface LensOcrLine {
  line_index: number;
  text: string;
  /** [x, y, w, h] in image pixel coordinates — the **tiled** card
   *  bbox (expanded vertically to adjacent-line midpoints plus
   *  small horizontal padding). This is where the card renders. */
  bbox: [number, number, number, number];
  /** Original OCR bbox before tile expansion, in image pixel
   *  coordinates. Used by the frontend to size the font to the
   *  source glyph height rather than the inflated card height. */
  source_bbox: [number, number, number, number];
  /** Mean RGB of the 2px rim (solid-fill fallback). */
  bg_color: [number, number, number];
  /** Present when the rim has a dominant gradient axis. */
  bg_gradient?: LensBgGradient | null;
  /** Source-text colour sampled from inside the bbox. */
  fg_color: [number, number, number];
}

export interface LensOcrLines {
  id: string;
  flow_id: number;
  image_w: number;
  image_h: number;
  lines: LensOcrLine[];
}

/** A single streaming token for a specific line within a flow. */
export interface LensLineChunk {
  id: string;
  flow_id: number;
  line_index: number;
  delta: string;
  done: boolean;
}

/** Terminal marker: every line of the flow has finished (or failed). */
export interface LensFlowDone {
  id: string;
  flow_id: number;
  line_count: number;
}

/** Pre/post-capture mask signal. When `masked=true`, the lens UI
 * should hide all cards (opacity 0) so the screen capture can see
 * through our transparent window to the source underneath. This
 * replaces window-level hide/show which caused a visible blink. */
export interface LensMask {
  id: string;
  masked: boolean;
}

export const onLensFlowStart = (
  handler: (event: LensFlowStart) => void
): Promise<UnlistenFn> =>
  listen<LensFlowStart>('moonlens://lens-flow-start', (e) => handler(e.payload));

export const onLensOcrLines = (
  handler: (event: LensOcrLines) => void
): Promise<UnlistenFn> =>
  listen<LensOcrLines>('moonlens://lens-ocr-lines', (e) => handler(e.payload));

export const onLensLineChunk = (
  handler: (event: LensLineChunk) => void
): Promise<UnlistenFn> =>
  listen<LensLineChunk>('moonlens://lens-line-chunk', (e) => handler(e.payload));

export const onLensFlowDone = (
  handler: (event: LensFlowDone) => void
): Promise<UnlistenFn> =>
  listen<LensFlowDone>('moonlens://lens-flow-done', (e) => handler(e.payload));

export const onLensMask = (handler: (event: LensMask) => void): Promise<UnlistenFn> =>
  listen<LensMask>('moonlens://lens-mask', (e) => handler(e.payload));

/** Summary of what the global panic button actually stopped. Mirror of
 * the Rust struct in `src-tauri/src/ipc.rs`. */
export interface StopAllReport {
  lens_count: number;
  subtitle_was_running: boolean;
  overlay_was_open: boolean;
}

/** Tear down every Lens window, halt subtitle capture, and hide the
 * overlay. Safe to call when nothing is running (all fields will be 0
 * / false in the returned report). */
export const stopAll = () =>
  safeInvoke<StopAllReport>('stop_all', undefined, {
    lens_count: 0,
    subtitle_was_running: false,
    overlay_was_open: false
  });

/** Fired after `stopAll` completes (from either the hotkey or the
 * command). Other windows listen so they can reset local streaming
 * state. */
export const onStopAll = (handler: (report: StopAllReport) => void): Promise<UnlistenFn> =>
  listen<StopAllReport>('moonlens://stop-all', (e) => handler(e.payload));

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
