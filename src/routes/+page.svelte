<script lang="ts">
  import { goto } from '$app/navigation';
  import { onDestroy, onMount } from 'svelte';
  import {
    Camera,
    Keyboard,
    Puzzle,
    History,
    Download,
    Settings as SettingsIcon
  } from 'lucide-svelte';
  import { captureAndProcess, installOcrModels, onHotkey } from '$lib/ipc';

  let running = $state(false);
  let lastError = $state<string | null>(null);
  let lastProvider = $state<string | null>(null);
  let installing = $state(false);
  let modelsPath = $state<string | null>(null);
  let unlisten: (() => void) | undefined;

  onMount(async () => {
    unlisten = await onHotkey((action) => {
      if (action === 'open_history') goto('/history/');
      if (action === 'open_command_palette') goto('/settings/');
    });
  });
  onDestroy(() => unlisten?.());

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

  <section
    class="rounded-ml border border-border bg-surface-elevated p-5 shadow-ml"
  >
    <div class="flex items-center justify-between gap-4">
      <div>
        <h2 class="text-base font-semibold">キャプチャ</h2>
        <p class="text-sm text-ink-muted">プライマリモニタ全体を読み取り、OCR＋翻訳を実行します。</p>
      </div>
      <button
        class="inline-flex items-center gap-2 rounded-ml-sm bg-accent px-4 py-2 text-sm font-medium text-white shadow-ml transition hover:opacity-90 disabled:opacity-40"
        onclick={capture}
        disabled={running}
      >
        <Camera class="h-4 w-4" />
        {running ? '処理中…' : 'いますぐキャプチャ'}
      </button>
    </div>
    {#if lastProvider}
      <p class="mt-3 text-xs text-ink-faint">前回のプロバイダ: <code class="font-mono">{lastProvider}</code></p>
    {/if}
    {#if lastError}
      <p class="mt-3 text-xs text-red-500">エラー: {lastError}</p>
    {/if}
  </section>

  <section
    class="rounded-ml border border-border bg-surface-elevated p-5 shadow-ml"
  >
    <div class="flex items-center justify-between gap-4">
      <div>
        <h2 class="text-base font-semibold">OCR モデル</h2>
        <p class="text-sm text-ink-muted">
          初回のみ ocrs 用 text-detection / text-recognition モデル (≈100 MB) をダウンロードします。
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
