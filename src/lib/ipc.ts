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
export const uninstallPlugin = (id: string) =>
  safeInvoke<void>('uninstall_plugin', { id });
export const revealPluginFolder = (id: string) =>
  safeInvoke<void>('reveal_plugin_folder', { id });
export const installOcrModels = () => safeInvoke<string>('install_ocr_models');

/** Status of the PaddleOCR PP-OCRv5 model files on disk. The
 * settings UI uses this to render either a "ダウンロード" CTA or
 * a "準備済み" badge with the on-disk size. */
export interface PaddleOcrModelStatus {
  dir: string;
  multi_present: boolean;
  korean_present: boolean;
  total_bytes: number;
}

export const paddleOcrModelsStatus = () =>
  safeInvoke<PaddleOcrModelStatus>('paddleocr_models_status', undefined, {
    dir: '',
    multi_present: false,
    korean_present: false,
    total_bytes: 0,
  });

/** Trigger the model download for the new Phase 28 PP-OCRv5 primary
 * OCR engine. `installKorean=true` (default) fetches the extra
 * Korean recogniser alongside the multilingual one for the
 * best-of-two Hangul router. Listens on
 * `moonlens://paddleocr-model-progress` for per-chunk events. */
export const installPaddleOcrModels = (installKorean: boolean = true) =>
  safeInvoke<string>('install_paddleocr_models', { installKorean });

export const onPaddleOcrModelProgress = (
  handler: (p: ModelProgress) => void,
): Promise<UnlistenFn> =>
  listen<ModelProgress>('moonlens://paddleocr-model-progress', (e) => handler(e.payload));

/** Status of the embedded LLM model on disk. Used by the settings
 * UI to decide whether to render the "ダウンロード" CTA or a
 * "準備済み" badge, and to surface the license advert so corporate
 * deployments can audit the bundle before approving. */
export interface EmbeddedModelStatus {
  path: string;
  present: boolean;
  size_bytes: number | null;
  /** Resolved `repo:filename` of the currently-installed model. */
  model_id: string;
  /** SPDX-style license advert (e.g. "Apache-2.0"). */
  license: string | null;
  /** Canonical license URL for click-through audit. */
  license_url: string | null;
}

export const embeddedModelStatus = () =>
  safeInvoke<EmbeddedModelStatus>('embedded_model_status', undefined, {
    path: '',
    present: false,
    size_bytes: null,
    model_id: '',
    license: null,
    license_url: null,
  });

/** Result of a successful `package_embedded_model` call. The UI
 * displays the path back to the user so they can copy it to a
 * fileserver. */
export interface EmbeddedModelPackageResult {
  path: string;
  model_id: string;
  size_bytes: number;
  sha256: string;
  license: string | null;
}

export const packageEmbeddedModel = (
  targetPath: string,
  note?: string,
): Promise<EmbeddedModelPackageResult> =>
  safeInvoke<EmbeddedModelPackageResult>(
    'package_embedded_model',
    { targetPath, note: note ?? null },
    {
      path: '',
      model_id: '',
      size_bytes: 0,
      sha256: '',
      license: null,
    },
  );

/** Manifest metadata returned by `inspect_embedded_model_package`.
 * Mirrors the Rust `PackageManifest` struct. */
export interface EmbeddedModelPackageManifest {
  kind: string;
  format_version: number;
  model_id: string;
  model_filename: string;
  source_repo: string;
  source_filename: string;
  sha256: string;
  size_bytes: number;
  license: string | null;
  license_url: string | null;
  exporter_version: string;
  exported_at: string;
  note: string | null;
}

export const inspectEmbeddedModelPackage = (packagePath: string) =>
  safeInvoke<EmbeddedModelPackageManifest | null>(
    'inspect_embedded_model_package',
    { packagePath },
    null,
  );

/** Apply a package — verifies sha256 and atomically swaps in the
 * GGUF on success. Returns the on-disk path of the imported model. */
export const applyEmbeddedModelPackage = (packagePath: string) =>
  safeInvoke<string>('apply_embedded_model_package', { packagePath });

/** Open a "save as" dialog and package the current model. Returns
 * `null` when the user cancels the dialog. */
export const pickAndPackageEmbeddedModel = (note?: string) =>
  safeInvoke<EmbeddedModelPackageResult | null>(
    'pick_and_package_embedded_model',
    { note: note ?? null },
    null,
  );

