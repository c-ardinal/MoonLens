<script lang="ts">
  import { goto } from '$app/navigation';
  import { onDestroy, onMount } from 'svelte';
  import { AlertCircle, Camera, Focus, Maximize, Mic, Square } from 'lucide-svelte';
  import {
    getSettings,
    listLlmModels,
    newLensWindow,
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

  let lastError = $state<string | null>(null);
  let subtitleRunning = $state(false);
  let subtitlePreview = $state<string | null>(null);
  let hotkeys = $state<Record<string, string>>({});
  let targetLanguage = $state('ja');
  /** What still has to be downloaded before translation works. */
  let missing = $state<string[]>([]);
  const unlisteners: (() => void)[] = [];

  async function checkSetup() {
    try {
      const [ocr, llm] = await Promise.all([ocrModelsStatus(), listLlmModels()]);
      const out: string[] = [];
      if (!llm.models.find((m) => m.id === llm.selected)?.installed) out.push('翻訳モデル');
      if (!ocr.ppocr) out.push('文字認識（OCR）モデル');
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
    try {
      unlisteners.push(await onSettingsChanged(applySettings));
      unlisteners.push(
        await onHotkey((action) => {
          if (action === 'toggle_subtitle_failed')
            lastError = 'ライブ字幕を開始できませんでした。';
          if (action === 'toggle_pin_overlay_empty')
            lastError = 'ピン留めする範囲がありません。先に範囲を選んで翻訳してください。';
        })
      );
      unlisteners.push(
        await onSubtitle((event) => {
          if (event.kind === 'started') subtitleRunning = true;
          else if (event.kind === 'stopped') subtitleRunning = false;
          else if (event.kind === 'chunk') subtitlePreview = event.translated || event.original;
          else if (event.kind === 'error') lastError = `字幕エラー: ${event.message}`;
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
      lastError = String(e).replace(/^Error:\s*/, '');
    }
  }

  const key = (name: string) => (hotkeys[name] ? displayAccelerator(hotkeys[name]) : '');
</script>

<main class="mx-auto flex max-w-3xl flex-col gap-4 px-4 py-6 sm:gap-5 sm:px-6">
  <header>
    <h1 class="text-xl font-semibold tracking-tight">画面をその場で翻訳</h1>
    <p class="text-sm text-ink-muted">
      すべてこの端末の中で処理します。翻訳先: {languageLabel(targetLanguage) || targetLanguage}
      <a href="/settings/" class="ml-1 text-accent underline-offset-2 hover:underline">変更</a>
    </p>
  </header>

  {#if missing.length > 0}
    <section
      class="flex flex-col gap-3 rounded-ml border border-accent/40 bg-accent-soft p-4 sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="flex min-w-0 gap-2">
        <AlertCircle class="mt-0.5 h-4 w-4 shrink-0 text-accent" />
        <p class="text-sm">
          はじめに{missing.join('と')}のダウンロードが必要です。
        </p>
      </div>
      <button
        class="shrink-0 self-start rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white sm:self-auto"
        onclick={() => goto('/settings/#section-models')}
      >
        モデルを準備する
      </button>
    </section>
  {/if}

  <section class="rounded-ml border border-border bg-surface-elevated p-4 shadow-ml sm:p-5">
    <h2 class="text-base font-semibold">画面を翻訳</h2>
    <p class="text-sm text-ink-muted">
      画面を止めて、文字の上に訳文を重ねて表示します。どのアプリからでもショートカットで呼び出せます。
    </p>
    <div class="mt-4 grid gap-2 sm:grid-cols-2">
      <button
        class="flex items-center justify-between gap-3 rounded-ml-sm bg-accent px-4 py-2.5 text-left text-sm font-medium text-white shadow-ml transition hover:opacity-90"
        onclick={() => attempt(() => startRegionSelection(false))}
      >
        <span class="flex items-center gap-2"><Camera class="h-4 w-4 shrink-0" />範囲を選んで翻訳</span>
        {#if key('capture_region')}<kbd class="font-mono text-xs opacity-80">{key('capture_region')}</kbd>{/if}
      </button>
      <button
        class="flex items-center justify-between gap-3 rounded-ml-sm border border-border bg-surface px-4 py-2.5 text-left text-sm font-medium transition hover:border-accent/60"
        onclick={() => attempt(() => startRegionSelection(true))}
      >
        <span class="flex items-center gap-2"><Maximize class="h-4 w-4 shrink-0" />画面全体を翻訳</span>
        {#if key('capture_window')}<kbd class="font-mono text-xs text-ink-faint">{key('capture_window')}</kbd>{/if}
      </button>
    </div>
    <div class="mt-3 flex flex-col gap-2 border-t border-border pt-3 sm:flex-row sm:items-center sm:justify-between">
      <p class="text-xs text-ink-muted">
        Lens ウィンドウ: 動画の字幕やゲーム画面など、同じ場所を続けて翻訳したいときに。
      </p>
      <button
        class="inline-flex shrink-0 items-center gap-2 self-start rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted transition hover:border-accent/60 hover:text-ink sm:self-auto"
        onclick={() => attempt(newLensWindow)}
      >
        <Focus class="h-4 w-4" />
        Lens ウィンドウを開く
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
        <p class="text-sm text-ink-muted">パソコンで再生中の音声を文字起こしして、翻訳を字幕で表示します。</p>
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
    {#if key('open_command_palette')}<kbd class="font-mono">{key('open_command_palette')}</kbd> でコマンドパレット。{/if}
    ショートカットは<a href="/settings/#section-hotkeys" class="underline-offset-2 hover:underline">設定</a>で変更できます。
  </p>
</main>
