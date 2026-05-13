<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    Search,
    RotateCcw,
    Save,
    Settings as SettingsIcon,
    Cpu,
    Layers,
    Mic,
    Keyboard,
    Shield,
    Info,
    Database,
    Gauge,
    Puzzle,
    Clipboard,
    Plus,
    Trash2,
    ArrowUp,
    ArrowDown,
    Folder,
    Globe,
    HardDrive,
    Cloud,
    Server,
    Sparkles
  } from 'lucide-svelte';
  import {
    benchEmbeddedTranslate,
    downloadEmbeddedModel,
    embeddedModelStatus,
    getSettingsSchema,
    modelTierStatus,
    onBenchProgress,
    onEmbeddedModelProgress,
    pickAndApplyEmbeddedModelPackage,
    pickAndPackageEmbeddedModel,
    restartApp,
    getUserTier,
    setUserTier,
    previewRouting,
    routedModelStatus,
    downloadRoutedModel,
    type BenchOutcome,
    type BenchProgressEvent,
    type BenchSample,
    type EmbeddedModelStatus,
    type ModelProgress,
    type ModelTierStatus,
    type UserTier,
    type UserTierStatus,
    type RoutingPreview,
    type RoutedModelStatus,
  } from '$lib/ipc';
  import {
    currentSettings,
    ensureSettingsLoaded,
    readSetting,
    updateSetting,
    replaceSettings,
    saveNow as saveSettingsNow,
    saveStateStore,
    saveErrorMessage,
    settingsLoading,
    restartNeeded,
    resetToBootSnapshot
  } from '$lib/stores/settingsStore';
  import EngineCard, { type EngineKind, type EngineStatusBadge } from '$lib/components/EngineCard.svelte';
  import SchemaForm from '$lib/components/SchemaForm.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import SaveIndicator from '$lib/components/SaveIndicator.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import MoonLensMark from '$lib/components/MoonLensMark.svelte';
  import Toggle from '$lib/components/Toggle.svelte';
  import Segmented from '$lib/components/Segmented.svelte';
  import HotkeyInput from '$lib/components/HotkeyInput.svelte';
  import NumberSlider from '$lib/components/NumberSlider.svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { RotateCw, AlertCircle } from 'lucide-svelte';

  /** ----------------------------------------------------------------
   * Sectioned navigation
   * ---------------------------------------------------------------- */
  type Section =
    | 'appearance'
    | 'hotkeys'
    | 'subtitle'
    | 'overlay'
    | 'clipboard'
    | 'storage'
    | 'privacy'
    | 'providers'
    | 'plugins'
    | 'performance'
    | 'about'
    | 'advanced';

  // Group sections under top-level categories so the left rail reads
  // like a real preferences pane (macOS-style) instead of one flat
  // list of 12 entries.
  const navTree: {
    group: string;
    items: { id: Section; label: string; icon: typeof SettingsIcon }[];
  }[] = [
    {
      group: '表示',
      items: [
        { id: 'appearance', label: '外観', icon: SettingsIcon },
        { id: 'hotkeys', label: 'ホットキー', icon: Keyboard }
      ]
    },
    {
      group: '機能',
      items: [
        { id: 'subtitle', label: '音声字幕', icon: Mic },
        { id: 'overlay', label: 'オーバーレイ・Lens', icon: Layers },
        { id: 'clipboard', label: 'クリップボード', icon: Clipboard }
      ]
    },
    {
      group: 'データ',
      items: [
        { id: 'storage', label: '履歴・ストレージ', icon: Database },
        { id: 'privacy', label: 'プライバシー', icon: Shield }
      ]
    },
    {
      group: 'システム',
      items: [
        { id: 'providers', label: 'プロバイダ', icon: Cpu },
        { id: 'plugins', label: 'プラグイン', icon: Puzzle },
        { id: 'performance', label: 'パフォーマンス', icon: Gauge }
      ]
    },
    {
      group: 'その他',
      items: [
        { id: 'about', label: 'About', icon: Info },
        { id: 'advanced', label: '詳細 (生スキーマ)', icon: HardDrive }
      ]
    }
  ];

  /** ----------------------------------------------------------------
   * State — wired through the global settings store so changes show
   *         up in the layout (theme, accent, density) immediately.
   * ---------------------------------------------------------------- */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let schema = $state<any>(null);
  let query = $state('');
  let confirmReset = $state(false);
  let section = $state<Section>('appearance');
  let appVersion = $state<string | null>(null);
  let restarting = $state(false);
  let restartError = $state<string | null>(null);

  type ChainKey =
    | 'translate'
    | 'ocr'
    | 'asr'
    | 'explain'
    | 'summarize'
    | 'reverse_image_search'
    | 'qr_handler'
    | 'intent_classifier';
  let providerTab = $state<ChainKey>('translate');
  let newChainEntries = $state<Record<string, string>>({});

  /**
   * The advanced (raw schema) view uses SchemaForm which still
   * binds two-way via `bind:value`. We mirror the store into a
   * local $state, then push the mirror back to the store when
   * SchemaForm signals a change. This keeps the curated GUI on
   * the same source of truth as the JSON-form fallback.
   */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let advancedMirror = $state<any>(null);
  $effect(() => {
    advancedMirror = $currentSettings ? structuredClone($state.snapshot($currentSettings)) : null;
  });
  function onAdvancedChange() {
    if (!advancedMirror) return;
    // SchemaForm has just mutated `advancedMirror`. Push the whole
    // tree to the store, which schedules a debounced save.
    replaceSettings(structuredClone($state.snapshot(advancedMirror)), 250);
  }

  // Reactive proxies into the store so $-syntax reads trigger updates.
  const settings = $derived($currentSettings);
  const saveState = $derived($saveStateStore);
  const errorMessage = $derived($saveErrorMessage);
  const loading = $derived($settingsLoading);
  const showRestartBanner = $derived($restartNeeded);

  /** Status snapshot of the embedded LLM model file. Refreshed on
   * mount and after every successful download so the providers tab
   * reflects the on-disk reality without needing a settings reload. */
  let embeddedStatus = $state<EmbeddedModelStatus>({
    path: '',
    present: false,
    size_bytes: null,
    model_id: '',
    license: null,
    license_url: null,
  });
  let embeddedProgress = $state<ModelProgress | null>(null);
  let embeddedDownloading = $state(false);
  let embeddedError = $state<string | null>(null);
  let unlistenEmbeddedProgress: (() => void) | null = null;
  let embeddedPackaging = $state(false);
  let embeddedImporting = $state(false);
  let embeddedPackageMessage = $state<string | null>(null);
  // `tierStatus` is still consulted by:
  //   - the "推奨" badge on the embedded.llm engine card
  //     (compares the saved tier_id to the HW recommendation),
  //   - the benchmark result panel (`expectedTokensPerSecondFor`),
  //   - `tierDescriptionLine()` for the recommendation rationale.
  // The standalone モデルティア dropdown that previously drove
  // `selectedTierId` / `switchingTier` / `handleTierChange` /
  // `formatTierSize` was removed when the 翻訳ティア picker took
  // over, so those state vars + the related `modelTiers` /
  // `setModelTier` / `listModelTiers` imports were dropped.
  let tierStatus = $state<ModelTierStatus | null>(null);
  // Routing tier state — separate from `selectedTierId` because the
  // user picks ONE of three Tier names (Compact / Standard / Quality)
  // here, and the routing matrix decides which underlying model
  // (or model pair) handles each translate call.
  let userTierStatus = $state<UserTierStatus | null>(null);
  let switchingUserTier = $state(false);
  // Per-model presence for the active routing tier. Standard /
  // Quality need 2 GGUFs (a JA bridge + a multilingual model);
  // the legacy `embeddedStatus.present` flag only inspects the
  // single `embedded.llm` config block and misreports "ready"
  // when only one of the routed models is on disk.
  let routedModels = $state<RoutedModelStatus[]>([]);
  // Three common pairs surfaced as a "what does each route to?"
  // preview matrix beneath the picker. Lets the user verify the
  // tier picks the model they expect before committing.
  let routingPreviews = $state<Array<{
    label: string;
    src: string;
    tgt: string;
    preview: RoutingPreview | null;
  }>>([
    { label: '英→日', src: 'en', tgt: 'ja', preview: null },
    { label: '中→日', src: 'zh-CN', tgt: 'ja', preview: null },
    { label: '日→韓', src: 'ja', tgt: 'ko', preview: null },
    { label: '仏→独', src: 'fr', tgt: 'de', preview: null },
  ]);
  let benchRunning = $state(false);
  let benchResult = $state<BenchOutcome | null>(null);
  let benchError = $state<string | null>(null);
  /** Live progress state, populated by onBenchProgress. Cleared at
   * the start of each run so a re-run does not show stale numbers. */
  let benchProgress = $state<{
    totalSamples: number;
    currentIndex: number;
    tokensSoFar: number;
    elapsedMs: number;
    fixtureText: string;
    completedSamples: BenchSample[];
  } | null>(null);
  let unlistenBench: (() => void) | null = null;

  function handleBenchEvent(e: BenchProgressEvent) {
    switch (e.phase) {
      case 'started':
        benchProgress = {
          totalSamples: e.total_samples,
          currentIndex: 0,
          tokensSoFar: 0,
          elapsedMs: 0,
          fixtureText: '',
          completedSamples: [],
        };
        break;
      case 'sample_started':
        if (benchProgress) {
          benchProgress = {
            ...benchProgress,
            currentIndex: e.index,
            tokensSoFar: 0,
            elapsedMs: 0,
            fixtureText: e.fixture_text,
          };
        }
        break;
      case 'sample_progress':
        if (benchProgress) {
          benchProgress = {
            ...benchProgress,
            tokensSoFar: e.tokens_so_far,
            elapsedMs: e.elapsed_ms,
          };
        }
        break;
      case 'sample_completed':
        if (benchProgress) {
          benchProgress = {
            ...benchProgress,
            completedSamples: [...benchProgress.completedSamples, e.sample],
          };
        }
        break;
      case 'completed':
        // Final result arrives via the IPC return value, so just
        // mark the live progress as done.
        break;
    }
  }

  async function runEmbeddedBench() {
    if (benchRunning || !embeddedStatus.present) return;
    benchRunning = true;
    benchError = null;
    benchResult = null;
    benchProgress = null;
    // Subscribe to progress BEFORE invoking so we don't miss the
    // "started" event.
    if (!unlistenBench) {
      try {
        unlistenBench = await onBenchProgress(handleBenchEvent);
      } catch (e) {
        console.warn('bench progress listener failed:', e);
      }
    }
    try {
      benchResult = await benchEmbeddedTranslate(5);
    } catch (e) {
      benchError = e instanceof Error ? e.message : String(e);
    } finally {
      benchRunning = false;
    }
  }

  /** Hand-tuned per-tier baselines derived from public llama.cpp
   *  benchmarks on a "modern office laptop" CPU class (Intel
   *  Core i5-1240P / Ryzen 5 5500U or similar, 4-8 P-cores, no GPU,
   *  Q4_K_M quant). These are realistic targets — if the user's
   *  bench is below these, performance is genuinely off and points
   *  to a hardware/runtime issue, not the model.
   *
   *  Bucketing by approximate parameter count rather than exact
   *  tier id keeps the baseline list short (the same i5-1240P that
   *  hits 50 tok/s on Qwen 2.5 0.5B will hit roughly the same on
   *  Qwen 3 0.6B and Gemma 4 e2b). Family-specific differences
   *  (sparse-activation Gemma e2b/e4b activates fewer params per
   *  token, so its decode is closer to its effective param count
   *  than its total) are noted in the lookup. */
  function expectedTokensPerSecondFor(tierId: string | undefined): number | null {
    if (!tierId) return null;
    const id = tierId.toLowerCase();
    // Sub-1B: Qwen 2.5 0.5B, Qwen 3 0.6B
    if (id.includes('0.5b') || id.includes('0.6b')) return 50;
    // ~1.5-1.7B: Qwen 2.5 1.5B, Qwen 3 1.7B, SmolLM 2 1.7B
    if (id.includes('1.5b') || id.includes('1.7b')) return 25;
    // Gemma 4 nano: effective 2B / 4B activation, decode is close
    // to dense ~1B / ~2B respectively, NOT to total param count.
    if (id.includes('e2b')) return 30;
    if (id.includes('e4b')) return 15;
    // Mid-tier dense: Qwen 3 4B
    if (id.includes('4b')) return 10;
    // Quality tier: Qwen 2.5 7B
    if (id.includes('7b')) return 5;
    return null;
  }

  async function refreshTierState() {
    // Only fetches the tier STATUS (current + recommended class)
    // now; the full TIERS list isn't surfaced anywhere in the UI
    // after the model-tier dropdown removal. The bench result
    // panel and the recommended badge consult `tierStatus`
    // directly.
    try {
      tierStatus = await modelTierStatus();
    } catch (e) {
      console.warn('tier status load failed:', e);
    }
  }

  async function refreshUserTierState() {
    try {
      userTierStatus = await getUserTier();
      await refreshRoutingPreviews(userTierStatus?.current ?? 'standard');
      routedModels = await routedModelStatus();
    } catch (e) {
      console.warn('user tier status load failed:', e);
    }
  }

  function formatModelSize(mb: number): string {
    if (mb < 1024) return `${mb} MB`;
    return `${(mb / 1024).toFixed(1)} GB`;
  }

  /** Per-row in-flight flag so the user cannot double-tap the
   * same row while a download is running. Keyed by tier_id. */
  let downloadingTiers = $state<Record<string, boolean>>({});

  async function handleDownloadRouted(tierId: string) {
    if (downloadingTiers[tierId]) return;
    downloadingTiers = { ...downloadingTiers, [tierId]: true };
    try {
      await downloadRoutedModel(tierId, false);
      // Refresh per-model status so the row flips to "準備済み"
      // without requiring a full page reload.
      await refreshUserTierState();
    } catch (e) {
      console.warn(`download_routed_model(${tierId}) failed:`, e);
    } finally {
      downloadingTiers = { ...downloadingTiers, [tierId]: false };
    }
  }

  async function refreshRoutingPreviews(tier: UserTier) {
    try {
      const next = await Promise.all(
        routingPreviews.map(async (row) => ({
          ...row,
          preview: await previewRouting(row.src, row.tgt, tier),
        })),
      );
      routingPreviews = next;
    } catch (e) {
      console.warn('routing preview load failed:', e);
    }
  }

  async function handleUserTierChange(nextTier: UserTier) {
    if (switchingUserTier) return;
    switchingUserTier = true;
    try {
      await setUserTier(nextTier);
      await refreshUserTierState();
    } catch (e) {
      console.warn('user tier switch failed:', e);
    } finally {
      switchingUserTier = false;
    }
  }

  function renderRoutingPreview(p: RoutingPreview | null): string {
    if (!p) return '—';
    if (p.mode === 'direct') return `直接: ${p.display_name}`;
    if (p.mode === 'pivot')
      return `Pivot: ${p.stage1_display_name} → ${p.stage2_display_name}`;
    return '✗ 未対応';
  }

  async function packageCurrentModel() {
    if (embeddedPackaging || !embeddedStatus.present) return;
    embeddedPackaging = true;
    embeddedError = null;
    embeddedPackageMessage = null;
    try {
      const result = await pickAndPackageEmbeddedModel();
      if (result && result.path) {
        const sizeMb = (result.size_bytes / (1024 * 1024)).toFixed(0);
        embeddedPackageMessage =
          `パッケージを保存しました: ${result.path} ` +
          `(${sizeMb} MB, ${result.license ?? 'ライセンス未指定'})`;
      }
    } catch (e) {
      embeddedError = e instanceof Error ? e.message : String(e);
    } finally {
      embeddedPackaging = false;
    }
  }

  async function importPackage() {
    if (embeddedImporting) return;
    embeddedImporting = true;
    embeddedError = null;
    embeddedPackageMessage = null;
    try {
      const imported = await pickAndApplyEmbeddedModelPackage();
      if (imported) {
        embeddedPackageMessage = `パッケージを取り込みました: ${imported}`;
        await refreshEmbeddedStatus();
      }
    } catch (e) {
      embeddedError = e instanceof Error ? e.message : String(e);
    } finally {
      embeddedImporting = false;
    }
  }

  async function refreshEmbeddedStatus() {
    try {
      embeddedStatus = await embeddedModelStatus();
    } catch (e) {
      console.warn('embedded model status failed:', e);
    }
  }

  async function startEmbeddedDownload(force = false) {
    if (embeddedDownloading) return;
    embeddedDownloading = true;
    embeddedError = null;
    embeddedProgress = null;
    try {
      await downloadEmbeddedModel(force);
      await refreshEmbeddedStatus();
    } catch (e) {
      embeddedError = e instanceof Error ? e.message : String(e);
    } finally {
      embeddedDownloading = false;
    }
  }

  function formatBytes(bytes: number | null | undefined): string {
    if (bytes == null) return '不明';
    const mb = bytes / (1024 * 1024);
    if (mb < 1024) return `${mb.toFixed(1)} MB`;
    return `${(mb / 1024).toFixed(2)} GB`;
  }

  onMount(async () => {
    void ensureSettingsLoaded();
    try {
      schema = await getSettingsSchema();
    } catch (e) {
      // Schema is only consumed by the "詳細" fallback view; failing
      // to load it must NOT block the main GUI.
      console.warn('schema load failed:', e);
    }
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = null;
    }
    void refreshEmbeddedStatus();
    void refreshTierState();
    void refreshUserTierState();
    try {
      const off = await onEmbeddedModelProgress((p) => {
        embeddedProgress = p;
      });
      unlistenEmbeddedProgress = off;
    } catch (e) {
      console.warn('embedded progress listener failed:', e);
    }
  });

  onDestroy(() => {
    /* store owns the debounce timer */
    if (unlistenEmbeddedProgress) {
      unlistenEmbeddedProgress();
      unlistenEmbeddedProgress = null;
    }
    if (unlistenBench) {
      unlistenBench();
      unlistenBench = null;
    }
  });

  // The curated controls bind through these helpers; they delegate to
  // the global store so changes propagate everywhere (layout shell,
  // sidebar, audio page) in real time.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function read<T>(path: string, fallback: T): T {
    // Subscribe via the derived `settings` so this re-runs reactively.
    void settings;
    return readSetting<T>(path, fallback);
  }
  function update(path: string, value: unknown) {
    updateSetting(path, value);
  }
  async function saveNow() {
    await saveSettingsNow();
  }

  async function resetAll() {
    confirmReset = false;
    resetToBootSnapshot();
  }

  async function performRestart() {
    if (restarting) return;
    restarting = true;
    restartError = null;
    try {
      // Flush pending changes before the process tears down.
      await saveSettingsNow();
      await restartApp();
    } catch (e) {
      restartError = String(e);
      restarting = false;
    }
  }

  /** ----------------------------------------------------------------
   * Appearance
   * ---------------------------------------------------------------- */
  const accentSwatches = [
    { value: '#A8C5DA', label: 'ムーンライト' },
    { value: '#C4B5A0', label: 'サンド' },
    { value: '#7DD3C0', label: 'シーフォーム' },
    { value: '#B8A5D9', label: 'ラベンダー' },
    { value: '#E0B66E', label: 'アンバー' }
  ];

  /** ----------------------------------------------------------------
   * Hotkeys
   * ---------------------------------------------------------------- */
  const hotkeyRows = [
    { key: 'capture_region', label: '範囲キャプチャ' },
    { key: 'capture_window', label: 'ウィンドウキャプチャ' },
    { key: 'toggle_pin_overlay', label: 'オーバーレイをピン留め' },
    { key: 'toggle_subtitle', label: '音声字幕の開始/停止' },
    { key: 'open_command_palette', label: 'コマンドパレット' },
    { key: 'open_history', label: '履歴を開く' },
    { key: 'reverse_image_search', label: '画像で検索' },
    { key: 'stop_all', label: '全停止 (パニックボタン)' }
  ];

  /** ----------------------------------------------------------------
   * Providers — chain helpers + provider taxonomy
   * ---------------------------------------------------------------- */
  type ProviderKind = 'local' | 'external' | 'cloud';
  interface ProviderHint {
    kind: ProviderKind;
    label: string;
    description: string;
  }
  // Hints surface each provider id's deployment story so the user
  // can tell at a glance what they're committing to. New providers
  // default to "external" until classified explicitly.
  const providerHints: Record<string, ProviderHint> = {
    // Local / built-in
    windows: {
      kind: 'local',
      label: 'Windows.Media.Ocr',
      description: 'Windows 内蔵 OCR (日中韓対応)'
    },
    ocrs: {
      kind: 'local',
      label: 'ocrs',
      description: '組み込み Rust OCR (Latin 系に強い)'
    },
    rqrr: {
      kind: 'local',
      label: 'rqrr',
      description: '組み込み QR デコーダ'
    },
    'builtin.heuristic': {
      kind: 'local',
      label: 'Heuristic',
      description: '組み込みヒューリスティック分類'
    },
    'embedded.llm': {
      kind: 'local',
      label: '組み込みローカル LLM',
      description: 'MoonLens に同梱されるローカル LLM (Qwen 2.5, Apache 2.0)。初回起動時にモデルを自動ダウンロード。'
    },
    // External servers (require user-managed daemon)
    'ollama.gemma4': {
      kind: 'external',
      label: 'Ollama (Gemma 4)',
      description: 'http://127.0.0.1:11434 で稼働するローカル LLM サーバ (オプション)'
    },
    'whisper_cpp.large_v3_turbo': {
      kind: 'external',
      label: 'whisper.cpp',
      description: 'http://127.0.0.1:9002 で稼働する音声認識サーバ'
    },
    paddle: {
      kind: 'external',
      label: 'PaddleOCR',
      description: '別途起動した PaddleOCR サーバ'
    },
    // Cloud APIs
    deepl: {
      kind: 'cloud',
      label: 'DeepL',
      description: 'クラウド翻訳 API (DEEPL_API_KEY)'
    },
    gemini: {
      kind: 'cloud',
      label: 'Google Gemini',
      description: 'クラウド LLM API (GEMINI_API_KEY)'
    },
    'browser.google_images': {
      kind: 'cloud',
      label: 'Google 画像検索',
      description: 'ブラウザで開く逆引き検索'
    },
    'browser.bing': {
      kind: 'cloud',
      label: 'Bing 画像検索',
      description: 'ブラウザで開く逆引き検索'
    },
    'browser.yandex': {
      kind: 'cloud',
      label: 'Yandex 画像検索',
      description: 'ブラウザで開く逆引き検索'
    }
  };

  function hintFor(id: string): ProviderHint {
    if (providerHints[id]) return providerHints[id];
    if (id.startsWith('plugin.')) {
      return {
        kind: 'local',
        label: id,
        description: 'プラグイン提供のプロバイダ'
      };
    }
    return { kind: 'external', label: id, description: '未分類のプロバイダ' };
  }

  const chainCatalog: {
    id: ChainKey;
    label: string;
    description: string;
    suggestions: string[];
  }[] = [
    {
      id: 'translate',
      label: '翻訳',
      description: 'OCR後・字幕の本文翻訳',
      suggestions: ['embedded.llm', 'ollama.gemma4', 'deepl', 'gemini']
    },
    {
      id: 'ocr',
      label: 'OCR',
      description: '画像から文字を認識',
      suggestions: ['windows', 'ocrs', 'paddle']
    },
    {
      id: 'asr',
      label: '音声認識',
      description: '音声を文字起こし',
      suggestions: ['whisper_cpp.large_v3_turbo']
    },
    {
      id: 'explain',
      label: '説明',
      description: '画像や文章を要約・説明',
      suggestions: ['embedded.llm', 'ollama.gemma4', 'gemini']
    },
    {
      id: 'summarize',
      label: '要約',
      description: '長文を短く要約',
      suggestions: ['embedded.llm', 'ollama.gemma4', 'gemini']
    },
    {
      id: 'reverse_image_search',
      label: '画像検索',
      description: '逆引き画像検索のソース',
      suggestions: ['browser.google_images', 'browser.bing', 'browser.yandex']
    },
    {
      id: 'qr_handler',
      label: 'QRコード',
      description: 'QRコードのデコーダ',
      suggestions: ['rqrr']
    },
    {
      id: 'intent_classifier',
      label: '意図分類',
      description: 'キャプチャの種類を自動判別',
      suggestions: ['builtin.heuristic']
    }
  ];

  // Catalog integrity self-check. Every provider id referenced by
  // `chainCatalog.suggestions` MUST appear in `providerHints`,
  // otherwise the UI falls back to "未分類のプロバイダ" — which is
  // exactly how the 14.x bug manifested when a sed-based rename
  // missed single-quoted occurrences and the keys/labels drifted
  // between the two catalogs. Logging on module init means the
  // dev console screams the moment a future PR introduces drift,
  // so the bug never reaches production again.
  {
    const known = new Set(Object.keys(providerHints));
    const referenced = new Set<string>();
    for (const chain of chainCatalog) {
      for (const id of chain.suggestions) referenced.add(id);
    }
    const missing = [...referenced].filter(
      (id) => !known.has(id) && !id.startsWith('plugin.'),
    );
    if (missing.length > 0) {
      // eslint-disable-next-line no-console
      console.error(
        '[settings] provider catalog drift — these ids are recommended but lack hints, ' +
          'and would render as "未分類のプロバイダ":',
        missing,
      );
    }
  }

  function reorderChain(key: ChainKey, fromIndex: number, toIndex: number) {
    const chain: string[] = read(`providers.${key}.chain`, []);
    if (
      toIndex < 0 ||
      toIndex >= chain.length ||
      fromIndex === toIndex ||
      fromIndex < 0 ||
      fromIndex >= chain.length
    ) {
      return;
    }
    const next = chain.slice();
    const [moved] = next.splice(fromIndex, 1);
    next.splice(toIndex, 0, moved);
    update(`providers.${key}.chain`, next);
  }

  function removeChainEntry(key: ChainKey, index: number) {
    const chain: string[] = read(`providers.${key}.chain`, []);
    if (index < 0 || index >= chain.length) return;
    const next = chain.slice();
    next.splice(index, 1);
    update(`providers.${key}.chain`, next);
  }

  function addChainEntry(key: ChainKey, value?: string) {
    const v = (value ?? newChainEntries[key] ?? '').trim();
    if (!v) return;
    const chain: string[] = read(`providers.${key}.chain`, []);
    if (chain.includes(v)) return;
    update(`providers.${key}.chain`, [...chain, v]);
    newChainEntries = { ...newChainEntries, [key]: '' };
  }

  const fallbackOptions = [
    { value: 'timeout', label: 'タイムアウト' },
    { value: 'error', label: 'エラー' },
    { value: 'rate_limit', label: 'レート制限' },
    { value: 'offline', label: 'オフライン' },
    { value: 'unsupported_language', label: '未対応言語' },
    { value: 'low_confidence', label: '低信頼度' }
  ];
  function toggleFallback(key: ChainKey, value: string) {
    const current: string[] = read(`providers.${key}.fallback_on`, []);
    const next = current.includes(value)
      ? current.filter((v) => v !== value)
      : [...current, value];
    update(`providers.${key}.fallback_on`, next);
  }

  /** ----------------------------------------------------------------
   * Providers — preset switcher (local / cloud)
   * ---------------------------------------------------------------- */
  function applyLocalOnlyPreset() {
    update('providers.translate.chain', []);
    update('providers.translate.allow_cloud', false);
    update('providers.translate.allow_network', false);
    update('providers.asr.chain', []);
    update('providers.explain.chain', []);
    update('providers.explain.allow_cloud', false);
    update('providers.explain.allow_network', false);
    update('providers.summarize.chain', []);
    update('providers.summarize.allow_cloud', false);
    update('providers.summarize.allow_network', false);
    update('providers.reverse_image_search.chain', []);
    update('privacy.allow_cloud_globally', false);
  }

  function applyOllamaPreset() {
    const ensureFirst = (path: string, id: string) => {
      const chain: string[] = read(`providers.${path}.chain`, []);
      const next = [id, ...chain.filter((c) => c !== id)];
      update(`providers.${path}.chain`, next);
    };
    ensureFirst('translate', 'ollama.gemma4');
    update('providers.translate.allow_network', true);
    ensureFirst('explain', 'ollama.gemma4');
    update('providers.explain.allow_network', true);
    ensureFirst('summarize', 'ollama.gemma4');
    update('providers.summarize.allow_network', true);
  }

  function applyCloudPreset() {
    const ensureLast = (path: string, ids: string[]) => {
      const chain: string[] = read(`providers.${path}.chain`, []);
      const filtered = chain.filter((c) => !ids.includes(c));
      update(`providers.${path}.chain`, [...filtered, ...ids]);
    };
    ensureLast('translate', ['deepl', 'gemini']);
    update('providers.translate.allow_cloud', true);
    update('providers.translate.allow_network', true);
    ensureLast('explain', ['gemini']);
    update('providers.explain.allow_cloud', true);
    update('providers.explain.allow_network', true);
    update('privacy.allow_cloud_globally', true);
  }

  /** Plugins helpers */
  function trustedPublishersText(): string {
    const list = read<string[]>('plugins.trusted_publishers', []);
    return list.join(', ');
  }
  function setTrustedPublishersText(text: string) {
    const arr = text
      .split(/[,\n]/)
      .map((s) => s.trim())
      .filter(Boolean);
    update('plugins.trusted_publishers', arr);
  }

  /** Active sub-section helpers (for UI accents on the nav tree) */
  const activeGroup = $derived.by(() => {
    for (const grp of navTree) {
      if (grp.items.find((i) => i.id === section)) return grp.group;
    }
    return '';
  });

  // For the Providers section, the chain count drives the empty/CTA
  // state. Reactive so adding the first provider hides the empty
  // state immediately.
  const currentChainIds = $derived<string[]>(
    read(`providers.${providerTab}.chain`, [])
  );
  const currentChainCatalog = $derived(
    chainCatalog.find((c) => c.id === providerTab) ?? chainCatalog[0]
  );
  const currentSuggestions = $derived(
    currentChainCatalog.suggestions.filter((s) => !currentChainIds.includes(s))
  );

  /** ----------------------------------------------------------------
   * EngineCard helpers — used by every task-tab to derive (enabled,
   * priority) from the saved chain. The chain is the source of truth;
   * EngineCard is a presentational shell that calls back into these
   * for write-side mutations.
   * ---------------------------------------------------------------- */
  function engineEnabled(task: ChainKey, providerId: string): boolean {
    const chain: string[] = read(`providers.${task}.chain`, []);
    return chain.includes(providerId);
  }
  function enginePriority(task: ChainKey, providerId: string): number | null {
    const chain: string[] = read(`providers.${task}.chain`, []);
    const idx = chain.indexOf(providerId);
    return idx >= 0 ? idx + 1 : null;
  }
  function setEngineEnabled(
    task: ChainKey,
    providerId: string,
    next: boolean,
  ) {
    const chain: string[] = read(`providers.${task}.chain`, []);
    const idx = chain.indexOf(providerId);
    if (next && idx === -1) {
      update(`providers.${task}.chain`, [...chain, providerId]);
    } else if (!next && idx !== -1) {
      const out = chain.slice();
      out.splice(idx, 1);
      update(`providers.${task}.chain`, out);
    }
  }
  function moveEngineUp(task: ChainKey, providerId: string) {
    const chain: string[] = read(`providers.${task}.chain`, []);
    const idx = chain.indexOf(providerId);
    if (idx <= 0) return;
    reorderChain(task, idx, idx - 1);
  }
  function moveEngineDown(task: ChainKey, providerId: string) {
    const chain: string[] = read(`providers.${task}.chain`, []);
    const idx = chain.indexOf(providerId);
    if (idx === -1 || idx >= chain.length - 1) return;
    reorderChain(task, idx, idx + 1);
  }

  /** Per-card detail open/close state, keyed by `${task}:${providerId}`.
   *  Persists across the session so a card stays open while the user
   *  edits config in another sub-section. */
  let detailOpen = $state<Record<string, boolean>>({});
  function isDetailOpen(task: ChainKey, providerId: string): boolean {
    return Boolean(detailOpen[`${task}:${providerId}`]);
  }
  function toggleDetailOpen(task: ChainKey, providerId: string) {
    const k = `${task}:${providerId}`;
    detailOpen = { ...detailOpen, [k]: !detailOpen[k] };
  }

  /** Build an engine status badge for a translate-capable provider. */
  function translateEngineStatus(providerId: string): EngineStatusBadge | null {
    if (providerId === 'embedded.llm' || providerId.startsWith('embedded.')) {
      if (embeddedDownloading) {
        return { label: 'ダウンロード中…', tone: 'pending' };
      }
      if (embeddedStatus.present) {
        return {
          label: `準備済み ${formatBytes(embeddedStatus.size_bytes)}`,
          tone: 'ok',
        };
      }
      return { label: '未ダウンロード', tone: 'warn' };
    }
    if (providerId.startsWith('ollama.')) {
      // The provider itself just hits HTTP; we cannot tell from the
      // settings whether the user has an `ollama serve` running. The
      // honest read-out is "(別途) Ollama 必要" until a future
      // health-check IPC lands.
      return { label: 'Ollama 起動が必要', tone: 'info' };
    }
    if (providerId === 'deepl') {
      const env = read<string>('providers.configs.deepl.api_key_env', 'DEEPL_API_KEY');
      return {
        label: `API キー: ${env}`,
        tone: 'info',
      };
    }
    if (providerId === 'gemini') {
      const env = read<string>('providers.configs.gemini.api_key_env', 'GEMINI_API_KEY');
      return {
        label: `API キー: ${env}`,
        tone: 'info',
      };
    }
    return null;
  }

  /** The fixed catalog of translate engines we render. User-added
   *  ids (e.g. `plugin.my_translator`) are appended dynamically so
   *  they do not disappear from the UI just because we did not list
   *  them up-front. */
  const TRANSLATE_BUILTIN_ENGINES: ReadonlyArray<{
    id: string;
    title: string;
    subtitle: string;
    kind: EngineKind;
  }> = [
    {
      id: 'embedded.llm',
      title: '組み込みローカル LLM',
      subtitle: 'スタンドアロン翻訳エンジン (Apache 2.0)',
      kind: 'local',
    },
    {
      id: 'ollama.gemma4',
      title: 'Ollama 連携',
      subtitle: 'http://127.0.0.1:11434 で稼働するローカル LLM サーバ',
      kind: 'external',
    },
    {
      id: 'deepl',
      title: 'DeepL',
      subtitle: 'クラウド翻訳 API',
      kind: 'cloud',
    },
    {
      id: 'gemini',
      title: 'Google Gemini',
      subtitle: 'クラウド LLM API',
      kind: 'cloud',
    },
  ];

  // Translate-tab catalog integrity check. Same intent as the
  // chainCatalog self-check above, but scoped to the translate
  // tab's bespoke engine list — every id here must round-trip
  // through providerHints so the engine card renders with a
  // proper kind pill. Without this, a stale id (e.g.
  // `embedded.gemma` after the 12.x rename) would surface the
  // engine in the translate tab but render its hint as
  // "未分類のプロバイダ" — a contradictory state where the user
  // can toggle the card but the rest of the app silently treats
  // the id as unknown.
  {
    const known = new Set(Object.keys(providerHints));
    const missing = TRANSLATE_BUILTIN_ENGINES.filter(
      (e) => !known.has(e.id) && !e.id.startsWith('plugin.'),
    );
    if (missing.length > 0) {
      // eslint-disable-next-line no-console
      console.error(
        '[settings] TRANSLATE_BUILTIN_ENGINES drift — these ids lack hints:',
        missing.map((e) => e.id),
      );
    }
  }

  /** Compute the actual list of cards to render for the translate
   *  tab — the built-in catalog plus any custom ids the user has
   *  in their chain that we do not recognise (e.g. plugin providers). */
  const translateEngineList = $derived.by(() => {
    const chain: string[] = read('providers.translate.chain', []);
    const known = new Set(TRANSLATE_BUILTIN_ENGINES.map((e) => e.id));
    const custom = chain.filter((id) => !known.has(id));
    return [
      ...TRANSLATE_BUILTIN_ENGINES,
      ...custom.map((id) => ({
        id,
        title: id,
        subtitle: id.startsWith('plugin.')
          ? 'プラグイン提供のプロバイダ'
          : '未分類のプロバイダ',
        kind: 'external' as EngineKind,
      })),
    ];
  });

  /** Overall task-level health string. Drives the green/yellow
   *  banner at the top of the translate tab. */
  const translateHealth = $derived.by(() => {
    const chain: string[] = read('providers.translate.chain', []);
    if (chain.length === 0) {
      return {
        tone: 'warn' as const,
        label: '⚠️ 翻訳エンジンが未選択です — 下から1つ以上有効化してください',
      };
    }
    const first = chain[0];
    if (first === 'embedded.llm' || first.startsWith('embedded.')) {
      if (embeddedStatus.present) {
        return {
          tone: 'ok' as const,
          label: '✅ 翻訳できる状態です (組み込み LLM が応答可能)',
        };
      }
      if (chain.length > 1) {
        return {
          tone: 'warn' as const,
          label: `⚠️ 組み込みモデル未DL — フォールバックで ${chain[1]} が試されます`,
        };
      }
      return {
        tone: 'warn' as const,
        label: '⚠️ 組み込みモデルが未ダウンロードです',
      };
    }
    if (first.startsWith('ollama.')) {
      return {
        tone: 'info' as const,
        label: 'ℹ️ Ollama を優先設定 — 別途 `ollama serve` が必要です',
      };
    }
    if (first === 'deepl' || first === 'gemini') {
      return {
        tone: 'info' as const,
        label: `ℹ️ クラウド優先 — ${first.toUpperCase()} の API キー (環境変数) が必要です`,
      };
    }
    return {
      tone: 'info' as const,
      label: `ℹ️ ${first} を優先設定`,
    };
  });

  /** Wrapper that uses the model-tier registry for the embedded
   *  card's tier dropdown, which is shown inside the EngineCard
   *  detail body. Mirrors the previous tier-row behaviour but
   *  without the standalone embedded card. */
  function tierDescriptionLine(): string {
    if (!tierStatus) return '';
    return tierStatus.recommended_reason;
  }