/** Open an "open file" dialog and apply the chosen package.
 * Returns the on-disk path of the imported GGUF, or `null` when
 * the user cancels. */
export const pickAndApplyEmbeddedModelPackage = () =>
  safeInvoke<string | null>('pick_and_apply_embedded_model_package', undefined, null);

/** A single embedded-model tier the picker can show. Mirrors the
 * Rust `ModelTier` struct in `model_registry.rs`. */
export interface ModelTier {
  id: string;
  display_name: string;
  description: string;
  repo: string;
  filename: string;
  size_mb: number;
  min_ram_gb: number;
  license: string;
  license_url: string;
  class: 'compact' | 'standard' | 'quality';
}

/** Snapshot of (current tier, recommended tier, host capabilities)
 * used by the settings dropdown and the onboarding modal to render
 * the "現在 / 推奨 / RAM" trio in one round-trip. */
export interface ModelTierStatus {
  current: ModelTier | null;
  recommended: ModelTier;
  recommended_reason: string;
  total_ram_gb: number;
  cpu_cores: number;
  /** True when the currently-selected tier requires more RAM than
   * the host has. The settings UI renders a prominent warning + a
   * one-click "推奨ティアへ切替" action when this fires. */
  current_underpowered: boolean;
}

export const listModelTiers = () =>
  safeInvoke<ModelTier[]>('list_model_tiers', undefined, []);

export const modelTierStatus = () =>
  safeInvoke<ModelTierStatus | null>('model_tier_status', undefined, null);

/** Switch the embedded provider to a different tier. Saves the
 * `repo`/`filename_remote` overrides into user.toml and rebuilds
 * the provider map; does NOT trigger a download — the caller drives
 * that via `downloadEmbeddedModel`. */
export const setModelTier = (tierId: string) =>
  safeInvoke<void>('set_model_tier', { tierId });

/* ---------------- translation routing tier ---------------- */

/** The three user-facing routing tiers exposed by the matrix.
 * Snake-case to match the Rust `UserTier` serde rename. */
export type UserTier = 'compact' | 'standard' | 'quality';

/** Hardware bucket the router uses. CPU mode picks single-step
 * routes; GPU mode picks Pivot for ja↔X / X→Y pairs. Derived from
 * the build's GPU feature flag. */
export type Hardware = 'cpu' | 'gpu';

/** Snapshot of the saved routing tier + detected hardware. When
 * `current === null` the user has not opted into routing yet and
 * the legacy single-tier `embedded.llm` path is still in effect. */
export interface UserTierStatus {
  current: UserTier | null;
  hardware: Hardware;
}

/** One row of the routing-preview matrix surfaced by the settings
 * UI. `mode` mirrors the Rust `RoutingPreview` enum. */
export type RoutingPreview =
  | { mode: 'direct'; tier_id: string; display_name: string }
  | {
      mode: 'pivot';
      stage1_tier_id: string;
      stage1_display_name: string;
      stage2_tier_id: string;
      stage2_display_name: string;
    }
  | { mode: 'unsupported' };

export const getUserTier = () =>
  safeInvoke<UserTierStatus | null>('get_user_tier', undefined, null);

export const setUserTier = (tierId: UserTier) =>
  safeInvoke<void>('set_user_tier', { tierId });

export const clearUserTier = () => safeInvoke<void>('clear_user_tier', undefined);

/** Resolve which model(s) the router would dispatch for one
 * `(source, target)` pair. `tier_override` lets the picker preview
 * a different tier without saving it. */
export const previewRouting = (
  source: string,
  target: string,
  tierOverride?: UserTier,
) =>
  safeInvoke<RoutingPreview | null>(
    'preview_routing',
    { source, target, tierOverride: tierOverride ?? null },
    null,
  );

/** Per-model readiness for one entry in the user's active routing
 * tier. Standard / Quality require 2 GGUFs; the settings UI
 * renders one row per entry so users can see which are
 * downloaded and which need to be fetched. */
export interface RoutedModelStatus {
  tier_id: string;
  display_name: string;
  size_mb: number;
  present: boolean;
  license: string;
  repo: string;
  primary_filename: string;
}

/** Aggregate model status for the routing tier the user has
 * selected. Returns `[]` (rather than null) when the IPC succeeds
 * with an empty list (e.g. an unrecognised tier id); the caller
 * just renders nothing in that case. */
export const routedModelStatus = () =>
  safeInvoke<RoutedModelStatus[]>('routed_model_status', undefined, []);

