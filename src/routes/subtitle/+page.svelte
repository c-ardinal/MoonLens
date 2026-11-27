<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { AppWindow, Check, ChevronDown, Eye, EyeOff, Monitor, Plus, RotateCw, X } from 'lucide-svelte';
  import {
    closeSubtitleWindow,
    getSettings,
    listAudioApps,
    onSettingsChanged,
    onSubtitle,
    openSubtitleWindow,
    setSubtitleSource,
    subtitleSession,
    type AudioApp,
    type AudioSource,
    type Settings
  } from '$lib/ipc';
  import { errorText } from '$lib/errors';
  import { languageLabel } from '$lib/lens-overlay';
  import {
    initialView,
    reduce,
    restore,
    sameSource,
    sourceLabel,
    sourceOptions,
    statusLabel
  } from '$lib/subtitle';

  // 音声翻訳ウィンドウ（複数可）。タイトルバーのドラッグで移動、右下隅で寸法変更、
  // × で当該ウィンドウを停止・閉鎖、＋ でウィンドウ追加。音源（PC 全体／アプリ単位）は
  // ウィンドウ単位で選択。字幕は時系列で蓄積し、最新行へ自動スクロール。

  /** 音源メニュー表示中のアプリ一覧の更新間隔。 */
  const APPS_REFRESH_MS = 1500;

  let view = $state(initialView());
  const status = $derived(statusLabel(view));
  const busy = $derived(view.running && view.phase !== 'waiting');
  // 設定 → 音声翻訳
  let showOriginal = $state(true);
  let fontSize = $state(22);
  let targetLanguage = $state('ja');
  let scroller = $state<HTMLElement>();
  const unlisteners: (UnlistenFn | undefined)[] = [];

  // 音源メニュー
  let menuOpen = $state(false);
  let apps = $state<AudioApp[]>([]);
  let appsSupported = $state(true);
  let appsLoaded = $state(false);
  /** 切替処理中の音源（多重操作の防止、行内の進行表示）。 */
  let switching = $state<AudioSource | null>(null);
  let menuError = $state<string | null>(null);
  let menu = $state<HTMLElement>();
  let sourceButton = $state<HTMLElement>();
  const options = $derived(sourceOptions(apps, view));
  const source = $derived(sourceLabel(view));

  function applySettings(s: Settings) {
    showOriginal = s?.subtitle?.show_original ?? true;
    fontSize = s?.subtitle?.font_size_px ?? 22;
    targetLanguage = s?.subtitle?.target_language ?? 'ja';
  }

  function close() {
    closeSubtitleWindow().catch(() => {});
  }

  function addWindow() {
    openSubtitleWindow().catch((e) => (view = { ...view, error: errorText(e) }));
  }

  async function refreshApps() {
    try {
      const found = await listAudioApps();
      apps = found.apps;
      appsSupported = found.supported;
    } catch {
      /* 一覧の取得失敗：前回の一覧を維持 */
    }
    appsLoaded = true;
  }

  async function toggleMenu() {
    menuOpen = !menuOpen;
    menuError = null;
    if (!menuOpen) return;
    refreshApps();
    await tick();
    menu?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
  }

  function closeMenu() {
    menuOpen = false;
    sourceButton?.focus();
  }

  /** 音源を変更。失敗時は従来の音源を継続し、理由をメニュー内へ表示。 */
  async function choose(next: AudioSource) {
    if (switching) return;
    switching = next;
    menuError = null;
    try {
      await setSubtitleSource(next);
      closeMenu();
    } catch (e) {
      menuError = errorText(e);
      refreshApps();
    } finally {
      switching = null;
    }
  }

  /** 停止後（連続失敗・アプリ終了）：同一音源で再開。 */
  function restart() {
    setSubtitleSource(view.source).catch((e) => (view = { ...view, error: errorText(e) }));
  }

  function onMenuKey(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const items = [...(menu?.querySelectorAll<HTMLElement>('[role="menuitemradio"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const step = e.key === 'ArrowDown' ? 1 : -1;
    items[(at + step + items.length) % items.length]?.focus();
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

  // メニュー表示中：再生状況を定期更新。
  $effect(() => {
    if (!menuOpen) return;
    const timer = setInterval(refreshApps, APPS_REFRESH_MS);
    return () => clearInterval(timer);
  });

  onMount(async () => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (menuOpen) closeMenu();
      else close();
    };
    window.addEventListener('keydown', onKey);
    unlisteners.push(() => window.removeEventListener('keydown', onKey));
    getSettings().then(applySettings).catch(() => {});
    try {
      const label = getCurrentWebviewWindow().label;
      unlisteners.push(await onSettingsChanged(applySettings));
      unlisteners.push(
        await onSubtitle((ev) => {
          if (ev.id === label) view = reduce(view, ev);
        })
      );
      // 開始イベントは受信登録より先行：現況を照会。
      if (!view.running) view = restore(view, await subtitleSession());
    } catch {
      /* not running inside Tauri */
    }
  });
  onDestroy(() => unlisteners.forEach((u) => u?.()));
</script>

<main class="sub">
  <header class="bar" data-tauri-drag-region>
    <button class="btn" title="音声翻訳ウィンドウ追加" aria-label="音声翻訳ウィンドウ追加" onclick={addWindow}>
      <Plus class="h-4 w-4" />
    </button>
    <span class="title" data-tauri-drag-region>音声翻訳</span>
    <button
      class="source"
      class:open={menuOpen}
      bind:this={sourceButton}
      title="音源選択：{source}"
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      onclick={toggleMenu}
    >
      {#if view.source.kind === 'system'}
        <Monitor class="h-3.5 w-3.5 flex-none" />
      {:else}
        <AppWindow class="h-3.5 w-3.5 flex-none" />
      {/if}
      <span class="source-name">{source}</span>
      <ChevronDown class="chevron h-3 w-3 flex-none" />
    </button>
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
          {#if !view.running}
            音声翻訳は停止中。
          {:else if view.source.kind === 'system'}
            PC の再生音声を認識し、字幕表示。
            {#if view.device}<br />音声取得：{view.device}{/if}
          {:else}
            アプリ「{source}」の再生音声を認識し、字幕表示。
          {/if}
        </p>
      {/if}
    {/each}
    {#if view.error}
      <p class="err" role="alert">{errorText(view.error)}</p>
    {/if}
    {#if !view.running}
      <button class="restart" onclick={restart}><RotateCw class="h-3.5 w-3.5" />再開</button>
    {/if}
  </section>

  {#if menuOpen}
    <!-- 小寸法のウィンドウでも一覧可能とするため、字幕領域全面へ表示 -->
    <div class="menu" role="menu" tabindex="-1" aria-label="音源選択" bind:this={menu} onkeydown={onMenuKey}>
      {#each options as option (option.source.kind === 'app' ? option.source.pid : 'system')}
        {@const pending = !!switching && sameSource(switching, option.source)}
        <button
          class="option"
          class:selected={option.selected}
          role="menuitemradio"
          aria-checked={option.selected}
          disabled={!!switching}
          onclick={() => choose(option.source)}
        >
          {#if option.source.kind === 'system'}
            <Monitor class="h-4 w-4 flex-none" />
          {:else}
            <AppWindow class="h-4 w-4 flex-none" />
          {/if}
          <span class="option-text">
            <span class="option-name">{option.label}</span>
            {#if option.detail}<span class="option-detail">{option.detail}</span>{/if}
          </span>
          <span class="meter" class:playing={option.playing} title={option.playing ? '再生中' : '無音'} aria-label={option.playing ? '再生中' : '無音'}>
            <i></i><i></i><i></i>
          </span>
          <span class="mark">
            {#if pending}
              <span class="spin"></span>
            {:else if option.selected}
              <Check class="h-4 w-4" />
            {/if}
          </span>
        </button>
      {/each}
      {#if !appsSupported}
        <p class="note">アプリ単位の音声取得は Windows 限定。</p>
      {:else if appsLoaded && apps.length === 0}
        <p class="note">音声出力中のアプリなし。再生開始後、自動で一覧へ追加。</p>
      {/if}
      {#if menuError}
        <p class="note bad" role="alert">{menuError}</p>
      {/if}
    </div>
  {/if}
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
    padding: 0 4px;
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
  /* 音源選択：現在の音源名を常時表示、幅不足時は省略表記 */
  .source {
    flex: 0 1 auto;
    min-width: 44px;
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: 4px;
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
    background: rgba(255, 255, 255, 0.08);
    transition: background-color 120ms;
  }
  .source:hover,
  .source.open {
    background: color-mix(in srgb, var(--ml-accent, #8b5cf6) 45%, transparent);
  }
  .source-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .source :global(.chevron) {
    opacity: 0.75;
    transition: transform 120ms;
  }
  .source.open :global(.chevron) {
    transform: rotate(180deg);
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
  /* 狭幅：見出し・翻訳先言語を省略し、音源・状態表記と操作を優先 */
  @container (max-width: 400px) {
    .title,
    .lang {
      display: none;
    }
  }
  @container (max-width: 260px) {
    .state {
      display: none;
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
  .restart {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-top: 8px;
    border-radius: 6px;
    padding: 4px 12px;
    font-size: 12px;
    cursor: pointer;
    background: color-mix(in srgb, var(--ml-accent, #8b5cf6) 45%, transparent);
  }
  .restart:hover {
    background: color-mix(in srgb, var(--ml-accent, #8b5cf6) 65%, transparent);
  }
  /* 音源メニュー：字幕領域全面 */
  .menu {
    position: absolute;
    inset: 32px 0 0;
    z-index: 1;
    overflow-y: auto;
    padding: 4px;
    border-radius: 0 0 8px 8px;
    background: rgba(24, 24, 28, 0.97);
    animation: menu-in 120ms ease-out;
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .option {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 6px;
    padding: 4px 8px;
    min-height: 32px;
    text-align: left;
    cursor: pointer;
  }
  .option:hover:not(:disabled),
  .option:focus-visible {
    outline: none;
    background: rgba(255, 255, 255, 0.1);
  }
  .option.selected {
    background: color-mix(in srgb, var(--ml-accent, #8b5cf6) 30%, transparent);
  }
  .option:disabled {
    cursor: progress;
  }
  .option-text {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .option-name {
    flex: none;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    font-weight: 600;
  }
  .option-detail {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    opacity: 0.6;
  }
  /* 再生状況：再生中は 3 本のバーを伸縮、無音時は減光・静止 */
  .meter {
    flex: none;
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 12px;
    opacity: 0.3;
  }
  .meter i {
    width: 2px;
    height: 4px;
    border-radius: 1px;
    background: currentColor;
  }
  .meter.playing {
    opacity: 1;
    color: var(--ml-accent, #8b5cf6);
  }
  .meter.playing i {
    height: 12px;
    transform-origin: bottom;
    animation: meter 900ms ease-in-out infinite;
  }
  .meter.playing i:nth-child(2) {
    animation-delay: -300ms;
  }
  .meter.playing i:nth-child(3) {
    animation-delay: -600ms;
  }
  @keyframes meter {
    0%,
    100% {
      transform: scaleY(0.3);
    }
    50% {
      transform: scaleY(1);
    }
  }
  .mark {
    flex: none;
    width: 16px;
    height: 16px;
  }
  .spin {
    display: block;
    width: 12px;
    height: 12px;
    margin: 2px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.25);
    border-top-color: #fff;
    animation: spin 700ms linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .note {
    margin: 0;
    padding: 4px 8px;
    font-size: 11px;
    line-height: 1.6;
    opacity: 0.7;
  }
  .note.bad {
    opacity: 1;
    color: #fca5a5;
    overflow-wrap: anywhere;
  }
  @media (prefers-reduced-motion: reduce) {
    .menu,
    .meter.playing i,
    .source :global(.chevron) {
      animation: none;
      transition: none;
    }
  }
  .grip {
    z-index: 2;
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
