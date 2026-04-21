<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    Camera,
    Focus,
    Download,
    Mic,
    MicOff,
    KeyboardIcon,
    Sparkles
  } from 'lucide-svelte';
  import {
    captureAndProcess,
    installOcrModels,
    newLensWindow,
    onHotkey,
    onModelProgress,
    onSubtitle,
    startSubtitle,
    stopSubtitle,
    subtitleState,
    type ModelProgress
  } from '$lib/ipc';
  import { goto } from '$lib/navigation';

  let running = $state(false);
  let lastError = $state<string | null>(null);
  let lastProvider = $state<string | null>(null);
  let installing = $state(false);
  let modelsPath = $state<string | null>(null);
  let modelProgress = $state<ModelProgress | null>(null);
  let unlistenProgress: (() => void) | undefined;
  let subtitleRunning = $state(false);
  let subtitlePreview = $state<string | null>(null);
  let unlisten: (() => void) | undefined;
  let unlistenSubtitle: (() => void) | undefined;

  onMount(async () => {
    unlisten = await onHotkey((action) => {
      if (action === 'open_history') goto('/history/');
      if (action === 'open_command_palette') goto('/settings/');
    });
    unlistenSubtitle = await onSubtitle((event) => {
      if (event.kind === 'started') subtitleRunning = true;
      else if (event.kind === 'stopped') subtitleRunning = false;
      else if (event.kind === 'chunk') subtitlePreview = event.translated || event.original;
      else if (event.kind === 'error') lastError = `字幕エラー: ${event.message}`;
    });
    subtitleRunning = await subtitleState();
    unlistenProgress = await onModelProgress((p) => {
      modelProgress = p;
    });
  });

  onDestroy(() => {
    unlisten?.();
    unlistenSubtitle?.();
    unlistenProgress?.();
  });

  async function startSub() {
    try {
      await startSubtitle();
      subtitleRunning = true;
    } catch (e) {
      lastError = String(e);
    }
  }

  async function stopSub() {
    try {
      await stopSubtitle();
      subtitleRunning = false;
      subtitlePreview = null;
    } catch (e) {
      lastError = String(e);
    }
  }

  async function capture() {
    running = true;
    lastError = null;
    try {
      const res = await captureAndProcess();
      lastProvider = res.result.used_provider;
    } catch (e) {
      lastError = String(e);
    } finally {
      running = false;
    }
  }

  async function createLens() {
    try {
      await newLensWindow();
    } catch (e) {
      lastError = String(e);
    }
  }

  async function setupModels() {
    installing = true;
    try {
      modelsPath = await installOcrModels();
    } catch (e) {
      lastError = String(e);
    } finally {
      installing = false;
    }
  }

  const shortcuts = [
    { keys: '⌘⇧Space', desc: '領域キャプチャ' },
    { keys: '⌘⇧L', desc: 'Lens ウィンドウ' },
    { keys: '⌘⇧S', desc: 'ライブ字幕' },
    { keys: '⌘K', desc: 'コマンドパレット' },
    { keys: '⌘⇧H', desc: '履歴' }
  ];
</script>