/** Trigger download of a specific routing-tier model by id.
 * Returns the resolved on-disk path on success. Progress arrives
 * via the existing `moonlens://embedded-model-progress` event
 * channel — subscribe with `onEmbeddedModelProgress`. */
export const downloadRoutedModel = (tierId: string, force = false) =>
  safeInvoke<string | null>(
    'download_routed_model',
    { tierId, force },
    null,
  );

/** Per-sample result from the embedded-LLM benchmark.
 *
 * Two throughput numbers are reported per sample:
 *   - `tokens_per_second`: wall-clock rate. Includes context
 *     setup + prompt eval. Reflects the per-line latency a Lens
 *     user actually feels.
 *   - `decode_tokens_per_second`: steady-state generation rate
 *     after the first token is out. The model's intrinsic
 *     throughput; this is the apples-to-apples metric vs
 *     `llama-bench tg` upstream and what the verdict compares
 *     against the published baseline.
 */
export interface BenchSample {
  index: number;
  total_ms: number;
  prompt_eval_ms: number;
  decode_ms: number;
  output_tokens: number;
  tokens_per_second: number;
  decode_tokens_per_second: number;
  translated_preview: string;
}

/** Aggregate result returned by the bench IPC. */
export interface BenchOutcome {
  tier_display_name: string;
  model_path: string;
  samples: BenchSample[];
  p50_tokens_per_second: number;
  p95_tokens_per_second: number;
  mean_tokens_per_second: number;
  p50_decode_tokens_per_second: number;
  p95_decode_tokens_per_second: number;
  mean_decode_tokens_per_second: number;
  p50_latency_ms: number;
  p95_latency_ms: number;
  host_total_ram_gb: number;
  host_cpu_cores: number;
  total_run_ms: number;
}

/** Run an embedded-LLM benchmark using fixed JA/EN translation
 * fixtures. Returns per-sample latency + aggregate p50/p95 so the
 * settings UI can show the user honest perf numbers from their
 * own machine. */
export const benchEmbeddedTranslate = (samples?: number) =>
  safeInvoke<BenchOutcome | null>(
    'bench_embedded_translate',
    samples != null ? { samples } : undefined,
    null,
  );

/** Live progress event fired during `bench_embedded_translate`. The
 * settings UI subscribes via `onBenchProgress` so the user sees
 * "サンプル X/N 実行中… YY tokens" instead of a silent spinner. */
export type BenchProgressEvent =
  | {
      phase: 'started';
      total_samples: number;
      tier_display_name: string;
    }
  | {
      phase: 'sample_started';
      index: number;
      total: number;
      fixture_text: string;
    }
  | {
      phase: 'sample_progress';
      index: number;
      total: number;
      tokens_so_far: number;
      elapsed_ms: number;
    }
  | {
      phase: 'sample_completed';
      sample: BenchSample;
    }
  | { phase: 'completed' };

export const onBenchProgress = (
  handler: (e: BenchProgressEvent) => void,
): Promise<UnlistenFn> =>
  listen<BenchProgressEvent>('moonlens://embedded-bench-progress', (e) =>
    handler(e.payload),
  );

/** Trigger the embedded LLM download. Idempotent: returns immediately
 * with the resolved path when the file already exists, unless
 * `force=true` is supplied. Progress events arrive via
 * `onEmbeddedModelProgress`. */
export const downloadEmbeddedModel = (force = false) =>
  safeInvoke<string>('download_embedded_model', { force });

export const onEmbeddedModelProgress = (
  handler: (p: ModelProgress) => void,
): Promise<UnlistenFn> =>
  listen<ModelProgress>('moonlens://embedded-model-progress', (e) => handler(e.payload));

/** Lifecycle event emitted around `mistralrs::Model::build`. The
 * layout shell uses these to render a "ローカル LLM をロード中…"
 * badge during the multi-second cold-start so the user is not
 * staring at a silent translation request. */
export type EmbeddedModelLoadingEvent =
  | {
      phase: 'started';
      provider_id: string;
      model_path: string;
    }
  | {
      phase: 'finished';
      provider_id: string;
      model_path: string;
      success: boolean;
      error: string | null;
    };

export const onEmbeddedModelLoading = (
  handler: (e: EmbeddedModelLoadingEvent) => void,
): Promise<UnlistenFn> =>
  listen<EmbeddedModelLoadingEvent>('moonlens://embedded-model-loading', (e) =>
    handler(e.payload),
  );
