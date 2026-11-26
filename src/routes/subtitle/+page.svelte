<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { Eye, EyeOff, X } from 'lucide-svelte';
  import {
    getSettings,
    onSettingsChanged,
    onSubtitle,
    stopSubtitle,
    subtitleState,
    type Settings
  } from '$lib/ipc';
  import { errorText } from '$lib/errors';
  import { languageLabel } from '$lib/lens-overlay';
  import { initialView, reduce, statusLabel } from '$lib/subtitle';

  // 音声翻訳ウィンドウ。タイトルバーのドラッグで移動、右下隅で寸法変更、
  // × で音声翻訳を停止。字幕は時系列で蓄積し、最新行へ自動スクロール。

  let view = $state(initialView());
  const status = $derived(statusLabel(view));
  const busy = $derived(view.running && view.phase !== 'waiting');
  // 設定 → 音声翻訳
  let showOriginal = $state(true);
  let fontSize = $state(22);
  let targetLanguage = $state('ja');
  let scroller: HTMLElement;
  const unlisteners: (UnlistenFn | undefined)[] = [];

  function applySettings(s: Settings) {
    showOriginal = s?.subtitle?.show_original ?? true;
    fontSize = s?.subtitle?.font_size_px ?? 22;
    targetLanguage = s?.subtitle?.target_language ?? 'ja';
  }

  function close() {
    stopSubtitle().catch(() => {});
  }

  function startResize(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    getCurrentWebviewWindow().startResizeDragging('SouthEast').catch(() => {});
  }

  // 最新の字幕・エラーを表示範囲へ。
  $effect(() => {
    void view.lines.length;
    void view.error;
    tick().then(() => scroller?.scrollTo({ top: scroller.scrollHeight }));
  });

  onMount(async () => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
    };
    window.addEventListener('keydown', onKey);
    unlisteners.push(() => window.removeEventListener('keydown', onKey));
    getSettings().then(applySettings).catch(() => {});
    try {
      unlisteners.push(await onSettingsChanged(applySettings));
      unlisteners.push(await onSubtitle((ev) => (view = reduce(view, ev))));
      // 再読込時：開始イベントは受信済みのため、現況を照会。
      if (!view.running && (await subtitleState())) view = { ...view, running: true };
    } catch {
      /* not running inside Tauri */
    }
  });
  onDestroy(() => unlisteners.forEach((u) => u?.()));
</script>

<main class="sub">
  <header class="bar" data-tauri-drag-region>
    <span class="title" data-tauri-drag-region>音声翻訳</span>
    <span
      class="state"
      class:busy
      class:error={!!view.error}
      title={view.device ? `音声取得：${view.device}` : undefined}
      data-tauri-drag-region
    >
      {status}
    </span>
    <span class="fill" data-tauri-drag-region></span>
    <span class="lang" title="翻訳先言語" data-tauri-drag-region>{languageLabel(targetLanguage)}</span>
    <button
      class="btn"
      class:on={showOriginal}
      aria-pressed={showOriginal}
      title="原文表示"
      onclick={() => (showOriginal = !showOriginal)}
    >
      {#if showOriginal}<Eye class="h-4 w-4" />{:else}<EyeOff class="h-4 w-4" />{/if}
    </button>
    <button class="btn" title="閉じる" onclick={close}><X class="h-4 w-4" /></button>
  </header>

  <section class="body" bind:this={scroller} aria-live="polite" aria-label="字幕">
    {#each view.lines as line (line.id)}
      <div class="line">
        {#if showOriginal && line.translated}
          <p class="src" style="font-size: {Math.round(fontSize * 0.55)}px">{line.original}</p>
        {/if}
        <p class="text" style="font-size: {fontSize}px">{line.translated || line.original}</p>
      </div>
    {:else}
      {#if !view.error}
        <p class="hint">
          {view.running ? 'PC の再生音声を認識し、字幕表示。' : '音声翻訳は停止中。'}
          {#if view.running && view.device}<br />音声取得：{view.device}{/if}
        </p>
      {/if}
    {/each}
    {#if view.error}
      <p class="err" role="alert">{errorText(view.error)}</p>
    {/if}
  </section>
  <div class="grip" role="presentation" title="サイズ変更" onpointerdown={startResize}></div>
</main>

<style>
  :global(html),
  :global(body) {
    background: transparent;
    margin: 0;
  }
  .sub {
    position: relative;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    color: #f4f4f5;
    border-radius: 8px;
    background: rgba(24, 24, 28, 0.82);
    /* ウィンドウ翻訳の範囲枠と同色 */
    box-shadow: inset 0 0 0 1px rgba(139, 92, 246, 0.6);
  }
  /* ウィンドウ翻訳のタイトルバーと同一寸法・配色 */
  .bar {
    box-sizing: border-box;
    height: 32px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 4px 0 12px;
    background: rgba(24, 24, 28, 0.92);
    border-radius: 8px 8px 0 0;
    user-select: none;
    cursor: move;
    container-type: inline-size;
  }
  .title {
    flex: none;
    font-size: 11px;
    opacity: 0.75;
    white-space: nowrap;
  }
  .state {
    flex: none;
    margin-left: 4px;
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 11px;
    white-space: nowrap;
    background: rgba(255, 255, 255, 0.08);
  }
  .state.busy {
    background: rgba(139, 92, 246, 0.45);
  }
  .state.error {
    color: #fca5a5;
  }
  .fill {
    flex: 1;
    align-self: stretch;
    min-width: 0;
  }
  .lang {
    flex: none;
    margin-right: 4px;
    font-size: 11px;
    opacity: 0.75;
    white-space: nowrap;
  }
  /* 狭幅：見出し・翻訳先言語を省略し、状態表記と操作を優先 */
  @container (max-width: 300px) {
    .title,
    .lang {
      display: none;
    }
    .state {
      margin-left: 0;
    }
  }
  .btn {
    flex: none;
    border-radius: 6px;
    padding: 4px;
    cursor: pointer;
    opacity: 0.75;
  }
  .btn.on {
    opacity: 1;
  }
  .btn:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.12);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 16px 12px;
    text-align: center;
    user-select: text;
    -webkit-user-select: text;
  }
  .line + .line {
    margin-top: 12px;
  }
  .line p {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .src {
    opacity: 0.7;
    line-height: 1.4;
  }
  .text {
    font-weight: 600;
    line-height: 1.35;
  }
  /* 過去の字幕は減光、最新行を強調 */
  .line:not(:last-of-type) {
    opacity: 0.6;
  }
  .hint,
  .err {
    margin: 0;
    font-size: 13px;
    line-height: 1.6;
  }
  .hint {
    opacity: 0.7;
  }
  .err {
    margin-top: 8px;
    color: #fca5a5;
    overflow-wrap: anywhere;
  }
  .grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 14px;
    height: 14px;
    cursor: nwse-resize;
    background: linear-gradient(135deg, transparent 50%, rgba(139, 92, 246, 0.85) 50%);
    border-bottom-right-radius: 8px;
  }
</style>
