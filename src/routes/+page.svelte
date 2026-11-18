<script lang="ts">
  import { goto } from '$app/navigation';
  import { onDestroy, onMount } from 'svelte';
  import { AlertCircle, AlertTriangle, Camera, Focus, Maximize, Mic, Square, X } from 'lucide-svelte';
  import {
    dismissNotice,
    getSettings,
    listLlmModels,
    listNotices,
    newLensWindow,
    onNotices,
    ocrModelsStatus,
    onHotkey,
    onSettingsChanged,
    onSubtitle,
    startRegionSelection,
    startSubtitle,
    stopSubtitle,
    subtitleState
  } from '$lib/ipc';
  import { displayAccelerator } from '$lib/hotkey';
  import { languageLabel } from '$lib/lens-overlay';
  import { errorText } from '$lib/errors';
  import type { Notice } from '$lib/ipc';

  let lastError = $state<string | null>(null);
  let subtitleRunning = $state(false);
  let subtitlePreview = $state<string | null>(null);
  let hotkeys = $state<Record<string, string>>({});
  let targetLanguage = $state('ja');
  /** What still has to be downloaded before translation works. */
  let missing = $state<string[]>([]);
  /** Problems found at start-up or in the background (config, hotkeys). */
  let notices = $state<Notice[]>([]);
  const unlisteners: (() => void)[] = [];

  async function checkSetup() {
    try {
      const [ocr, llm] = await Promise.all([ocrModelsStatus(), listLlmModels()]);
      const out: string[] = [];
      if (!llm.models.find((m) => m.id === llm.selected)?.installed) out.push('翻訳モデル');
      if (!ocr.ppocr) out.push('OCR モデル');
      missing = out;
    } catch {
      missing = [];
    }
  }

  function applySettings(s: any) {
    hotkeys = s?.hotkeys ?? {};
    targetLanguage = s?.subtitle?.target_language ?? 'ja';
  }

  onMount(async () => {
    getSettings().then(applySettings).catch(() => {});
    checkSetup();
    listNotices()
      .then((n) => (notices = n))
      .catch(() => {});
    try {
      unlisteners.push(await onNotices((n) => (notices = n)));
      unlisteners.push(await onSettingsChanged(applySettings));
      unlisteners.push(
        await onHotkey((action) => {
          if (action === 'toggle_subtitle_failed')
            lastError = 'ライブ字幕の開始に失敗。';
          if (action === 'toggle_pin_overlay_empty')
            lastError = 'ピン留め対象なし。範囲翻訳を先に実行。';
        })
      );
      unlisteners.push(
        await onSubtitle((event) => {
          if (event.kind === 'started') subtitleRunning = true;
          else if (event.kind === 'stopped') subtitleRunning = false;
          else if (event.kind === 'chunk') subtitlePreview = event.translated || event.original;
          else if (event.kind === 'error') lastError = `ライブ字幕：${event.message}`;
        })
      );
      subtitleRunning = await subtitleState();
    } catch {
      /* pure frontend dev */
    }
  });
  onDestroy(() => unlisteners.forEach((u) => u()));

  async function attempt(fn: () => Promise<unknown>) {
    lastError = null;
    try {
      await fn();
    } catch (e) {
      lastError = errorText(e);
    }
  }

  const key = (name: string) => (hotkeys[name] ? displayAccelerator(hotkeys[name]) : '');
</script>

