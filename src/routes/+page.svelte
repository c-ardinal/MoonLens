<script lang="ts">
  import { goto } from '$app/navigation';
  import { onDestroy, onMount } from 'svelte';
  import {
    Camera,
    Focus,
    Keyboard,
    Puzzle,
    History,
    Download,
    Settings as SettingsIcon
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

  const nav = [
    { href: '/settings/', label: '設定', icon: SettingsIcon, hint: '⌘,' },
    { href: '/plugins/', label: 'プラグイン', icon: Puzzle, hint: '' },
    { href: '/history/', label: '履歴', icon: History, hint: '⌘⇧H' },
    { href: '/subtitle/', label: '字幕', icon: Keyboard, hint: '⌘⇧S' }
  ];
</script>

<main class="mx-auto flex max-w-3xl flex-col gap-6 p-8">
  <header class="flex items-baseline justify-between">
    <h1 class="text-2xl font-semibold tracking-tight">MoonLens</h1>
    <p class="text-sm text-ink-muted">Lens + 字幕 + 検索、すべて1つのホットキーで。</p>
  </header>

  <section class="rounded-ml border border-border bg-surface-elevated p-5 shadow-ml">
    <div class="flex items-center justify-between gap-4">
      <div>
        <h2 class="text-base font-semibold">キャプチャ</h2>
        <p class="text-sm text-ink-muted">
          ワンショットは <code class="font-mono">⌘⇧Space</code> でドラッグ選択。<br />
          Lens ウィンドウは領域を常駐させて継続監視（動画字幕・ゲームUIに最適）。
        </p>
      </div>
      <div class="flex flex-col gap-2">
        <button
          class="inline-flex items-center gap-2 rounded-ml-sm bg-accent px-4 py-2 text-sm font-medium text-white shadow-ml transition hover:opacity-90 disabled:opacity-40"
          onclick={capture}
          disabled={running}
        >
          <Camera class="h-4 w-4" />
          {running ? '処理中…' : '全画面キャプチャ'}
        </button>
        <button
          class="inline-flex items-center gap-2 rounded-ml-sm border border-border bg-surface px-4 py-2 text-sm font-medium text-ink-muted transition hover:border-accent/60 hover:text-ink"
          onclick={createLens}
        >
          <Focus class="h-4 w-4" />
          Lens ウィンドウを開く
        </button>
      </div>
    </div>
    {#if lastProvider}
      <p class="mt-3 text-xs text-ink-faint">
        前回のプロバイダ: <code class="font-mono">{lastProvider}</code>
      </p>
    {/if}
    {#if lastError}
      <p class="mt-3 text-xs text-red-500">エラー: {lastError}</p>
    {/if}
  </section>

  <section class="rounded-ml border border-border bg-surface-elevated p-5 shadow-ml">
    <div class="flex items-center justify-between gap-4">
      <div>
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
          システム音声 (ループバック) を書き起こし → 翻訳してオーバーレイ表示します。
        </p>
        {#if subtitlePreview}
          <p class="mt-2 line-clamp-2 font-mono text-xs text-ink-faint">
            {subtitlePreview}
          </p>
        {/if}
      </div>
      <div class="flex flex-col gap-2">
        {#if subtitleRunning}
          <button
            class="inline-flex items-center gap-2 rounded-ml-sm border border-red-500/60 bg-red-500/10 px-4 py-2 text-sm font-medium text-red-500"
            onclick={stopSub}
          >
            停止
          </button>
        {:else}
          <button
            class="inline-flex items-center gap-2 rounded-ml-sm border border-border bg-surface px-4 py-2 text-sm font-medium text-ink-muted hover:border-accent/60 hover:text-ink"
            onclick={startSub}
          >
            開始 (⌘⇧S)
          </button>
        {/if}
      </div>
    </div>
  </section>

  <section
    class="rounded-ml border border-border bg-surface-elevated p-5 shadow-ml"
  >
    <div class="flex items-center justify-between gap-4">
      <div>
        <h2 class="text-base font-semibold">OCR モデル</h2>
        <p class="text-sm text-ink-muted">
          初回のみ OCR モデルをダウンロードします（日本語・中国語・英語用 PP-OCRv5 約 26 MB ＋ 英語用 ocrs 約 12 MB）。
        </p>
      </div>
      <button
        class="inline-flex items-center gap-2 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted transition hover:border-accent/50 hover:text-ink disabled:opacity-40"
        onclick={setupModels}
        disabled={installing}
      >
        <Download class="h-4 w-4" />
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

  <section class="grid grid-cols-2 gap-3">
    {#each nav as item}
      <a
        href={item.href}
        class="group flex items-center justify-between rounded-ml border border-border bg-surface-elevated p-4 shadow-ml transition hover:border-accent/40"
      >
        <span class="flex items-center gap-3">
          <item.icon class="h-5 w-5 text-ink-muted group-hover:text-accent" />
          <span class="font-medium">{item.label}</span>
        </span>
        {#if item.hint}
          <kbd class="font-mono text-xs text-ink-faint">{item.hint}</kbd>
        {/if}
      </a>
    {/each}
  </section>

  <footer class="text-xs text-ink-faint">
    キャプチャ: ⌘⇧Space / ピン留め: ⌘⇧P / 字幕: ⌘⇧S / 検索: ⌘⇧G
  </footer>
</main>