</script>

<PageHeader title="設定" subtitle="変更は自動保存されます">
  {#snippet actions()}
    <div class="search-wrap">
      <Search class="search-ico h-3.5 w-3.5" aria-hidden="true" />
      <input
        class="input search-input"
        bind:value={query}
        placeholder="項目を検索"
      />
    </div>
    <button
      class="btn ghost"
      type="button"
      onclick={() => (confirmReset = true)}
      disabled={saveState === 'saving'}
      title="この画面を開いた時点の値に戻す"
    >
      <RotateCcw class="h-3.5 w-3.5" aria-hidden="true" />
      元に戻す
    </button>
    <button
      class="btn"
      type="button"
      onclick={saveNow}
      disabled={saveState !== 'dirty'}
      title="変更を即時保存"
    >
      <Save class="h-3.5 w-3.5" aria-hidden="true" />
      保存
    </button>
    <SaveIndicator state={saveState} {errorMessage} />
  {/snippet}
</PageHeader>

<div class="ml-content settings">
  <div class="settings-grid fadein">
    <!-- Tree-style sub-nav -->
    <aside class="sub-nav scroll" aria-label="設定セクション">
      {#each navTree as group (group.group)}
        <div class="nav-group" class:active={activeGroup === group.group}>
          <div class="nav-group-label">{group.group}</div>
          {#each group.items as item (item.id)}
            <button
              type="button"
              class="ml-nav-item"
              class:active={section === item.id}
              onclick={() => (section = item.id)}
            >
              <span class="ml-nav-icon">
                <item.icon class="h-4 w-4" aria-hidden="true" />
              </span>
              <span class="nav-label">{item.label}</span>
            </button>
          {/each}
        </div>
      {/each}
    </aside>

    <div class="scroll settings-body">
      {#if showRestartBanner}
        <div class="restart-banner" role="status" aria-live="polite">
          <div class="restart-icon">
            <AlertCircle class="h-5 w-5" aria-hidden="true" />
          </div>
          <div class="restart-text">
            <div class="restart-title">再起動が必要な変更があります</div>
            <p class="restart-body">
              変更したいくつかの設定 (DB 暗号化・ログレベル・フォントなど)
              は次回起動時から有効になります。今すぐ再起動するか、後で手動で
              MoonLens を再起動してください。
            </p>
          </div>
          <div class="restart-actions">
            <button
              type="button"
              class="btn primary"
              onclick={performRestart}
              disabled={restarting}
            >
              {#if restarting}
                <RotateCw class="h-3.5 w-3.5 spin" aria-hidden="true" />
                再起動中…
              {:else}
                <RotateCw class="h-3.5 w-3.5" aria-hidden="true" />
                今すぐ再起動
              {/if}
            </button>
          </div>
        </div>
        {#if restartError}
          <p class="restart-error">再起動に失敗: {restartError}</p>
        {/if}
      {/if}

      {#if !settings}
        {#if saveState === 'error'}
          <p class="error">{errorMessage}</p>
        {:else}
          <p class="muted">読み込み中…</p>
        {/if}
      {:else if section === 'appearance'}
        <header class="section-head">
          <h2>外観</h2>
          <p class="lead">アプリの見た目と読みやすさを調整します。</p>
        </header>

        <div class="card setting-card">
          <div class="row">
            <div class="row-text">
              <div class="row-label">テーマ</div>
              <div class="row-desc">アプリ全体の明るさ</div>
            </div>
            <Segmented
              value={read('appearance.theme', 'system')}
              onChange={(v) => update('appearance.theme', v)}
              options={[
                { value: 'light', label: 'ライト' },
                { value: 'dark', label: 'ダーク' },
                { value: 'system', label: '自動' }
              ]}
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">アクセントカラー</div>
              <div class="row-desc">ボタン・リンクの強調色</div>
            </div>
            <div class="swatch-row">
              {#each accentSwatches as sw (sw.value)}
                {@const active =
                  read('appearance.accent', '#A8C5DA').toLowerCase() ===
                  sw.value.toLowerCase()}
                <button
                  type="button"
                  class="swatch"
                  class:active
                  style="--swatch: {sw.value}"
                  onclick={() => update('appearance.accent', sw.value)}
                  title={sw.label}
                  aria-label={sw.label}
                >
                  <span class="swatch-dot"></span>
                  <span class="swatch-label">{sw.label}</span>
                </button>
              {/each}
              <input
                type="color"
                class="color-input"
                value={read('appearance.accent', '#A8C5DA')}
                oninput={(e) =>
                  update(
                    'appearance.accent',
                    (e.target as HTMLInputElement).value
                  )}
                aria-label="カスタムカラー"
              />
            </div>
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">UI 密度</div>
              <div class="row-desc">行間と要素間の詰め具合</div>
            </div>
            <Segmented
              value={read('appearance.density', 'normal')}
              onChange={(v) => update('appearance.density', v)}
              options={[
                { value: 'compact', label: 'コンパクト' },
                { value: 'normal', label: '標準' },
                { value: 'comfortable', label: 'ゆったり' }
              ]}
            />
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">UI 言語</div>
              <div class="row-desc">メニューやラベルの表示言語</div>
            </div>
            <select
              class="input narrow"
              value={read('appearance.ui_language', 'ja')}
              onchange={(e) =>
                update(
                  'appearance.ui_language',
                  (e.target as HTMLSelectElement).value
                )}
            >
              <option value="ja">日本語</option>
              <option value="en">English</option>
            </select>
          </div>
          <div class="row last">
            <div class="row-text">
              <div class="row-label">モーション削減</div>
              <div class="row-desc">アニメーションを最小限にします</div>
            </div>
            <Toggle
              checked={read('appearance.reduced_motion', false)}
              onChange={(v) => update('appearance.reduced_motion', v)}
              aria-label="モーション削減"
            />
          </div>
        </div>

        <details class="card setting-card collapsible">
          <summary class="row collapsible-head">
            <div class="row-text">
              <div class="row-label">フォント (詳細)</div>
              <div class="row-desc">既定のシステムフォントで通常は十分です</div>
            </div>
          </summary>
          <div class="row">
            <div class="row-text">
              <div class="row-label">UI フォント</div>
            </div>
            <input
              class="input narrow"
              type="text"
              value={read('appearance.font_family_ui', '')}
              oninput={(e) =>
                update(
                  'appearance.font_family_ui',
                  (e.target as HTMLInputElement).value
                )}
            />
          </div>
          <div class="row last">
            <div class="row-text">
              <div class="row-label">等幅フォント</div>
            </div>
            <input
              class="input narrow"
              type="text"
              value={read('appearance.font_family_mono', '')}
              oninput={(e) =>
                update(
                  'appearance.font_family_mono',
                  (e.target as HTMLInputElement).value
                )}
            />
          </div>
        </details>
      {:else if section === 'hotkeys'}
        <header class="section-head">
          <h2>ホットキー</h2>
          <p class="lead">
            グローバルホットキーは MoonLens がバックグラウンドにあるときも動作します。
          </p>
        </header>

        <div class="card setting-card">
          {#each hotkeyRows as row, i (row.key)}
            <div class="row" class:last={i === hotkeyRows.length - 1}>
              <div class="row-text">
                <div class="row-label">{row.label}</div>
              </div>
              <HotkeyInput
                value={read(`hotkeys.${row.key}`, '')}
                onChange={(v) => update(`hotkeys.${row.key}`, v)}
              />
            </div>
          {/each}
        </div>
      {:else if section === 'subtitle'}
        <header class="section-head">
          <h2>音声字幕</h2>
          <p class="lead">
            システム音声やマイクをリアルタイムで字幕化・翻訳します。
          </p>
        </header>

        <div class="card setting-card">
          <div class="kicker-row">言語</div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">原文の言語</div>
              <div class="row-desc">auto で自動検出</div>
            </div>
            <select
              class="input narrow"
              value={read('subtitle.source_language', 'auto')}
              onchange={(e) =>
                update(
                  'subtitle.source_language',
                  (e.target as HTMLSelectElement).value
                )}
            >
              <option value="auto">自動検出</option>
              <option value="en">英語</option>
              <option value="ja">日本語</option>
              <option value="zh">中国語</option>
              <option value="ko">韓国語</option>
            </select>
          </div>
          <div class="row last">
            <div class="row-text">
              <div class="row-label">翻訳先</div>
            </div>
            <select
              class="input narrow"
              value={read('subtitle.target_language', 'ja')}
              onchange={(e) =>
                update(
                  'subtitle.target_language',
                  (e.target as HTMLSelectElement).value
                )}
            >
              <option value="ja">日本語</option>
              <option value="en">英語</option>
            </select>
          </div>
        </div>

        <div class="card setting-card">
          <div class="kicker-row">認識</div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">チャンク長</div>
              <div class="row-desc">短いほどリアルタイム性が上がります</div>
            </div>
            <NumberSlider
              value={read('subtitle.chunk_seconds', 3.0)}
              onChange={(v) => update('subtitle.chunk_seconds', v)}
              min={1}
              max={10}
              step={0.5}
              suffix="秒"
              aria-label="チャンク長"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">オーバーラップ</div>
              <div class="row-desc">文の途中で切れにくくなります</div>
            </div>
            <NumberSlider
              value={read('subtitle.overlap_seconds', 0.5)}
              onChange={(v) => update('subtitle.overlap_seconds', v)}
              min={0}
              max={2}
              step={0.1}
              suffix="秒"
              aria-label="オーバーラップ"
            />
          </div>
          <div class="row stacked last">
            <div class="row-text">
              <div class="row-label">VAD 強度</div>
              <div class="row-desc">無音区間の検出感度 (0=甘い 3=厳しい)</div>
            </div>
            <NumberSlider
              value={read('subtitle.vad_aggressiveness', 2)}
              onChange={(v) => update('subtitle.vad_aggressiveness', v)}
              min={0}
              max={3}
              step={1}
              aria-label="VAD 強度"
            />
          </div>
        </div>

        <div class="card setting-card">
          <div class="kicker-row">表示</div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">原文を表示</div>
            </div>
            <Toggle
              checked={read('subtitle.show_original', true)}
              onChange={(v) => update('subtitle.show_original', v)}
              aria-label="原文を表示"
            />
          </div>
          <div class="row stacked last">
            <div class="row-text">
              <div class="row-label">字幕フォントサイズ</div>
            </div>
            <NumberSlider
              value={read('subtitle.font_size_px', 22)}
              onChange={(v) => update('subtitle.font_size_px', v)}
              min={14}
              max={48}
              step={1}
              suffix=" px"
              aria-label="字幕フォントサイズ"
            />
          </div>
        </div>
      {:else if section === 'overlay'}
        <header class="section-head">
          <h2>オーバーレイ・Lens</h2>
          <p class="lead">翻訳結果を表示するフローティングウィンドウの設定。</p>
        </header>

        <div class="card setting-card">
          <div class="kicker-row">見た目</div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">不透明度</div>
            </div>
            <NumberSlider
              value={read('overlay.opacity', 0.92)}
              onChange={(v) => update('overlay.opacity', v)}
              min={0.3}
              max={1}
              step={0.02}
              formatter={(v) => `${Math.round(v * 100)}`}
              suffix="%"
              aria-label="不透明度"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">背景ぼかし</div>
            </div>
            <NumberSlider
              value={read('overlay.blur_px', 24)}
              onChange={(v) => update('overlay.blur_px', v)}
              min={0}
              max={48}
              step={1}
              suffix=" px"
              aria-label="背景ぼかし"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">最大幅</div>
            </div>
            <NumberSlider
              value={read('overlay.max_width_px', 480)}
              onChange={(v) => update('overlay.max_width_px', v)}
              min={300}
              max={900}
              step={20}
              suffix=" px"
              aria-label="最大幅"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">フォントサイズ</div>
            </div>
            <NumberSlider
              value={read('overlay.font_size_px', 14)}
              onChange={(v) => update('overlay.font_size_px', v)}
              min={11}
              max={28}
              step={1}
              suffix=" px"
              aria-label="フォントサイズ"
            />
          </div>
          <div class="row stacked last">
            <div class="row-text">
              <div class="row-label">行間</div>
            </div>
            <NumberSlider
              value={read('overlay.line_height', 1.5)}
              onChange={(v) => update('overlay.line_height', v)}
              min={1}
              max={2.2}
              step={0.05}
              formatter={(v) => v.toFixed(2)}
              aria-label="行間"
            />
          </div>
        </div>

        <div class="card setting-card">
          <div class="kicker-row">動作</div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">ソースに追従</div>
              <div class="row-desc">キャプチャ領域の近くに表示</div>
            </div>
            <Toggle
              checked={read('overlay.follow_source', true)}
              onChange={(v) => update('overlay.follow_source', v)}
              aria-label="ソースに追従"
            />
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">表示位置</div>
              <div class="row-desc">追従 OFF 時の固定位置</div>
            </div>
            <Segmented
              value={read('overlay.position', 'near_source')}
              onChange={(v) => update('overlay.position', v)}
              options={[
                { value: 'near_source', label: 'ソース付近' },
                { value: 'top_right', label: '右上' },
                { value: 'bottom_right', label: '右下' },
                { value: 'center', label: '中央' }
              ]}
            />
          </div>
          <div class="row stacked last">
            <div class="row-text">
              <div class="row-label">Lens ポーリング間隔</div>
              <div class="row-desc">短いほど反応は早いが CPU 負荷が増えます</div>
            </div>
            <NumberSlider
              value={read('performance.lens_auto_poll_ms', 1500)}
              onChange={(v) => update('performance.lens_auto_poll_ms', v)}
              min={0}
              max={5000}
              step={100}
              suffix=" ms"
              formatter={(v) => (v === 0 ? 'OFF' : v.toString())}
              aria-label="Lens ポーリング間隔"
            />
          </div>
        </div>
      {:else if section === 'clipboard'}
        <header class="section-head">
          <h2>クリップボード</h2>
          <p class="lead">翻訳結果を自動でコピーするかを種類別に切り替えます。</p>
        </header>
        <div class="card setting-card">
          <div class="row">
            <div class="row-text">
              <div class="row-label">翻訳結果</div>
              <div class="row-desc">OCR + 翻訳の出力</div>
            </div>
            <Toggle
              checked={read('clipboard.auto_copy_translation', true)}
              onChange={(v) => update('clipboard.auto_copy_translation', v)}
              aria-label="翻訳結果"
            />
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">原文</div>
            </div>
            <Toggle
              checked={read('clipboard.auto_copy_source', false)}
              onChange={(v) => update('clipboard.auto_copy_source', v)}
              aria-label="原文"
            />
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">QR コード</div>
            </div>
            <Toggle
              checked={read('clipboard.auto_copy_qr', true)}
              onChange={(v) => update('clipboard.auto_copy_qr', v)}
              aria-label="QR コード"
            />
          </div>
          <div class="row last">
            <div class="row-text">
              <div class="row-label">文字起こし</div>
              <div class="row-desc">音声字幕の出力</div>
            </div>
            <Toggle
              checked={read('clipboard.auto_copy_transcript', false)}
              onChange={(v) => update('clipboard.auto_copy_transcript', v)}
              aria-label="文字起こし"
            />
          </div>
        </div>
      {:else if section === 'providers'}
        <header class="section-head">
          <h2>プロバイダ</h2>
          <p class="lead">
            各タスクで使うエンジンをチェックボックスで有効化します。チェックを入れたエンジンは順番に試され、失敗すると次のエンジンへフォールバックします。エンジンの ▼ ボタンで設定 (モデルダウンロード・APIキー・接続先) を開閉できます。
          </p>
        </header>

        <!-- Sub-tabs for each chain -->
        <div class="provider-tabs scroll-x" role="tablist">
          {#each chainCatalog as chain (chain.id)}
            {@const ids: string[] = read(`providers.${chain.id}.chain`, [])}
            <button
              type="button"
              role="tab"
              aria-selected={providerTab === chain.id}
              class="provider-tab"
              class:active={providerTab === chain.id}
              onclick={() => (providerTab = chain.id)}
            >
              <span class="tab-label">{chain.label}</span>
              {#if ids.length > 0}
                <span class="tab-count">{ids.length}</span>
              {:else}
                <span class="tab-count empty">未設定</span>
              {/if}
            </button>
          {/each}
        </div>

        <div class="card setting-card">
          <div class="kicker-row">
            {currentChainCatalog.label}
            <span class="kicker-desc">{currentChainCatalog.description}</span>
          </div>

          {#if providerTab === 'translate'}
            <!-- TASK-SPECIFIC ENGINE LIST: TRANSLATE -->
            <!--
              Each engine is one card. The checkbox toggles whether
              the engine is in the chain; the priority badge shows
              its position; the ▼ detail body holds engine-specific
              setup (download, API key, endpoint, …) so the user
              never has to leave the card to make the engine work.
            -->
            <div class="health-banner tone-{translateHealth.tone}">
              {translateHealth.label}
            </div>

            <div class="engine-list">
              {#each translateEngineList as engine (engine.id)}
                {@const enabled = engineEnabled('translate', engine.id)}
                {@const priority = enginePriority('translate', engine.id)}
                {@const detailOpenNow = isDetailOpen('translate', engine.id)}
                {@const status = translateEngineStatus(engine.id)}
                {@const recommended =
                  engine.id === 'embedded.llm' &&
                  tierStatus &&
                  tierStatus.current?.id === tierStatus.recommended.id}
                <EngineCard
                  title={engine.title}
                  subtitle={engine.subtitle}
                  providerId={engine.id}
                  kind={engine.kind}
                  {enabled}
                  {priority}
                  enabledCount={currentChainIds.length}
                  {status}
                  recommended={Boolean(recommended)}
                  detailOpen={detailOpenNow}
                  onToggle={(next) =>
                    setEngineEnabled('translate', engine.id, next)}
                  onMoveUp={priority && priority > 1
                    ? () => moveEngineUp('translate', engine.id)
                    : null}
                  onMoveDown={priority &&
                  priority < currentChainIds.length
                    ? () => moveEngineDown('translate', engine.id)
                    : null}
                  onToggleDetail={() => toggleDetailOpen('translate', engine.id)}
                >
                  {#if engine.id === 'embedded.llm'}
                    <!-- Embedded LLM detail: model status, tier
                         dropdown, download / package / import.
                         Replaces the standalone "組み込みローカル LLM"
                         card so DL state and the engine selection
                         live in the same visual block. -->
                    <div class="detail-grid">
                      <div class="detail-status">
                        {#if embeddedStatus.present}
                          <strong>準備済み</strong> · {formatBytes(embeddedStatus.size_bytes)}
                          {#if embeddedStatus.license}
                            · ライセンス: <strong>{embeddedStatus.license}</strong>
                            {#if embeddedStatus.license_url}
                              <a
                                href={embeddedStatus.license_url}
                                target="_blank"
                                rel="noreferrer noopener"
                                class="link-muted"
                              >
                                詳細
                              </a>
                            {/if}
                          {/if}
                          <div class="muted small mono">{embeddedStatus.path}</div>
                          {#if embeddedStatus.model_id}
                            <div class="muted small">モデル: {embeddedStatus.model_id}</div>
                          {/if}
                        {:else}
                          約 1GB の GGUF を
                          <strong>ダウンロード</strong>
                          するか、社内ファイルサーバ等から
                          <strong>パッケージファイル (.moonlens-model)</strong>
                          を取り込んでください。
                        {/if}
                      </div>

                      <div class="detail-row">
                        <label class="detail-label" for="user-tier-select">
                          翻訳ティア
                        </label>
                        <select
                          id="user-tier-select"
                          class="preset-btn"
                          value={userTierStatus?.current ?? 'standard'}
                          disabled={switchingUserTier}
                          onchange={(e) =>
                            handleUserTierChange(
                              (e.target as HTMLSelectElement).value as UserTier,
                            )}
                          style="min-width: 16rem;"
                        >
                          <option value="compact">Compact — 0.6 GB / EN↔JA 専用</option>
                          <option value="standard">Standard — 4.2 GB / 多言語対応</option>
                          <option value="quality">Quality — 6.1 GB / 最高品質</option>
                        </select>
                      </div>
                      <div class="muted small">
                        Tier を選ぶと言語ペアごとに最適なモデルが自動選択されます。
                        ハードウェア: {userTierStatus?.hardware === 'gpu' ? 'GPU' : 'CPU'}。
                      </div>
                      <div class="routing-preview">
                        {#each routingPreviews as row (row.label)}
                          <div class="routing-preview-row">
                            <span class="routing-preview-label">{row.label}</span>
                            <span class="routing-preview-value">
                              {renderRoutingPreview(row.preview)}
                            </span>
                          </div>
                        {/each}
                      </div>

                      {#if routedModels.length > 0}
                        <div class="routed-models">
                          <div class="routed-models-header">
                            必要なモデル
                          </div>
                          {#each routedModels as m (m.tier_id)}
                            <div class="routed-models-row">
                              <span class="routed-models-icon">
                                {m.present ? '✅' : '⬇️'}
                              </span>
                              <span class="routed-models-name">
                                {m.display_name}
                              </span>
                              <span class="routed-models-size">
                                {formatModelSize(m.size_mb)}
                              </span>
                              {#if m.present}
                                <span class="routed-models-status">
                                  準備済み
                                </span>
                              {:else}
                                <button
                                  type="button"
                                  class="routed-models-action"
                                  disabled={downloadingTiers[m.tier_id]}
                                  onclick={() =>
                                    handleDownloadRouted(m.tier_id)}
                                >
                                  {downloadingTiers[m.tier_id]
                                    ? 'ダウンロード中…'
                                    : 'ダウンロード'}
                                </button>
                              {/if}
                            </div>
                          {/each}
                        </div>
                      {/if}

                      {#if embeddedDownloading && embeddedProgress}
                        <div class="detail-progress">
                          {formatBytes(embeddedProgress.downloaded)}
                          {#if embeddedProgress.total}
                            / {formatBytes(embeddedProgress.total)}
                          {/if}
                          {#if embeddedProgress.fraction != null}
                            ({(embeddedProgress.fraction * 100).toFixed(1)}%)
                          {/if}
                          <progress
                            value={embeddedProgress.fraction ?? 0}
                            max="1"
                            style="width: 100%; margin-top: 0.25rem;"
                          ></progress>
                        </div>
                      {/if}

                      {#if embeddedError}
                        <div class="detail-error">エラー: {embeddedError}</div>
                      {/if}
                      {#if embeddedPackageMessage}
                        <div class="detail-info">{embeddedPackageMessage}</div>
                      {/if}

                      {#if embeddedStatus.present}
                        <!-- Embedded LLM benchmark. Runs 5
                             representative JA/EN translations and
                             reports per-sample latency + p50 / p95
                             tokens/sec — an honest read-out so the
                             user can compare their hardware against
                             the documented baseline (e.g. llama.cpp
                             Q4 0.5B ≈ 50 tok/s on a 2022-era
                             office laptop). If their numbers come
                             out an order of magnitude below, the
                             runtime (mistralrs) is the bottleneck
                             and a swap to llama-cpp-rs is justified. -->
                        <div class="bench-section">
                          <button
                            type="button"
                            class="preset-btn"
                            disabled={benchRunning ||
                              embeddedDownloading ||
                              embeddedImporting}
                            onclick={runEmbeddedBench}
                          >
                            {benchRunning
                              ? 'ベンチ実行中… (5サンプル直列)'
                              : '実機ベンチを実行 (5サンプル翻訳)'}
                          </button>

                          <!-- Live progress while benchRunning. Without
                               this panel the user stares at a button
                               that just says "実行中…" for minutes. The
                               token counter + elapsed time prove the
                               engine is still alive even on slow CPUs
                               where individual tokens take seconds. -->
                          {#if benchRunning && benchProgress}
                            {@const pct =
                              benchProgress.totalSamples > 0
                                ? Math.min(
                                    100,
                                    ((benchProgress.completedSamples.length +
                                      (benchProgress.currentIndex > 0 ? 0.5 : 0)) /
                                      benchProgress.totalSamples) *
                                      100,
                                  )
                                : 0}
                            <div class="bench-progress">
                              <div class="bench-progress-headline">
                                {#if benchProgress.currentIndex > 0}
                                  サンプル <strong
                                    >{benchProgress.currentIndex}/{benchProgress.totalSamples}</strong
                                  > 実行中
                                  ・ <strong>{benchProgress.tokensSoFar}</strong> tokens 生成
                                  ・ {(benchProgress.elapsedMs / 1000).toFixed(1)} 秒経過
                                {:else}
                                  ベンチ準備中…
                                {/if}
                              </div>
                              <progress value={pct} max="100" class="bench-progress-bar"></progress>
                              {#if benchProgress.fixtureText}
                                <div class="muted small">
                                  入力: <code class="muted">{benchProgress.fixtureText}</code>
                                </div>
                              {/if}
                              {#if benchProgress.completedSamples.length > 0}
                                <div class="muted small">
                                  完了済み: {benchProgress.completedSamples
                                    .map(
                                      (s) =>
                                        `#${s.index} ${s.tokens_per_second.toFixed(1)} tok/s`,
                                    )
                                    .join(' / ')}
                                </div>
                              {/if}
                            </div>
                          {/if}

                          {#if benchError}
                            <div class="detail-error">
                              ベンチ失敗: {benchError}
                            </div>
                          {/if}
                          {#if benchResult}
                            {@const expected = expectedTokensPerSecondFor(
                              tierStatus?.current?.id,
                            )}
                            {@const decodeRatio = expected
                              ? (benchResult.p50_decode_tokens_per_second /
                                  expected) *
                                100
                              : null}
                            <div class="bench-result">
                              <div class="bench-headline">
                                <strong>生成速度 (decode)</strong>
                                p50 {benchResult.p50_decode_tokens_per_second.toFixed(1)}
                                / mean {benchResult.mean_decode_tokens_per_second.toFixed(1)} tok/s
                                {#if expected != null}
                                  <span
                                    class:bench-ratio-bad={decodeRatio != null && decodeRatio < 30}
                                    class:bench-ratio-warn={decodeRatio != null && decodeRatio >= 30 && decodeRatio < 70}
                                    class:bench-ratio-ok={decodeRatio != null && decodeRatio >= 70}
                                    class="bench-ratio"
                                  >
                                    (期待値 {expected} tok/s の {decodeRatio?.toFixed(0)}%)
                                  </span>
                                {/if}
                              </div>
                              <div class="bench-subline muted small">
                                <strong>per-line wall-clock</strong>
                                p50 {benchResult.p50_tokens_per_second.toFixed(1)}
                                / mean {benchResult.mean_tokens_per_second.toFixed(1)} tok/s
                                — context 構築 + プロンプト評価込みの
                                ライン単体スループット
                              </div>
                              <div class="muted small">
                                {benchResult.tier_display_name}
                                ・ RAM {benchResult.host_total_ram_gb.toFixed(1)} GB
                                ・ {benchResult.host_cpu_cores} cores
                                ・ 全 {(benchResult.total_run_ms / 1000).toFixed(1)} 秒
                              </div>
                              <details class="bench-detail">
                                <summary>サンプル別の詳細</summary>
                                <table class="bench-table">
                                  <thead>
                                    <tr>
                                      <th>#</th>
                                      <th>tokens</th>
                                      <th>total ms</th>
                                      <th>prompt eval ms</th>
                                      <th>decode tok/s</th>
                                      <th>wall tok/s</th>
                                      <th>preview</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {#each benchResult.samples as s (s.index)}
                                      <tr>
                                        <td>{s.index}</td>
                                        <td>{s.output_tokens}</td>
                                        <td>{s.total_ms}</td>
                                        <td>{s.prompt_eval_ms}</td>
                                        <td>{s.decode_tokens_per_second.toFixed(1)}</td>
                                        <td>{s.tokens_per_second.toFixed(1)}</td>
                                        <td class="bench-preview">{s.translated_preview}</td>
                                      </tr>
                                    {/each}
                                  </tbody>
                                </table>
                              </details>
                              {#if expected != null && decodeRatio != null && decodeRatio < 30}
                                <div class="bench-verdict">
                                  ⚠️ 生成速度が期待値の 30% 未満です。
                                  考えられる原因: ① CPU が古い (AVX2 非対応 / 5+ 年前)、
                                  ② バックグラウンドプロセスによる負荷、
                                  ③ DDR メモリ帯域不足、④ サーマルスロットリング。
                                  低位ティアへ切替えるか、常駐ソフト停止後に再ベンチを推奨。
                                  <code class="muted">docs/EMBEDDED_LLM_PERF.md</code> 参照。
                                </div>
                              {:else if expected != null && decodeRatio != null && decodeRatio < 70}
                                <div class="bench-verdict bench-verdict-warn">
                                  生成速度は期待値の 30〜70%。CPU 世代相応の
                                  数値です。より高速にしたい場合は不要な常駐プロセスを
                                  停止するか、低位ティアへ切替えてください。
                                </div>
                              {:else if expected != null && decodeRatio != null}
                                <div class="bench-verdict bench-verdict-ok">
                                  ✓ 生成速度は期待値の 70% 以上 — llama.cpp が
                                  期待通り動作しています。per-line wall-clock が
                                  低い場合は context 構築 + プロンプト評価が
                                  支配的なため、複数行を連続翻訳すれば
                                  実効スループットは向上します。
                                </div>
                              {/if}
                            </div>
                          {/if}
                        </div>
                      {/if}

                      <div class="detail-actions">
                        {#if !embeddedStatus.present}
                          <button
                            type="button"
                            class="preset-btn"
                            disabled={embeddedDownloading || embeddedImporting}
                            onclick={() => startEmbeddedDownload(false)}
                          >
                            {embeddedDownloading
                              ? 'ダウンロード中…'
                              : 'ダウンロード'}
                          </button>
                          <button
                            type="button"
                            class="preset-btn"
                            disabled={embeddedDownloading || embeddedImporting}
                            onclick={importPackage}
                            title="社内のファイルサーバ等から .moonlens-model パッケージを取り込む"
                          >
                            {embeddedImporting
                              ? '取込中…'
                              : 'パッケージから取込'}
                          </button>
                        {:else}
                          <button
                            type="button"
                            class="preset-btn"
                            disabled={embeddedDownloading}
                            onclick={() => startEmbeddedDownload(true)}
                          >
                            再ダウンロード
                          </button>
                          <button
                            type="button"
                            class="preset-btn"
                            disabled={embeddedPackaging}
                            onclick={packageCurrentModel}
                            title="ダウンロード済みモデルを .moonlens-model にパッケージ化して、社内のオフライン PC へ配布できるようにします"
                          >
                            {embeddedPackaging
                              ? 'パッケージ化中…'
                              : 'パッケージ化'}
                          </button>
                          <button
                            type="button"
                            class="preset-btn"
                            disabled={embeddedImporting}
                            onclick={importPackage}
                            title="別のパッケージファイルから取込 (上書き)"
                          >
                            {embeddedImporting ? '取込中…' : 'パッケージから取込'}
                          </button>
                        {/if}
                      </div>
                    </div>
                  {:else if engine.id === 'ollama.gemma4'}
                    <!-- Ollama detail: endpoint + model + reminder
                         that the daemon must be running separately. -->
                    <div class="detail-grid">
                      <div class="detail-status">
                        外部 Ollama サーバを翻訳に使います。Ollama を別途
                        インストールして
                        <code class="muted">ollama serve</code> で起動した上で、
                        <code class="muted">ollama pull gemma4:e2b</code>
                        を済ませておく必要があります。
                      </div>
                      <div class="detail-row">
                        <label class="detail-label" for="ollama-endpoint">
                          エンドポイント
                        </label>
                        <input
                          id="ollama-endpoint"
                          class="input"
                          type="text"
                          value={read<string>(
                            'providers.configs."ollama.gemma4".endpoint',
                            'http://127.0.0.1:11434',
                          )}
                          oninput={(e) =>
                            update(
                              'providers.configs."ollama.gemma4".endpoint',
                              (e.target as HTMLInputElement).value,
                            )}
                        />
                      </div>
                      <div class="detail-row">
                        <label class="detail-label" for="ollama-model">
                          モデル
                        </label>
                        <input
                          id="ollama-model"
                          class="input"
                          type="text"
                          value={read<string>(
                            'providers.configs."ollama.gemma4".model',
                            'gemma4:e2b',
                          )}
                          oninput={(e) =>
                            update(
                              'providers.configs."ollama.gemma4".model',
                              (e.target as HTMLInputElement).value,
                            )}
                        />
                      </div>
                    </div>
                  {:else if engine.id === 'deepl'}
                    <!-- DeepL detail: API key env var + formality. -->
                    <div class="detail-grid">
                      <div class="detail-status">
                        DeepL クラウド翻訳 API を使います。API キーは
                        <code class="muted">.env</code>
                        / 環境変数経由で渡してください (キー名は下で指定)。
                      </div>
                      <div class="detail-row">
                        <label class="detail-label" for="deepl-key-env">
                          API キーの環境変数名
                        </label>
                        <input
                          id="deepl-key-env"
                          class="input"
                          type="text"
                          value={read<string>(
                            'providers.configs.deepl.api_key_env',
                            'DEEPL_API_KEY',
                          )}
                          oninput={(e) =>
                            update(
                              'providers.configs.deepl.api_key_env',
                              (e.target as HTMLInputElement).value,
                            )}
                        />
                      </div>
                      <div class="detail-row">
                        <label class="detail-label" for="deepl-formality">
                          formality
                        </label>
                        <input
                          id="deepl-formality"
                          class="input"
                          type="text"
                          value={read<string>(
                            'providers.configs.deepl.formality',
                            'default',
                          )}
                          oninput={(e) =>
                            update(
                              'providers.configs.deepl.formality',
                              (e.target as HTMLInputElement).value,
                            )}
                        />
                      </div>
                    </div>
                  {:else if engine.id === 'gemini'}
                    <!-- Gemini detail: API key env + model. -->
                    <div class="detail-grid">
                      <div class="detail-status">
                        Google Gemini クラウド LLM を使います。API キーは
                        <code class="muted">.env</code>
                        / 環境変数経由で渡してください。
                      </div>
                      <div class="detail-row">
                        <label class="detail-label" for="gemini-key-env">
                          API キーの環境変数名
                        </label>
                        <input
                          id="gemini-key-env"
                          class="input"
                          type="text"
                          value={read<string>(
                            'providers.configs.gemini.api_key_env',
                            'GEMINI_API_KEY',
                          )}
                          oninput={(e) =>
                            update(
                              'providers.configs.gemini.api_key_env',
                              (e.target as HTMLInputElement).value,
                            )}
                        />
                      </div>
                      <div class="detail-row">
                        <label class="detail-label" for="gemini-model">
                          モデル
                        </label>
                        <input
                          id="gemini-model"
                          class="input"
                          type="text"
                          value={read<string>(
                            'providers.configs.gemini.model',
                            'gemini-2.5-flash',
                          )}
                          oninput={(e) =>
                            update(
                              'providers.configs.gemini.model',
                              (e.target as HTMLInputElement).value,
                            )}
                        />
                      </div>
                    </div>
                  {:else}
                    <!-- Plugin / unknown provider — no curated UI,
                         the user added it via custom id input. -->
                    <div class="detail-status">
                      カスタムプロバイダ — 詳細設定は
                      <code class="muted">providers.configs."{engine.id}"</code>
                      を user.toml で直接編集してください。
                    </div>
                  {/if}
                </EngineCard>
              {/each}

              <!-- Custom id input — kept for power users (plugin
                   providers, bespoke ids) that the registry does
                   not know about up-front. -->
              <div class="chain-add">
                <input
                  class="input"
                  type="text"
                  placeholder="その他のエンジン ID (例: plugin.my-engine)"
                  value={newChainEntries[providerTab] ?? ''}
                  oninput={(e) =>
                    (newChainEntries = {
                      ...newChainEntries,
                      [providerTab]: (e.target as HTMLInputElement).value
                    })}
                  onkeydown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      addChainEntry(providerTab);
                    }
                  }}
                />
                <button
                  class="btn"
                  type="button"
                  onclick={() => addChainEntry(providerTab)}
                >
                  <Plus class="h-3.5 w-3.5" aria-hidden="true" /> カスタム追加
                </button>
              </div>
            </div>
          {:else}
            <!-- TASK-SPECIFIC ENGINE LIST: OCR / ASR / others -->
            <!--
              Same EngineCard pattern as translate but driven by
              `chainCatalog[task].suggestions` plus any custom ids
              already in the chain. Detail bodies are intentionally
              skipped here — these tasks do not have model downloads
              or API keys, so a simple toggle + status pill is
              enough. Detailed config still lives in user.toml for
              power users.
            -->
            <div class="engine-list">
              {#each currentChainCatalog.suggestions
                .concat(currentChainIds.filter(
                  (id) => !currentChainCatalog.suggestions.includes(id),
                )) as engineId, idx (engineId + idx)}
                {@const enabled = engineEnabled(providerTab, engineId)}
                {@const priority = enginePriority(providerTab, engineId)}
                {@const hint = hintFor(engineId)}
                <EngineCard
                  title={hint.label}
                  subtitle={hint.description}
                  providerId={engineId}
                  kind={hint.kind}
                  {enabled}
                  {priority}
                  enabledCount={currentChainIds.length}
                  status={null}
                  detailOpen={false}
                  onToggle={(next) =>
                    setEngineEnabled(providerTab, engineId, next)}
                  onMoveUp={priority && priority > 1
                    ? () => moveEngineUp(providerTab, engineId)
                    : null}
                  onMoveDown={priority &&
                  priority < currentChainIds.length
                    ? () => moveEngineDown(providerTab, engineId)
                    : null}
                  onToggleDetail={null}
                />
              {/each}

              <div class="chain-add">
                <input
                  class="input"
                  type="text"
                  placeholder="その他のエンジン ID (例: plugin.my-engine)"
                  value={newChainEntries[providerTab] ?? ''}
                  oninput={(e) =>
                    (newChainEntries = {
                      ...newChainEntries,
                      [providerTab]: (e.target as HTMLInputElement).value
                    })}
                  onkeydown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      addChainEntry(providerTab);
                    }
                  }}
                />
                <button
                  class="btn"
                  type="button"
                  onclick={() => addChainEntry(providerTab)}
                >
                  <Plus class="h-3.5 w-3.5" aria-hidden="true" /> カスタム追加
                </button>
              </div>
            </div>
          {/if}

          <div class="kicker-row mt-divider">挙動</div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">並列レース</div>
              <div class="row-desc">全プロバイダに同時送信し最初の応答を採用</div>
            </div>
            <Toggle
              checked={read(`providers.${providerTab}.parallel_race`, false)}
              onChange={(v) =>
                update(`providers.${providerTab}.parallel_race`, v)}
              aria-label="並列レース"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">タイムアウト</div>
              <div class="row-desc">単一プロバイダへの応答待ち上限</div>
            </div>
            <NumberSlider
              value={read(`providers.${providerTab}.timeout_ms`, 3000)}
              onChange={(v) =>
                update(`providers.${providerTab}.timeout_ms`, v)}
              min={500}
              max={60000}
              step={500}
              suffix=" ms"
              aria-label="タイムアウト"
            />
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">クラウドを許可</div>
              <div class="row-desc">クラウド分類のプロバイダを利用可能に</div>
            </div>
            <Toggle
              checked={read(`providers.${providerTab}.allow_cloud`, false)}
              onChange={(v) =>
                update(`providers.${providerTab}.allow_cloud`, v)}
              aria-label="クラウドを許可"
            />
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">ネットワークを許可</div>
              <div class="row-desc">外部サーバ・API への通信を許可</div>
            </div>
            <Toggle
              checked={read(`providers.${providerTab}.allow_network`, false)}
              onChange={(v) =>
                update(`providers.${providerTab}.allow_network`, v)}
              aria-label="ネットワークを許可"
            />
          </div>
          <div class="row stacked last">
            <div class="row-text">
              <div class="row-label">フォールバック条件</div>
              <div class="row-desc">次のプロバイダに切り替える条件</div>
            </div>
            <div class="fallback-row">
              {#each fallbackOptions as opt (opt.value)}
                {@const checked = read<string[]>(
                  `providers.${providerTab}.fallback_on`,
                  []
                ).includes(opt.value)}
                <label class="chip" class:active={checked}>
                  <input
                    type="checkbox"
                    {checked}
                    onchange={() => toggleFallback(providerTab, opt.value)}
                  />
                  {opt.label}
                </label>
              {/each}
            </div>
          </div>
        </div>
      {:else if section === 'plugins'}
        <header class="section-head">
          <h2>プラグイン</h2>
          <p class="lead">
            プラグインの一覧と有効化は <a href="/plugins/" class="inline-link">プラグインページ</a>
            から行えます。ここではセキュリティ設定を扱います。
          </p>
        </header>
        <div class="card setting-card">
          <div class="row">
            <div class="row-text">
              <div class="row-label">自動更新チェック</div>
              <div class="row-desc">起動時に新バージョンを確認</div>
            </div>
            <Toggle
              checked={read('plugins.auto_update_check', false)}
              onChange={(v) => update('plugins.auto_update_check', v)}
              aria-label="自動更新チェック"
            />
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">署名を必須にする</div>
              <div class="row-desc">未署名プラグインの有効化を拒否</div>
            </div>
            <Toggle
              checked={read('plugins.require_signature', false)}
              onChange={(v) => update('plugins.require_signature', v)}
              aria-label="署名を必須にする"
            />
          </div>
          <div class="row stacked last">
            <div class="row-text">
              <div class="row-label">信頼済み発行者</div>
              <div class="row-desc">改行・カンマ区切りの公開鍵フィンガープリント</div>
            </div>
            <textarea
              class="input textarea"
              value={trustedPublishersText()}
              oninput={(e) =>
                setTrustedPublishersText(
                  (e.target as HTMLTextAreaElement).value
                )}
              placeholder="例: ed25519:abc..."
            ></textarea>
          </div>
        </div>
      {:else if section === 'performance'}
        <header class="section-head">
          <h2>パフォーマンス</h2>
          <p class="lead">CPU 負荷とレスポンス速度のバランス。</p>
        </header>
        <div class="card setting-card">
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">同時実行タスク</div>
              <div class="row-desc">並列で走らせる翻訳/OCR の上限</div>
            </div>
            <NumberSlider
              value={read('performance.max_concurrent_tasks', 4)}
              onChange={(v) => update('performance.max_concurrent_tasks', v)}
              min={1}
              max={16}
              step={1}
              aria-label="同時実行タスク"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">ストリーミング debounce</div>
            </div>
            <NumberSlider
              value={read('performance.streaming_debounce_ms', 50)}
              onChange={(v) =>
                update('performance.streaming_debounce_ms', v)}
              min={10}
              max={500}
              step={10}
              suffix=" ms"
              aria-label="ストリーミング debounce"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">領域監視ポーリング</div>
              <div class="row-desc">ピン留めオーバーレイの再キャプチャ間隔</div>
            </div>
            <NumberSlider
              value={read('performance.region_watcher_poll_ms', 250)}
              onChange={(v) =>
                update('performance.region_watcher_poll_ms', v)}
              min={50}
              max={2000}
              step={50}
              suffix=" ms"
              aria-label="領域監視ポーリング"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">キャッシュ TTL</div>
            </div>
            <NumberSlider
              value={read('performance.cache_ttl_seconds', 3600)}
              onChange={(v) => update('performance.cache_ttl_seconds', v)}
              min={0}
              max={86400}
              step={60}
              suffix=" 秒"
              aria-label="キャッシュ TTL"
            />
          </div>
          <div class="row last">
            <div class="row-text">
              <div class="row-label">GPU を使う</div>
              <div class="row-desc">利用可能なら GPU を優先 (OCR/ASR)</div>
            </div>
            <Toggle
              checked={read('performance.use_gpu_if_available', true)}
              onChange={(v) => update('performance.use_gpu_if_available', v)}
              aria-label="GPU を使う"
            />
          </div>
        </div>
      {:else if section === 'storage'}
        <header class="section-head">
          <h2>履歴・ストレージ</h2>
          <p class="lead">翻訳履歴とキャプチャデータの保管ポリシー。</p>
        </header>
        <div class="card setting-card">
          <div class="row">
            <div class="row-text">
              <div class="row-label">履歴を保存</div>
              <div class="row-desc">OFF にすると翻訳結果がディスクに残りません</div>
            </div>
            <Toggle
              checked={read('storage.keep_history', true)}
              onChange={(v) => update('storage.keep_history', v)}
              aria-label="履歴を保存"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">保持期間</div>
              <div class="row-desc">この日数より古い履歴は自動削除</div>
            </div>
            <NumberSlider
              value={read('storage.retention_days', 90)}
              onChange={(v) => update('storage.retention_days', v)}
              min={0}
              max={3650}
              step={1}
              suffix=" 日"
              formatter={(v) => (v === 0 ? '無制限' : v.toString())}
              aria-label="保持期間"
            />
          </div>
          <div class="row stacked">
            <div class="row-text">
              <div class="row-label">最大エントリ数</div>
            </div>
            <NumberSlider
              value={read('storage.max_entries', 10000)}
              onChange={(v) => update('storage.max_entries', v)}
              min={100}
              max={100000}
              step={100}
              aria-label="最大エントリ数"
            />
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">キャプチャ画像を保存</div>
              <div class="row-desc">OFF だと画像はテキストのみ保存</div>
            </div>
            <Toggle
              checked={read('storage.save_capture_images', true)}
              onChange={(v) => update('storage.save_capture_images', v)}
              aria-label="キャプチャ画像を保存"
            />
          </div>
          <div class="row last">
            <div class="row-text">
              <div class="row-label">ディスク暗号化</div>
              <div class="row-desc">履歴 DB を OS 鍵で暗号化 (要再起動)</div>
            </div>
            <Toggle
              checked={read('storage.encrypt_at_rest', false)}
              onChange={(v) => update('storage.encrypt_at_rest', v)}
              aria-label="ディスク暗号化"
            />
          </div>
        </div>
      {:else if section === 'privacy'}
        <header class="section-head">
          <h2>プライバシー</h2>
          <p class="lead">外部送信の許可とログの取り扱い。</p>
        </header>

        <div class="card privacy-banner">
          <Shield class="h-5 w-5 accent-text" aria-hidden="true" />
          <div>
            <div class="banner-title">既定はローカル処理</div>
            <p class="muted small banner-body">
              MoonLens はオフラインでも動作するよう設計されています。クラウドフォールバックは下の設定で個別に許可できます。
            </p>
          </div>
        </div>

        <div class="card setting-card">
          <div class="kicker-row">クラウド連携</div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">クラウドプロバイダの利用を許可</div>
              <div class="row-desc">DeepL / Gemini などをチェーンに追加可能に</div>
            </div>
            <Toggle
              checked={read('privacy.allow_cloud_globally', false)}
              onChange={(v) => update('privacy.allow_cloud_globally', v)}
              aria-label="クラウドプロバイダの利用を許可"
            />
          </div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">クラウド使用時に通知</div>
            </div>
            <Toggle
              checked={read('privacy.notify_on_fallback_to_cloud', true)}
              onChange={(v) =>
                update('privacy.notify_on_fallback_to_cloud', v)}
              aria-label="クラウド使用時に通知"
            />
          </div>
          <div class="row last">
            <div class="row-text">
              <div class="row-label">プライバシーバッジを表示</div>
            </div>
            <Toggle
              checked={read('privacy.show_privacy_indicator', true)}
              onChange={(v) =>
                update('privacy.show_privacy_indicator', v)}
              aria-label="プライバシーバッジを表示"
            />
          </div>
        </div>

        <div class="card setting-card">
          <div class="kicker-row">ログ</div>
          <div class="row">
            <div class="row-text">
              <div class="row-label">ログレベル</div>
            </div>
            <Segmented
              value={read('privacy.log_level', 'info')}
              onChange={(v) => update('privacy.log_level', v)}
              options={[
                { value: 'error', label: 'エラー' },
                { value: 'warn', label: '警告' },
                { value: 'info', label: '通常' },
                { value: 'debug', label: 'デバッグ' },
                { value: 'trace', label: 'トレース' }
              ]}
            />
          </div>
          <div class="row last">
            <div class="row-text">
              <div class="row-label">ログから機密を伏せる</div>
            </div>
            <Toggle
              checked={read('privacy.redact_sensitive_in_logs', true)}
              onChange={(v) =>
                update('privacy.redact_sensitive_in_logs', v)}
              aria-label="ログから機密を伏せる"
            />
          </div>
        </div>
      {:else if section === 'about'}
        <div class="card about">
          <MoonLensMark size={64} />
          <div>
            <div class="about-name">MoonLens</div>
            <div class="muted small">
              {appVersion ? `v${appVersion}` : 'バージョン情報を取得中…'}
            </div>
          </div>
          <p class="about-desc">
            ローカル組み込みモデルで動作する、プライバシー重視のオフライン翻訳ツール。
            外部サーバやクラウド API は完全にオプションです。
          </p>
          <div class="about-actions">
            <button class="btn" type="button">
              <RotateCcw class="h-3.5 w-3.5" aria-hidden="true" />
              更新を確認
            </button>
            <button class="btn" type="button">
              <Folder class="h-3.5 w-3.5" aria-hidden="true" />
              データフォルダ
            </button>
            <a
              class="btn"
              href="https://github.com/moonlens"
              target="_blank"
              rel="noreferrer"
            >
              <Globe class="h-3.5 w-3.5" aria-hidden="true" />
              Web サイト
            </a>
          </div>
        </div>
      {:else if section === 'advanced'}
        <header class="section-head">
          <h2>詳細 (生スキーマ)</h2>
          <p class="lead">
            GUI が用意されていない項目を直接編集します。誤った値はパイプラインを停止させる可能性があります。
          </p>
        </header>
        {#if schema && advancedMirror}
          <SchemaForm
            {schema}
            bind:value={advancedMirror}
            {query}
            topic=""
            onChange={onAdvancedChange}
          />
        {/if}
      {/if}
    </div>
  </div>
</div>

<CommandPalette />

{#if confirmReset}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={() => (confirmReset = false)}
  >
    <div
      class="card modal"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <h2>変更を破棄しますか？</h2>
      <p class="muted">
        このセッションで変更した全ての項目が、開いた時点の値に戻ります。
      </p>
      <div class="modal-actions">
        <button class="btn" type="button" onclick={() => (confirmReset = false)}>
          キャンセル
        </button>
        <button class="btn primary" type="button" onclick={resetAll}>戻す</button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* ----- Engine card list (providers tab) -----
   * Each engine = one EngineCard. The list itself just stacks
   * cards with a uniform gap so the priority badges line up. */
  .engine-list {
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
    padding: 0.5rem 0;
  }
  .health-banner {
    margin: 0.5rem 0;
    padding: 0.625rem 0.875rem;
    border-radius: 8px;
    font-size: 0.875rem;
    line-height: 1.4;
    border: 1px solid;
  }
  .health-banner.tone-ok {
    background: color-mix(in srgb, #10b981 12%, transparent);
    border-color: color-mix(in srgb, #10b981 35%, transparent);
    color: color-mix(in srgb, #10b981 95%, var(--ink));
  }
  .health-banner.tone-warn {
    background: color-mix(in srgb, #f59e0b 12%, transparent);
    border-color: color-mix(in srgb, #f59e0b 35%, transparent);
    color: color-mix(in srgb, #f59e0b 95%, var(--ink));
  }
  .health-banner.tone-info {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
    color: var(--ink);
  }
  /* HW capability mismatch warning. Sat between the tier dropdown
     and the download button so the user reads the warning BEFORE
     they kick off a multi-GB download for a tier they cannot
     realistically run. */
  .hw-warning {
    margin-top: 0.5rem;
    padding: 0.625rem 0.875rem;
    border-radius: 8px;
    font-size: 0.8125rem;
    line-height: 1.5;
    background: color-mix(in srgb, #ef4444 10%, transparent);
    border: 1px solid color-mix(in srgb, #ef4444 35%, transparent);
    color: color-mix(in srgb, #ef4444 95%, var(--ink));
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }
  .hw-warning strong {
    color: color-mix(in srgb, #ef4444 100%, var(--ink));
  }
  .hw-warning-action {
    margin-left: auto;
    padding: 0.25rem 0.625rem;
    border-radius: 6px;
    border: 1px solid color-mix(in srgb, #ef4444 50%, transparent);
    background: color-mix(in srgb, #ef4444 18%, transparent);
    color: color-mix(in srgb, #ef4444 100%, var(--ink));
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease;
  }
  .hw-warning-action:hover:not(:disabled) {
    background: color-mix(in srgb, #ef4444 28%, transparent);
    border-color: color-mix(in srgb, #ef4444 70%, transparent);
  }
  .hw-warning-action:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  /* Routing-preview matrix. Sat under the routing-tier picker so
     the user can see at a glance which model handles each common
     language pair before committing to the tier. */
  .routing-preview {
    margin-top: 0.5rem;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, var(--ink) 12%, transparent);
    background: color-mix(in srgb, var(--ink) 4%, transparent);
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .routing-preview-row {
    display: flex;
    gap: 0.75rem;
    font-size: 0.8125rem;
    line-height: 1.5;
  }
  .routing-preview-label {
    min-width: 4rem;
    color: color-mix(in srgb, var(--ink) 65%, transparent);
  }
  .routing-preview-value {
    flex: 1;
    color: color-mix(in srgb, var(--ink) 90%, transparent);
  }
  /* Per-model readiness panel for the active routing tier.
     Distinct from `.routing-preview` because the routing matrix
     and the per-model GGUF inventory answer different questions
     ("which model runs for this pair?" vs "which GGUFs do I
     still need to download?"). Both are scoped under the
     embedded.llm detail body so they share the parent's
     padding. */
  .routed-models {
    margin-top: 0.5rem;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, var(--ink) 12%, transparent);
    background: color-mix(in srgb, var(--ink) 4%, transparent);
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .routed-models-header {
    font-size: 0.8125rem;
    font-weight: 500;
    color: color-mix(in srgb, var(--ink) 75%, transparent);
    margin-bottom: 0.125rem;
  }
  .routed-models-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    font-size: 0.8125rem;
    line-height: 1.5;
  }
  .routed-models-icon {
    width: 1.25rem;
    text-align: center;
  }
  .routed-models-name {
    flex: 1;
    color: color-mix(in srgb, var(--ink) 90%, transparent);
  }
  .routed-models-size {
    min-width: 4rem;
    text-align: right;
    color: color-mix(in srgb, var(--ink) 65%, transparent);
    font-variant-numeric: tabular-nums;
  }
  .routed-models-status {
    min-width: 5.5rem;
    text-align: right;
    color: color-mix(in srgb, var(--ink) 70%, transparent);
  }
  .routed-models-action {
    min-width: 5.5rem;
    padding: 0.25rem 0.625rem;
    border-radius: 6px;
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: color-mix(in srgb, var(--accent) 100%, var(--ink));
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease;
  }
  .routed-models-action:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .routed-models-action:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .bench-section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px dashed var(--border);
  }
  .bench-progress {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    background: color-mix(in srgb, var(--accent) 8%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    font-size: 0.8125rem;
  }
  .bench-progress-headline {
    color: var(--ink);
  }
  .bench-progress-bar {
    width: 100%;
    height: 0.375rem;
  }
  .bench-result {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
    padding: 0.625rem 0.75rem;
    border-radius: 8px;
    background: var(--surface);
    border: 1px solid var(--border);
    font-size: 0.8125rem;
  }
  .bench-headline {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: baseline;
    line-height: 1.5;
  }
  .bench-ratio {
    margin-left: auto;
    font-size: 0.75rem;
    padding: 0.125rem 0.5rem;
    border-radius: 999px;
  }
  .bench-ratio-ok {
    background: color-mix(in srgb, #10b981 18%, transparent);
    color: #10b981;
  }
  .bench-ratio-warn {
    background: color-mix(in srgb, #f59e0b 22%, transparent);
    color: #f59e0b;
  }
  .bench-ratio-bad {
    background: color-mix(in srgb, #ef4444 22%, transparent);
    color: #ef4444;
  }
  .bench-detail {
    margin-top: 0.25rem;
  }
  .bench-detail > summary {
    cursor: pointer;
    color: var(--ink-muted);
    font-size: 0.75rem;
    padding: 0.125rem 0;
  }
  .bench-table {
    width: 100%;
    margin-top: 0.5rem;
    font-size: 0.75rem;
    border-collapse: collapse;
  }
  .bench-table th,
  .bench-table td {
    text-align: left;
    padding: 0.25rem 0.5rem;
    border-bottom: 1px solid var(--border);
  }
  .bench-table th {
    color: var(--ink-muted);
    font-weight: 500;
  }
  .bench-preview {
    color: var(--ink-muted);
    max-width: 18rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bench-verdict {
    margin-top: 0.5rem;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    background: color-mix(in srgb, #ef4444 12%, transparent);
    border: 1px solid color-mix(in srgb, #ef4444 35%, transparent);
    color: color-mix(in srgb, #ef4444 95%, var(--ink));
    font-size: 0.8125rem;
    line-height: 1.5;
  }
  .detail-grid {
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
  }
  .detail-status {
    color: var(--ink-muted);
    font-size: 0.8125rem;
    line-height: 1.4;
  }
  .detail-row {
    display: flex;
    align-items: center;
    gap: 0.625rem;
  }
  .detail-label {
    flex: 0 0 8rem;
    font-size: 0.8125rem;
    color: var(--ink-muted);
  }
  .detail-row .input {
    flex: 1;
    min-width: 0;
  }
  .detail-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .detail-progress {
    font-size: 0.8125rem;
    color: var(--ink-muted);
  }
  .detail-error {
    color: #ef4444;
    font-size: 0.8125rem;
  }
  .detail-info {
    color: var(--accent);
    font-size: 0.8125rem;
  }
  .mono {
    font-family: var(--font-mono);
  }

  .settings {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .settings-grid {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 20px;
    height: 100%;
    min-height: 0;
  }
  .sub-nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow-y: auto;
    padding-right: 4px;
    min-height: 0;
  }
  .nav-group {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .nav-group + .nav-group {
    margin-top: 8px;
  }
  .nav-group-label {
    font-size: 10px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 600;
    padding: 8px 10px 4px;
  }
  .nav-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .settings-body {
    overflow: auto;
    padding: 0 4px 24px;
    min-width: 0;
  }
  .section-head {
    margin: 4px 0 16px;
  }
  .section-head h2 {
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.015em;
    margin: 0;
  }
  .lead {
    font-size: 12px;
    color: var(--ink-2);
    line-height: 1.6;
    margin: 4px 0 0;
    max-width: 720px;
  }

  .search-wrap {
    position: relative;
    width: 200px;
  }
  :global(.search-ico) {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--ink-3);
  }
  .search-input {
    padding-left: 30px;
  }

  .setting-card {
    padding: 0;
    margin-bottom: 14px;
    overflow: hidden;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 16px;
    align-items: center;
    padding: 11px 14px;
    border-bottom: 1px solid var(--line);
  }
  .row.stacked {
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
    align-items: stretch;
  }
  .row.last,
  .row:last-child {
    border-bottom: 0;
  }
  .row-text {
    min-width: 0;
  }
  .row-label {
    font-size: 12.5px;
    color: var(--ink-0);
    font-weight: 500;
  }
  .row-desc {
    font-size: 11px;
    color: var(--ink-2);
    margin-top: 2px;
    line-height: 1.5;
  }
  .kicker-row {
    padding: 10px 14px 4px;
    font-size: 10px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-weight: 600;
    display: flex;
    align-items: baseline;
    gap: 10px;
    border-bottom: 1px solid var(--line);
  }
  .kicker-desc {
    text-transform: none;
    letter-spacing: normal;
    font-weight: 400;
    color: var(--ink-2);
    font-size: 11px;
  }
  .kicker-row.mt-divider {
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    margin-top: 0;
  }

  .input.narrow {
    width: 200px;
  }
  .input.textarea {
    min-height: 60px;
    resize: vertical;
    font-family: var(--font-mono);
    font-size: 11.5px;
    line-height: 1.5;
  }

  .collapsible summary {
    cursor: pointer;
    list-style: none;
  }
  .collapsible summary::-webkit-details-marker {
    display: none;
  }
  .collapsible-head {
    border-bottom: 0;
  }
  details[open] .collapsible-head {
    border-bottom: 1px solid var(--line);
  }

  .swatch-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .swatch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px 5px 8px;
    border-radius: var(--r-pill);
    background: var(--bg-2);
    border: 1px solid var(--line);
    cursor: pointer;
    font-size: 11.5px;
    color: var(--ink-1);
    transition: border-color var(--dur-fast), background var(--dur-fast);
  }
  .swatch:hover {
    border-color: var(--ink-3);
  }
  .swatch.active {
    background: var(--bg-3);
    color: var(--ink-0);
    box-shadow: 0 0 0 1.5px var(--swatch) inset;
    border-color: transparent;
  }
  .swatch-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--swatch);
    box-shadow: 0 0 0 1px var(--line) inset;
  }
  .color-input {
    width: 32px;
    height: 28px;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    padding: 0;
    background: transparent;
    cursor: pointer;
  }

  .restart-banner {
    margin: 0 0 16px;
    padding: 14px 16px;
    border-radius: var(--r-md);
    background: rgba(224, 182, 110, 0.1);
    border: 1px solid rgba(224, 182, 110, 0.4);
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
    animation: ml-fadeup var(--dur-base) var(--ease-out) both;
  }
  .restart-icon {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(224, 182, 110, 0.18);
    color: #c69655;
    display: grid;
    place-items: center;
  }
  .restart-text {
    min-width: 0;
  }
  .restart-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-0);
    margin-bottom: 4px;
  }
  .restart-body {
    margin: 0;
    font-size: 11.5px;
    color: var(--ink-1);
    line-height: 1.6;
  }
  .restart-actions {
    flex: 0 0 auto;
  }
  .restart-error {
    margin: -8px 0 12px;
    font-size: 11.5px;
    color: #d87a7a;
  }
  :global(.spin) {
    animation: ml-spin 0.9s linear infinite;
  }

  .privacy-banner {
    margin-bottom: 14px;
    padding: 14px 16px;
    display: flex;
    gap: 14px;
    align-items: flex-start;
    border-color: var(--accent-soft);
    background: var(--accent-soft);
  }
  .banner-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-0);
    margin-bottom: 2px;
  }
  .banner-body {
    margin: 0;
    line-height: 1.6;
  }

  /* Provider preset cards */
  .preset-card {
    padding: 0;
    margin-bottom: 14px;
  }
  .preset-grid {
    padding: 12px 14px;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 8px;
  }
  .preset-btn {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    cursor: pointer;
    text-align: left;
    color: var(--ink-1);
    transition: border-color var(--dur-fast), background var(--dur-fast);
  }
  .preset-btn:hover {
    background: var(--bg-3);
    border-color: var(--ink-3);
    color: var(--ink-0);
  }
  .preset-btn :global(svg) {
    flex: 0 0 auto;
    margin-top: 2px;
    color: var(--accent);
  }
  .preset-title {
    font-size: 12.5px;
    font-weight: 550;
    color: var(--ink-0);
    margin-bottom: 2px;
  }
  .preset-desc {
    font-size: 11px;
    color: var(--ink-2);
    line-height: 1.4;
  }

  /* Provider sub-tabs */
  .provider-tabs {
    display: flex;
    gap: 2px;
    overflow-x: auto;
    padding: 4px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    margin-bottom: 14px;
  }
  .provider-tab {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: transparent;
    border: 0;
    border-radius: 5px;
    color: var(--ink-2);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background var(--dur-fast), color var(--dur-fast);
  }
  .provider-tab:hover {
    color: var(--ink-0);
  }
  .provider-tab.active {
    background: var(--bg-4);
    color: var(--ink-0);
  }
  .tab-count {
    display: inline-flex;
    align-items: center;
    padding: 1px 6px;
    border-radius: var(--r-pill);
    background: var(--accent-soft);
    color: var(--accent-strong);
    font-size: 10.5px;
    font-weight: 600;
    line-height: 1.4;
  }
  .tab-count.empty {
    background: transparent;
    color: var(--ink-3);
    font-weight: 400;
    font-style: italic;
  }
  :global(.scroll-x) {
    overflow-x: auto;
  }

  .chain-section {
    padding: 12px 14px;
  }
  .chain-empty-state {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 14px;
    border-radius: var(--r-sm);
    background: var(--accent-soft);
    margin-bottom: 12px;
  }
  .empty-title-line {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--ink-0);
    margin-bottom: 4px;
  }
  .chain-list {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .chain-row {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    background: var(--bg-2);
    border: 1px solid var(--line);
  }
  .chain-rank {
    font-size: 11px;
    color: var(--ink-3);
    text-align: center;
  }
  .chain-id-block {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 2px;
  }
  .chain-id {
    font-size: 12px;
    color: var(--ink-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chain-hint {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .kind-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 6px;
    border-radius: var(--r-pill);
    font-size: 10px;
    font-weight: 600;
    line-height: 1.5;
  }
  .kind-pill.compact {
    padding: 0 5px;
    font-size: 9.5px;
  }
  .kind-local {
    background: rgba(91, 185, 125, 0.15);
    color: #5bb97d;
  }
  .kind-external {
    background: rgba(224, 182, 110, 0.15);
    color: #c69655;
  }
  .kind-cloud {
    background: rgba(168, 197, 218, 0.18);
    color: var(--accent-strong);
  }
  .chain-actions {
    display: flex;
    gap: 4px;
  }
  .chain-btn {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: var(--r-sm);
    background: transparent;
    border: 0;
    color: var(--ink-2);
    cursor: pointer;
    transition: background var(--dur-fast), color var(--dur-fast);
  }
  .chain-btn:hover:not(:disabled) {
    background: var(--bg-3);
    color: var(--ink-0);
  }
  .chain-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
  .chain-btn.danger:hover:not(:disabled) {
    color: #d87a7a;
  }
  .suggestion-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    margin-bottom: 10px;
  }
  .suggestion-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: var(--r-pill);
    background: var(--bg-2);
    border: 1px dashed var(--line-strong);
    color: var(--ink-1);
    font-size: 11.5px;
    cursor: pointer;
    transition: background var(--dur-fast), border-color var(--dur-fast);
  }
  .suggestion-chip:hover {
    background: var(--bg-3);
    border-color: var(--ink-3);
    color: var(--ink-0);
  }
  .chain-add {
    display: flex;
    gap: 8px;
  }
  .chain-add .input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 11.5px;
  }

  .fallback-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: var(--r-pill);
    background: var(--bg-2);
    border: 1px solid var(--line);
    font-size: 11.5px;
    color: var(--ink-1);
    cursor: pointer;
  }
  .chip input {
    margin: 0;
  }
  .chip.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
    border-color: transparent;
  }

  :global(.accent-text) {
    color: var(--accent);
  }
  :global(.inline-link) {
    color: var(--accent-strong);
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-thickness: 1px;
  }

  .about {
    padding: 28px;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    max-width: 480px;
    margin: 0 auto;
  }
  .about-name {
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .about-desc {
    font-size: 13px;
    color: var(--ink-1);
    max-width: 380px;
    line-height: 1.6;
    margin: 0;
  }
  .about-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
    flex-wrap: wrap;
    justify-content: center;
  }

  .small {
    font-size: 11px;
  }
  .muted {
    color: var(--ink-2);
  }
  .error {
    color: #d87a7a;
    padding: 12px;
    border-radius: var(--r-sm);
    background: rgba(216, 122, 122, 0.1);
    border: 1px solid rgba(216, 122, 122, 0.3);
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(0, 0, 0, 0.4);
  }
  .modal {
    width: 100%;
    max-width: 360px;
  }
  .modal h2 {
    font-size: 14px;
    font-weight: 600;
    margin: 0 0 4px;
  }
  .modal-actions {
    margin-top: 16px;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