<div class="mx-auto flex max-w-4xl flex-col gap-6 px-8 py-6">
  <header class="flex items-baseline justify-between">
    <div>
      <h1 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
        <Sparkles class="h-5 w-5 text-accent" aria-hidden="true" />
        MoonLens
      </h1>
      <p class="text-sm text-ink-muted">
        画面の外国語テキストを、その場で読み替える Lens 型ツール。
      </p>
    </div>
    {#if lastProvider}
      <p class="text-xs text-ink-faint">
        前回: <code class="font-mono">{lastProvider}</code>
      </p>
    {/if}
  </header>

  <section class="grid grid-cols-2 gap-3">
    <button
      onclick={capture}
      disabled={running}
      class="group flex items-start gap-3 rounded-ml border border-border bg-surface-elevated p-4 text-left shadow-ml transition-colors hover:border-accent/60 disabled:opacity-50"
    >
      <div
        class="flex h-9 w-9 shrink-0 items-center justify-center rounded-ml-sm bg-accent-soft text-accent transition-colors group-hover:bg-accent group-hover:text-white"
      >
        <Camera class="h-4 w-4" aria-hidden="true" />
      </div>
      <div class="min-w-0">
        <p class="flex items-center gap-2 text-sm font-semibold">
          領域キャプチャ
          <kbd class="font-mono text-[10px] text-ink-faint">⌘⇧Space</kbd>
        </p>
        <p class="mt-0.5 text-xs text-ink-muted">
          {running ? '処理中…' : 'ドラッグで選択 → OCR + 翻訳'}
        </p>
      </div>
    </button>

    <button
      onclick={createLens}
      class="group flex items-start gap-3 rounded-ml border border-border bg-surface-elevated p-4 text-left shadow-ml transition-colors hover:border-accent/60"
    >
      <div
        class="flex h-9 w-9 shrink-0 items-center justify-center rounded-ml-sm bg-accent-soft text-accent transition-colors group-hover:bg-accent group-hover:text-white"
      >
        <Focus class="h-4 w-4" aria-hidden="true" />
      </div>
      <div class="min-w-0">
        <p class="flex items-center gap-2 text-sm font-semibold">
          Lens ウィンドウ
          <kbd class="font-mono text-[10px] text-ink-faint">⌘⇧L</kbd>
        </p>
        <p class="mt-0.5 text-xs text-ink-muted">領域を常駐監視して、その場で翻訳</p>
      </div>
    </button>
  </section>

  <section class="rounded-ml border border-border bg-surface-elevated p-5 shadow-ml">
    <div class="flex items-center justify-between gap-4">
      <div class="min-w-0">
        <h2 class="flex items-center gap-2 text-base font-semibold">
          ライブ字幕
          {#if subtitleRunning}
            <span
              class="inline-flex items-center gap-1 rounded-full bg-accent-soft px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-accent"
            >
              <span
                class="block h-1.5 w-1.5 animate-pulse rounded-full bg-accent"
                aria-hidden="true"
              ></span>
              ON AIR
            </span>
          {/if}
        </h2>
        <p class="text-sm text-ink-muted">
          システム音声 (ループバック) を書き起こし → 翻訳してオーバーレイ表示。
        </p>
        {#if subtitlePreview}
          <p class="mt-2 line-clamp-2 font-mono text-xs text-ink-faint">
            {subtitlePreview}
          </p>
        {/if}
      </div>
      {#if subtitleRunning}
        <button
          class="inline-flex items-center gap-2 rounded-ml-sm border border-red-500/60 bg-red-500/10 px-4 py-2 text-sm font-medium text-red-500 transition-colors hover:bg-red-500/20"
          onclick={stopSub}
        >
          <MicOff class="h-4 w-4" aria-hidden="true" />
          停止
        </button>
      {:else}
        <button
          class="inline-flex items-center gap-2 rounded-ml-sm border border-border bg-surface px-4 py-2 text-sm font-medium text-ink-muted transition-colors hover:border-accent/60 hover:text-ink"
          onclick={startSub}
        >
          <Mic class="h-4 w-4" aria-hidden="true" />
          開始
        </button>
      {/if}
    </div>
  </section>

  <section class="rounded-ml border border-border bg-surface-elevated p-5 shadow-ml">
    <div class="flex items-center justify-between gap-4">
      <div class="min-w-0">
        <h2 class="text-base font-semibold">OCR モデル</h2>
        <p class="text-sm text-ink-muted">
          初回のみ ocrs 用 text-detection / text-recognition モデル (≈100 MB) をダウンロードします。
        </p>
      </div>
      <button
        class="inline-flex items-center gap-2 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted transition-colors hover:border-accent/60 hover:text-ink disabled:opacity-40"
        onclick={setupModels}
        disabled={installing}
      >
        <Download class="h-4 w-4" aria-hidden="true" />
        {installing ? '取得中…' : 'モデル取得'}
      </button>
    </div>
    {#if modelProgress}
      <div class="mt-3">
        <div class="flex items-center justify-between text-xs text-ink-faint">
          <span class="font-mono">{modelProgress.filename}</span>
          <span>
            {(modelProgress.downloaded / 1_048_576).toFixed(1)} MB
            {#if modelProgress.total}
              / {(modelProgress.total / 1_048_576).toFixed(1)} MB
            {/if}
          </span>
        </div>
        <div class="mt-1 h-1.5 overflow-hidden rounded-full bg-surface-muted">
          <div
            class="h-full rounded-full bg-accent transition-[width]"
            style="width: {((modelProgress.fraction ?? 0) * 100).toFixed(1)}%"
          ></div>
        </div>
      </div>
    {/if}
    {#if modelsPath}
      <p class="mt-3 text-xs text-ink-faint">
        インストール先: <code class="font-mono">{modelsPath}</code>
      </p>
    {/if}
  </section>

  <section class="rounded-ml border border-border/60 bg-surface p-4">
    <h3 class="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-ink-faint">
      <KeyboardIcon class="h-3 w-3" aria-hidden="true" />
      ショートカット
    </h3>
    <dl class="grid grid-cols-2 gap-x-6 gap-y-2 text-xs">
      {#each shortcuts as s}
        <div class="flex items-center justify-between">
          <dd class="text-ink-muted">{s.desc}</dd>
          <dt>
            <kbd class="font-mono text-ink-faint">{s.keys}</kbd>
          </dt>
        </div>
      {/each}
    </dl>
  </section>

  {#if lastError}
    <p class="rounded-ml-sm border border-red-500/40 bg-red-500/10 p-2 text-xs text-red-500">
      エラー: {lastError}
    </p>
  {/if}
</div>