<main class="mx-auto flex max-w-3xl flex-col gap-4 px-4 py-6 sm:gap-5 sm:px-6">
  <header>
    <h1 class="text-xl font-semibold tracking-tight">ホーム</h1>
    <p class="text-sm text-ink-muted">
      全処理ローカル実行／翻訳先：{languageLabel(targetLanguage) || targetLanguage}
      <a href="/settings/" class="ml-1 text-accent underline-offset-2 hover:underline">変更</a>
    </p>
  </header>

  {#each notices as n (n.id)}
    <section
      class="flex items-start gap-2 rounded-ml border border-amber-500/50 bg-amber-500/10 p-3 text-sm"
      role="alert"
    >
      <AlertTriangle class="mt-0.5 h-4 w-4 shrink-0 text-amber-600" />
      <p class="min-w-0 flex-1 break-words">
        {n.message}
        {#if n.action}
          <a href={n.action} class="ml-1 text-accent underline-offset-2 hover:underline">設定を開く</a>
        {/if}
      </p>
      <button
        class="shrink-0 rounded-ml-sm p-0.5 text-ink-faint hover:text-ink"
        title="閉じる"
        aria-label="通知を閉じる"
        onclick={() => dismissNotice(n.id).catch(() => {})}
      >
        <X class="h-4 w-4" />
      </button>
    </section>
  {/each}

  {#if missing.length > 0}
    <section
      class="flex flex-col gap-3 rounded-ml border border-accent/40 bg-accent-soft p-4 sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="flex min-w-0 gap-2">
        <AlertCircle class="mt-0.5 h-4 w-4 shrink-0 text-accent" />
        <p class="text-sm">
          初回セットアップ：{missing.join('・')}のダウンロードが必要。
        </p>
      </div>
      <button
        class="shrink-0 self-start rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white sm:self-auto"
        onclick={() => goto('/settings/#section-models')}
      >
        モデル準備
      </button>
    </section>
  {/if}

  <section class="rounded-ml border border-border bg-surface-elevated p-4 shadow-ml sm:p-5">
    <h2 class="text-base font-semibold">画面翻訳</h2>
    <p class="text-sm text-ink-muted">
      画面を静止し、原文位置に訳文をオーバーレイ表示。全アプリでショートカット起動可能。
    </p>
    <div class="mt-4 grid gap-2 sm:grid-cols-2">
      <button
        class="flex items-center justify-between gap-3 rounded-ml-sm bg-accent px-4 py-2.5 text-left text-sm font-medium text-white shadow-ml transition hover:opacity-90"
        onclick={() => attempt(() => startRegionSelection(false))}
      >
        <span class="flex items-center gap-2"><Camera class="h-4 w-4 shrink-0" />範囲翻訳</span>
        {#if key('capture_region')}<kbd class="font-mono text-xs opacity-80">{key('capture_region')}</kbd>{/if}
      </button>
      <button
        class="flex items-center justify-between gap-3 rounded-ml-sm border border-border bg-surface px-4 py-2.5 text-left text-sm font-medium transition hover:border-accent/60"
        onclick={() => attempt(() => startRegionSelection(true))}
      >
        <span class="flex items-center gap-2"><Maximize class="h-4 w-4 shrink-0" />全画面翻訳</span>
        {#if key('capture_window')}<kbd class="font-mono text-xs text-ink-faint">{key('capture_window')}</kbd>{/if}
      </button>
    </div>
    <div class="mt-3 flex flex-col gap-2 border-t border-border pt-3 sm:flex-row sm:items-center sm:justify-between">
      <p class="text-xs text-ink-muted">
        Lens ウィンドウ：動画字幕・ゲーム画面など定位置の継続翻訳用。
      </p>
      <button
        class="inline-flex shrink-0 items-center gap-2 self-start rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted transition hover:border-accent/60 hover:text-ink sm:self-auto"
        onclick={() => attempt(newLensWindow)}
      >
        <Focus class="h-4 w-4" />
        Lens ウィンドウ
      </button>
    </div>
  </section>

  <section class="rounded-ml border border-border bg-surface-elevated p-4 shadow-ml sm:p-5">
    <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
      <div class="min-w-0">
        <h2 class="flex flex-wrap items-center gap-2 text-base font-semibold">
          ライブ字幕
          {#if subtitleRunning}
            <span
              class="inline-flex items-center gap-1 rounded-full bg-accent-soft px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-accent"
            >
              <span class="block h-1.5 w-1.5 animate-pulse rounded-full bg-accent" aria-hidden="true"></span>
              ON AIR
            </span>
          {/if}
        </h2>
        <p class="text-sm text-ink-muted">システム音声を認識・翻訳し、字幕表示。</p>
        {#if subtitlePreview}
          <p class="mt-2 line-clamp-2 text-xs break-words text-ink-faint">{subtitlePreview}</p>
        {/if}
      </div>
      {#if subtitleRunning}
        <button
          class="inline-flex shrink-0 items-center gap-2 self-start rounded-ml-sm border border-red-500/60 bg-red-500/10 px-4 py-2 text-sm font-medium text-red-500"
          onclick={() =>
            attempt(async () => {
              await stopSubtitle();
              subtitleRunning = false;
              subtitlePreview = null;
            })}
        >
          <Square class="h-4 w-4" /> 停止
        </button>
      {:else}
        <button
          class="inline-flex shrink-0 items-center gap-2 self-start rounded-ml-sm border border-border bg-surface px-4 py-2 text-sm font-medium transition hover:border-accent/60"
          onclick={() =>
            attempt(async () => {
              await startSubtitle();
              subtitleRunning = true;
            })}
        >
          <Mic class="h-4 w-4" /> 開始
          {#if key('toggle_subtitle')}<kbd class="font-mono text-xs text-ink-faint">{key('toggle_subtitle')}</kbd>{/if}
        </button>
      {/if}
    </div>
  </section>

  {#if lastError}
    <p class="rounded-ml border border-red-500/50 bg-red-500/10 p-3 text-sm break-words text-red-500" role="alert">
      {lastError}
    </p>
  {/if}

  <p class="text-xs text-ink-faint">
    {#if key('open_command_palette')}<kbd class="font-mono">{key('open_command_palette')}</kbd>：コマンドパレット　{/if}ショートカット変更：<a href="/settings/#section-hotkeys" class="underline-offset-2 hover:underline">設定</a>
  </p>
</main>