export const showMain = () => safeInvoke<void>('show_main');
export const restartApp = () => safeInvoke<void>('restart_app');
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

export const captureFile = (path: string) =>
  safeInvoke<CaptureFlowResult>('capture_file', { path });

export const pickAndCaptureFile = () =>
  safeInvoke<CaptureFlowResult | null>('pick_and_capture_file');

export const startRegionSelection = () =>
  safeInvoke<void>('start_region_selection');

export interface WindowDescriptor {
  id: number;
  title: string;
  app_name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  is_minimized: boolean;
}

export const listWindows = () =>
  safeInvoke<WindowDescriptor[]>('list_windows', undefined, []);

export const captureWindowById = (id: number) =>
  safeInvoke<CaptureFlowResult>('capture_window_by_id', { id });

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

export interface AsrEndpointReport {
  reachable: boolean;
  status: number | null;
  url: string;
  provider: string;
  message: string;
}

export const testAsrEndpoint = () =>
  safeInvoke<AsrEndpointReport>('test_asr_endpoint');
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
  /** Whether the first-run embedded LLM setup modal should appear.
   * True means: chain references `embedded.llm`, the GGUF is not
   * on disk yet, and the user has not dismissed the prompt. */
  needs_embedded_model_setup: boolean;
  /** Resolved on-disk path the GGUF will land at — shown in the
   * modal so the user knows what's about to happen. */
  embedded_model_path: string;
  /** Whether the user's translate chain references any
   * `embedded.*` provider. False means we should NOT bother the
   * user about downloading a model they will not use. */
  embedded_in_translate_chain: boolean;
}

export const getOnboardingStatus = () =>
  safeInvoke<OnboardingStatus>('get_onboarding_status', undefined, {
    platform: 'other',
    needs_macos_permissions_review: false,
    macos_permissions: defaultPermissions,
    needs_embedded_model_setup: false,
    embedded_model_path: '',
    embedded_in_translate_chain: false,
  });

export const markMacosPermissionsReviewed = () =>
  safeInvoke<void>('mark_macos_permissions_reviewed');

/** Mark the embedded-model first-run modal as seen. Called whether
 * the user accepts the download or dismisses with "後で". */
export const markEmbeddedModelSetupSeen = () =>
  safeInvoke<void>('mark_embedded_model_setup_seen');

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
  /** CSS `font-weight` (400 / 600 / 700) estimated from the source
   *  glyph's stroke thickness. Older backends may omit this; the
   *  frontend falls back to 400. */
  font_weight?: number;
  /** Source-derived font size in image pixels — median height of
   *  the OCR member bboxes assigned to this card. No cap-height
   *  reduction: translation glyphs render at the same visible
   *  height as the source. */
  font_size_px?: number;
  /** Source-derived CSS `line-height` ratio (baseline-to-baseline
   *  distance ÷ glyph height). Frontend uses this directly for the
   *  translation's line-height so wrapped lines occupy the same
   *  vertical rhythm as the source. Fallback 1.25 when only a
   *  single source line is present. */
  line_height?: number;
  /** Number of source OCR lines this card covers. Frontend uses
   *  this as the target line count: if the wrapped translation
   *  would exceed it, horizontal compression (`transform:
   *  scaleX(<1)`) shrinks the text to fit without resizing or
   *  adding rows — strict source fidelity for size and stroke. */
  source_line_count?: number;
  /** Detected horizontal alignment of the SOURCE text: `"left"`,
   *  `"center"`, or `"right"`. Frontend maps this to CSS
   *  `text-align` so a centred heading or right-aligned status
   *  strip stays aligned the same way after translation. */
  text_align?: 'left' | 'center' | 'right';
  /** Tight rectangle the bg_patch covers, in image pixels. The
   *  card's `bbox` is widened toward the layout block so
   *  text-align has room to operate, but the inpaint bg_patch
   *  needs to stay anchored to the source text region.
   *  Frontend places the patch image at `(inpaint_bbox - bbox)`
   *  offset, sized to inpaint_bbox dimensions. */
  inpaint_bbox?: [number, number, number, number];
  /** Base64-encoded PNG of the inpainted source region at `bbox`
   *  (tiled) dimensions. When present, the frontend draws it as the
   *  card background instead of the flat `bg_color` / `bg_gradient`,
   *  so the card blends into the surrounding image. */
  bg_patch?: string;
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
