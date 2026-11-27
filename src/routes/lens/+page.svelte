<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { emitTo, type UnlistenFn } from '@tauri-apps/api/event';
  import PrivacyBadge from '$lib/components/PrivacyBadge.svelte';
  import FlowText from '$lib/components/FlowText.svelte';
  import LensActions from '$lib/components/LensActions.svelte';
  import LensText from '$lib/components/LensText.svelte';
  import {
    getSettings,
    listLensWindows,
    openLensBeside,
    runExplain,
    setLensInteractive,
    setLensOperate,
    setLensLanguage,
    setLensTextDock,
    type CaptureFlowResult,
    type LensBlock
  } from '$lib/ipc';
  import { joinedText, languageLabel, rgb } from '$lib/lens-overlay';
  import { liveBoxes, liveText, type LensLiveEvent } from '$lib/live';
  import {
    TEXT_COMMAND_EVENT,
    TEXT_READY_EVENT,
    TEXT_VIEW_EVENT,
    loadPlacement,
    savePlacement,
    textWindowLabel,
    type TextCommand,
    type TextPlacement,
    type TextView
  } from '$lib/lens-text';
  import { extractText } from '$lib/result';
  import { errorText } from '$lib/errors';
  import { displayAccelerator } from '$lib/hotkey';

  // Window translation, continuous. The window sits over the observed
  // range: the header above it can be dragged, the edges and the corner
  // grip resize it, and the body lets the mouse through to the app
  // underneath (the backend switches that by pointer position). Shows the
  // translations in place over the range, or as text in the text view:
  // a panel inside the window, or a window of its own outside of it.

  const params = new URLSearchParams(
    typeof window !== 'undefined' ? window.location.search : ''
  );
  const id = $derived(params.get('id') ?? '');

  let source = $state<string | null>(null);
  let translation = $state('');
  let viaCloud = $state(false);
  let provider = $state('');
  let busy = $state(false);
  let lastError = $state<string | null>(null);
  /** The last reading found no text in the range. */
  let empty = $state(false);
  /** Read the range continuously and re-translate on change (default). */
  let continuous = $state(true);
  /** Continuous reading: new text is with the model. */
  let translating = $state(false);
  /** Continuous reading: text recognition is running. */
  let reading = $state(false);
  /** Reading or translating: the sweep runs over the range. */
  const working = $derived(busy || reading || translating);
  /** Blocks of the last reading, painted in place over the range. */
  let blocks = $state<LensBlock[]>([]);
  /** Blocks whose source changed or moved away; hidden until re-read. */
  let hidden = $state<Set<number>>(new Set());
  /** Show the original (translations off) / the text view. */
  let showOriginal = $state(false);
  let showText = $state(false);
  let explanation = $state<string | null>(null);
  let sourceLanguage = $state<string | null>(null);
  let targetLanguage = $state('ja');
  let dpr = $state(typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1);
  let vw = $state(typeof window !== 'undefined' ? window.innerWidth : 0);
  const boxes = $derived(liveBoxes(blocks, hidden, dpr, vw * dpr));
  const hasText = $derived(!!(source?.trim() || translation.trim()));
  let headerEl: HTMLElement;
  const unlisteners: (UnlistenFn | undefined)[] = [];
  let reportedHeaderHeight = 0;

  const win = getCurrentWebviewWindow();

  async function reportInsets() {
    await tick();
    if (!headerEl) return;
    const top = Math.ceil(headerEl.getBoundingClientRect().height);
    if (top === reportedHeaderHeight) return;
    reportedHeaderHeight = top;
    try {
      await invoke('set_lens_insets', { id, top, bottom: 0, left: 0, right: 0 });
    } catch {
      /* Tauri may not be ready in dev */
    }
  }

  /** Bumped when the lens moves or resizes: results of captures started
   * before that belong to another spot and must not be painted. */
  let geometry = 0;
  /** `geometry` when the latest capture started. */
  let captureGeometry = 0;

  function apply(r: CaptureFlowResult, at = captureGeometry) {
    source = r.source_text;
    // No text found: show the empty state, not "(photo detected)".
    translation = 'Intent' in (r.result.output as object) ? '' : extractText(r.result);
    viaCloud = r.result.via_cloud;
    provider = r.result.used_provider;
    sourceLanguage = r.source_language ?? null;
    empty = !source?.trim() && !translation.trim();
    blocks = at === geometry ? (r.blocks ?? []) : [];
    hidden = new Set();
    // QR code etc.: no boxes to place, so it is shown as text.
    if (!blocks.length && translation.trim()) showText = true;
  }

  /** Continuous reading (`moonlens://lens-live`). */
  function applyLive(ev: LensLiveEvent) {
    if (ev.kind === 'blocks') {
      const t = liveText(ev.blocks);
      blocks = ev.blocks;
      hidden = new Set();
      source = t.source;
      translation = t.translation;
      translating = ev.translating;
      empty = false;
      lastError = null;
      if (ev.provider) {
        provider = ev.provider;
        viaCloud = ev.via_cloud ?? false;
      }
    } else if (ev.kind === 'reading') {
      reading = ev.on;
    } else if (ev.kind === 'stale') {
      hidden = new Set([...hidden, ...ev.indices]);
    } else if (ev.kind === 'clear') {
      blocks = [];
      hidden = new Set();
      source = null;
      translation = '';
      translating = false;
      empty = true;
      lastError = null;
    } else if (ev.kind === 'decoded') {
      // QR code: no boxes to place, so it is shown as text.
      blocks = [];
      hidden = new Set();
      source = null;
      translation = ev.text;
      translating = false;
      empty = false;
      lastError = null;
      showText = true;
    } else {
      translating = false;
      lastError = errorText(ev.message);
    }
  }

  async function refresh() {
    if (busy) return;
    // Continuous: ask the reader to read now (also when nothing changed).
    if (continuous) {
      try {
        if (await invoke<boolean>('refresh_lens', { id })) return;
      } catch {
        /* fall back to a one-off capture */
      }
    }
    busy = true;
    lastError = null;
    const at = (captureGeometry = geometry);
    try {
      const r = (await invoke('capture_lens_now', { id })) as CaptureFlowResult;
      apply(r, at);
    } catch (e) {
      lastError = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function close() {
    await invoke('close_lens_window', { id });
  }

  function copy() {
    const text = showOriginal ? (source ?? '') : translation;
    if (text) navigator.clipboard.writeText(text).catch(() => {});
  }

  async function changeLanguage(code: string) {
    if (code === targetLanguage) return;
    targetLanguage = code;
    await setLensLanguage(id, code).catch(() => {});
    // One-off mode: nothing re-reads on its own.
    if (!continuous) refresh();
  }

  async function explain() {
    const src = source?.trim() || joinedText(blocks, 'source');
    if (!src) return;
    showText = true;
    explanation = '説明を生成中…';
    try {
      const r = await runExplain(
        // copy-lint-ignore（LLM プロンプト）
        `次のテキストの内容を${languageLabel(targetLanguage) || '日本語'}で簡潔に説明してください:\n\n${src}`
      );
      explanation = extractText(r);
    } catch (e) {
      explanation = `説明の生成に失敗：${errorText(e)}`;
    }
  }

  $effect(() => {
    invoke('set_lens_continuous', { id, on: continuous }).catch(() => {});
    if (!continuous) {
      translating = false;
      reading = false;
    }
  });

  /** Where the text view goes: inside the window, or outside of it (a
   * window of its own that follows this one; see `lens_text.rs`). */
  let placement = $state<TextPlacement>(loadPlacement());
  const textInside = $derived(showText && placement === 'inside');
  const textOutside = $derived(showText && placement !== 'inside');
  const textView = $derived<TextView>({ blocks, translation, source, explanation, placement });
  /** Bumped when the outside text window reports it is listening. */
  let textReady = $state(0);
  function setPlacement(p: TextPlacement) {
    placement = p;
    savePlacement(p);
  }
  $effect(() => {
    setLensTextDock(id, showText && placement !== 'inside' ? placement : null).catch(() => {});
  });
  $effect(() => {
    void textReady;
    if (!textOutside) return;
    emitTo(textWindowLabel(id), TEXT_VIEW_EVENT, $state.snapshot(textView)).catch(() => {});
  });

  /** The overflow menu of the header is open (it hangs into the body). */
  let menuOpen = $state(false);

  /** Operate mode: the body takes the mouse, so the translation can be
   * selected and copied. Off (default): clicks go to the app underneath.
   * The text panel and the open menu have controls of their own, so the
   * body takes the mouse for them too. */
  let operate = $state(false);
  let operateKey = $state('');
  $effect(() => {
    setLensInteractive(id, operate || textInside || menuOpen).catch(() => {});
  });
  function setOperate(on: boolean) {
    operate = on;
    setLensOperate(id, on).catch(() => {});
    if (!on) window.getSelection()?.removeAllRanges();
  }

  /** Maximized: the window translates the whole screen. */
  let maximized = $state(false);
  function toggleMaximize() {
    win.toggleMaximize().catch(() => {});
  }

  function startResize(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    win.startResizeDragging('SouthEast').catch(() => {});
  }

  /** Last window position / size, to ignore move events that move nothing. */
  let lastPos = '';
  let lastSize = '';
  function moved(key: string, prev: string): boolean {
    if (key === prev) return false;
    geometry++;
    // Boxes no longer line up: hide them until the range is read again.
    hidden = new Set(blocks.map((_, i) => i));
    // Have the reader look now: it waits for the lens to hold still, and
    // shows the boxes again even if the move turns out to be none.
    if (continuous) invoke('refresh_lens', { id }).catch(() => {});
    return true;
  }

  onMount(async () => {
    reportInsets();
    const onResize = () => (vw = window.innerWidth);
    window.addEventListener('resize', onResize);
    unlisteners.push(() => window.removeEventListener('resize', onResize));
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (menuOpen) menuOpen = false;
      else close();
    };
    window.addEventListener('keydown', onKey);
    unlisteners.push(() => window.removeEventListener('keydown', onKey));
    // Language: the one this lens was opened with, else the configured one.
    Promise.all([listLensWindows(), getSettings()])
      .then(([lenses, s]: [{ id: string; target_language: string | null }[], any]) => {
        targetLanguage =
          lenses.find((l) => l.id === id)?.target_language ?? s?.subtitle?.target_language ?? 'ja';
        const k = s?.hotkeys?.toggle_window_operate;
        operateKey = k ? displayAccelerator(k) : '';
      })
      .catch(() => {});
    try {
      // Only this window's events: with several windows open, the global
      // `listen` would also paint the other windows' results here.
      // One-off captures (continuous reading off) also arrive here.
      unlisteners.push(
        await win.listen<CaptureFlowResult>('moonlens://lens-result', (e) => apply(e.payload))
      );
      // Operate-mode hotkey (all windows at once).
      unlisteners.push(await win.listen<boolean>('moonlens://lens-operate', (e) => setOperate(e.payload)));
      // Outside text window: it asks for the content once it listens,
      // and passes on what the user does there.
      unlisteners.push(await win.listen(TEXT_READY_EVENT, () => textReady++));
      unlisteners.push(
        await win.listen<TextCommand>(TEXT_COMMAND_EVENT, ({ payload: c }) => {
          if (c.kind === 'placement') setPlacement(c.value);
          else showText = false;
        })
      );
      unlisteners.push(
        await win.listen<LensLiveEvent>('moonlens://lens-live', (e) => {
          if (continuous) applyLive(e.payload);
        })
      );
    } catch {
      /* not running inside Tauri */
    }
    const ro = new ResizeObserver(() => reportInsets());
    if (headerEl) ro.observe(headerEl);
    unlisteners.push(() => ro.disconnect());
    try {
      unlisteners.push(
        await win.onMoved(({ payload: p }) => {
          const key = `${p.x},${p.y}`;
          if (moved(key, lastPos)) lastPos = key;
        })
      );
      unlisteners.push(
        await win.onResized(({ payload: p }) => {
          const key = `${p.width}x${p.height}`;
          if (moved(key, lastSize)) lastSize = key;
          dpr = window.devicePixelRatio || 1;
          win
            .isMaximized()
            .then((m) => (maximized = m))
            .catch(() => {});
        })
      );
      const pos = await win.outerPosition();
      const size = await win.innerSize();
      lastPos = `${pos.x},${pos.y}`;
      lastSize = `${size.width}x${size.height}`;
    } catch {
      /* not running inside Tauri */
    }
  });

  onDestroy(() => {
    unlisteners.forEach((u) => u?.());
    invoke('set_lens_continuous', { id, on: false }).catch(() => {});
  });

  const status = $derived(
    lastError
      ? lastError
      : busy || reading
        ? '文字認識中…'
        : translating
          ? '翻訳中…'
        : empty
          ? '範囲内に文字なし'
          : !hasText
            ? continuous
              ? '画面変化時に自動翻訳'
              : '再取得で翻訳'
            : null
  );
</script>

<main class="lens">
  <header bind:this={headerEl} data-tauri-drag-region class="bar">
    <LensActions
      bind:menuOpen
      {continuous}
      {showOriginal}
      {showText}
      {targetLanguage}
      {hasText}
      busy={working}
      oncontinuous={() => (continuous = !continuous)}
      {operate}
      {operateKey}
      onoperate={() => setOperate(!operate)}
      onrefresh={refresh}
      onoriginal={() => (showOriginal = !showOriginal)}
      onlanguage={changeLanguage}
      oncopy={copy}
      ontext={() => (showText = !showText)}
      onexplain={explain}
      {maximized}
      onmaximize={toggleMaximize}
      onadd={() => openLensBeside(id).catch(() => {})}
      onclose={close}
    >
      {#snippet lead()}
        <div class="lead pointer-events-none flex items-center gap-2" data-tauri-drag-region>
          <span class="title">ウィンドウ翻訳</span>
          {#if hasText}
            <PrivacyBadge {viaCloud} {provider} />
          {/if}
        </div>
      {/snippet}
    </LensActions>
  </header>

  <!-- The observed range. Transparent; the mouse passes through it. -->
  <section class="body" class:operate class:working aria-label="訳文" aria-busy={working}>
    {#if !showOriginal}
      {#each boxes as b (b.rect.key)}
        <div
          class="box"
          class:pending={b.pending}
          style="left: {b.rect.left}px; top: {b.rect.top}px; width: {b.rect.width}px; height: {b.rect.height}px;
                 background: {rgb(b.block.colors?.background, 'rgba(255,255,255,0.96)')};
                 color: {rgb(b.block.colors?.foreground, '#111')};"
        >
          {#if !b.pending}
            <FlowText
              block={b.block}
              text={b.block.translation ?? ''}
              scale={dpr}
              grow={b.grow}
              background={rgb(b.block.colors?.background, 'rgba(255,255,255,0.96)')}
            />
          {/if}
        </div>
      {/each}
    {/if}

    {#if textInside}
      <aside class="panel" aria-label="テキスト">
        <LensText view={textView} onplacement={setPlacement} onclose={() => (showText = false)} />
      </aside>
    {/if}

    {#if status}
      <div class="status" class:error={!!lastError}>{status}</div>
    {/if}
    <!-- Resize grip (also clickable while the body lets the mouse through). -->
    {#if !maximized}
      <div class="grip" role="presentation" title="サイズ変更" onpointerdown={startResize}></div>
    {/if}
  </section>
</main>

<style>
  :global(html),
  :global(body) {
    background: transparent;
    margin: 0;
  }
  .lens {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    color: #f4f4f5;
  }
  /* Fixed height: the backend puts this header right above the range
     (lens::HEADER_CSS). */
  .bar {
    /* 縦メニュー（LensActions）の配置基準 */
    position: relative;
    box-sizing: border-box;
    height: 32px;
    flex: none;
    display: flex;
    align-items: center;
    /* 項目間隔：LensActions の MORE（縦メニューボタンの占有幅）と同値 */
    gap: 4px;
    padding: 0 4px;
    background: rgba(24, 24, 28, 0.92);
    border-radius: 8px 8px 0 0;
    user-select: none;
    --la-hover: rgba(255, 255, 255, 0.12);
    --la-pressed: color-mix(in srgb, var(--ml-accent) 45%, transparent);
    --la-bg: rgb(24, 24, 28);
    container-type: inline-size;
  }
  .lead {
    flex: none;
    margin: 0 4px;
  }
  /* 狭い範囲：見出しを省略し操作列へ幅を譲渡 */
  @container (max-width: 300px) {
    .lead {
      display: none;
    }
  }
  /* 既定幅（460px）：見出しの文字を省略し、操作列の全項目を表示 */
  @container (max-width: 520px) {
    .title {
      display: none;
    }
  }
  .title {
    font-size: 11px;
    opacity: 0.75;
    white-space: nowrap;
  }
  .body {
    position: relative;
    flex: 1;
    overflow: hidden;
    /* Marks the range without covering it. */
    box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--ml-accent) 60%, transparent);
  }
  /* 操作モード：枠を破線へ変更し（配色に依存しない識別）、訳文を選択可能化 */
  .body.operate {
    box-shadow: none;
    outline: 2px dashed color-mix(in srgb, var(--ml-accent) 70%, white);
    outline-offset: -2px;
  }
  .box {
    position: absolute;
    border-radius: 3px;
  }
  .body.operate .box {
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
  }
  /* 処理中（文字認識・翻訳）：斜光が左から右へ反復走査。transform のみの演出（再描画なし） */
  .body.working::before {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 45%;
    pointer-events: none;
    background: linear-gradient(
      90deg,
      transparent,
      color-mix(in srgb, var(--ml-accent) 26%, transparent) 40%,
      rgba(255, 255, 255, 0.34) 50%,
      color-mix(in srgb, var(--ml-accent) 26%, transparent) 60%,
      transparent
    );
    transform: translateX(-140%) skewX(-16deg);
    animation: sweep 1.2s linear infinite;
    will-change: transform;
  }
  @keyframes sweep {
    to {
      transform: translateX(330%) skewX(-16deg);
    }
  }
  .box.pending {
    background-image: linear-gradient(100deg, transparent 30%, color-mix(in srgb, var(--ml-accent) 18%, transparent) 50%, transparent 70%) !important;
  }
  /* 動作軽減の設定時：走査を停止し、淡色の単色で表示 */
  @media (prefers-reduced-motion: reduce) {
    .body.working::before {
      animation: none;
      transform: none;
      width: 100%;
      background: color-mix(in srgb, var(--ml-accent) 12%, transparent);
    }
  }
  .panel {
    position: absolute;
    inset: 8px 8px 8px auto;
    width: min(360px, calc(100% - 16px));
    display: flex;
    flex-direction: column;
    border-radius: 10px;
    background: rgba(24, 24, 28, 0.94);
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.4);
  }
  .status {
    position: absolute;
    left: 8px;
    bottom: 8px;
    max-width: calc(100% - 40px);
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 11px;
    background: rgba(24, 24, 28, 0.85);
  }
  .status.error {
    color: #fca5a5;
  }
  .grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 18px;
    height: 18px;
    cursor: nwse-resize;
    background: linear-gradient(135deg, transparent 50%, color-mix(in srgb, var(--ml-accent) 85%, transparent) 50%);
    border-bottom-right-radius: 2px;
  }
</style>
