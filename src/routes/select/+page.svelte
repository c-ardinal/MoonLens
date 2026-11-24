<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { ClipboardCopy, ExternalLink, QrCode, Radio, Sparkles, X } from 'lucide-svelte';
  import PrivacyBadge from '$lib/components/PrivacyBadge.svelte';
  import LensActions from '$lib/components/LensActions.svelte';
  import {
    closeSelector,
    getFrozenFrame,
    onOverlay,
    openExternalUrl,
    openLens,
    openMainRoute,
    processFrozenSelection,
    retranslateSelection,
    runExplain,
    type CaptureFlowResult,
    type FrameReady,
    type FrameRect,
    type LensBlock,
    type TranslationProgress
  } from '$lib/ipc';
  import { extractText } from '$lib/result';
  import FlowText from '$lib/components/FlowText.svelte';
  import { errorText, parseError, type UiError } from '$lib/errors';
  import {
    allSameLanguage,
    applyStreaming,
    blockCssRect,
    growRoom,
    dragRect,
    joinedText,
    languageLabel,
    rgb,
    toCssRect,
    toFrameRect,
    toolbarPosition,
    type CssRect
  } from '$lib/lens-overlay';

  // Window translation. When it opens, the backend has already frozen the
  // monitor under the cursor; we show that frame, let the user drag (a
  // range) or click (the whole screen), and paint translations in place
  // over the frozen image. With 常時 on, the target opens as a continuous
  // window-translation window over the live screen instead. A new drag
  // starts over; Esc closes.

  type Phase = 'loading' | 'select' | 'processing' | 'result' | 'error';

  let phase = $state<Phase>('loading');
  let frameUrl = $state<string | null>(null);
  let frameW = $state(0);
  let frameH = $state(0);
  let vw = $state(typeof window !== 'undefined' ? window.innerWidth : 1);
  let vh = $state(typeof window !== 'undefined' ? window.innerHeight : 1);
  const scale = $derived(frameW > 0 ? frameW / vw : window.devicePixelRatio || 1);

  let drag = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  let selection = $state<CssRect | null>(null);
  let crop = $state<FrameRect | null>(null);
  let blocks = $state<LensBlock[]>([]);
  let result = $state<CaptureFlowResult | null>(null);
  let showOriginal = $state(false);
  let showPanel = $state(false);
  let failure = $state<UiError | null>(null);
  let notice = $state('');
  let explanation = $state<string | null>(null);
  let monitorIndex = 0;
  let streamed = '';
  /** Blocks as of the latest progress event; streamed text is laid over them. */
  let streamBase: LensBlock[] = [];
  /** Blocks the current request streams into (undefined: all pending). */
  let streamInto: number[] | undefined;
  let runId = 0;
  let toolbarEl = $state<HTMLElement | null>(null);
  let toolbarSize = $state({ width: 360, height: 44 });
  // Target language picked in the toolbar. Remembered per machine so the
  // next selection uses it too; null = the configured default.
  const LANG_KEY = 'moonlens.selector.targetLanguage';
  let chosenLang = $state<string | null>(readChosenLang());
  const targetLang = $derived(result?.target_language ?? chosenLang ?? 'ja');

  function readChosenLang(): string | null {
    try {
      return localStorage.getItem(LANG_KEY);
    } catch {
      return null;
    }
  }

  // 常時: a selection becomes a continuous window translation at once.
  // Remembered per machine like the language.
  const CONTINUOUS_KEY = 'moonlens.selector.continuous';
  let continuous = $state(readContinuous());
  function readContinuous(): boolean {
    try {
      return localStorage.getItem(CONTINUOUS_KEY) === '1';
    } catch {
      return false;
    }
  }
  function toggleContinuous() {
    continuous = !continuous;
    try {
      localStorage.setItem(CONTINUOUS_KEY, continuous ? '1' : '0');
    } catch {
      /* storage unavailable: keep the choice for this session only */
    }
  }

  const unlisteners: UnlistenFn[] = [];

  async function loadFrame(ready: FrameReady) {
    reset();
    phase = 'loading';
    try {
      const buf = await getFrozenFrame();
      if (frameUrl) URL.revokeObjectURL(frameUrl);
      frameUrl = URL.createObjectURL(new Blob([buf], { type: 'image/png' }));
      frameW = ready.width;
      frameH = ready.height;
      monitorIndex = ready.monitor_index ?? 0;
      phase = 'select';
      if (ready.auto_full) await run(null);
    } catch (e) {
      fail(e);
    }
  }

  function reset() {
    runId++;
    drag = null;
    selection = null;
    crop = null;
    blocks = [];
    result = null;
    showOriginal = false;
    showPanel = false;
    failure = null;
    notice = '';
    explanation = null;
    streamed = '';
  }

  function fail(e: unknown) {
    phase = 'error';
    failure = parseError(e);
  }

  /** Close the selector and show the page that fixes the error. */
  async function applyFix(route: string) {
    await close();
    await openMainRoute(route).catch(() => {});
  }

  async function run(css: CssRect | null) {
    if (continuous && phase === 'select') {
      const r = css ? toFrameRect(css, scale) : { x: 0, y: 0, w: frameW, h: frameH };
      return keepTranslating({ monitor_index: monitorIndex, ...r });
    }
    const id = ++runId;
    const full = { left: 0, top: 0, width: vw, height: vh };
    selection = css ?? full;
    crop = css ? toFrameRect(css, scale) : { x: 0, y: 0, w: frameW, h: frameH };
    blocks = [];
    result = null;
    streamed = '';
    explanation = null;
    notice = '';
    phase = 'processing';
    try {
      const r = await processFrozenSelection(css ? crop : null, chosenLang);
      if (id !== runId) return;
      showResult(r);
    } catch (e) {
      if (id === runId) fail(e);
    }
  }

  function showResult(r: CaptureFlowResult) {
    result = r;
    if (r.source_rect) crop = { x: r.source_rect.x, y: r.source_rect.y, w: r.source_rect.w, h: r.source_rect.h };
    blocks = r.blocks ?? [];
    phase = 'result';
    if (r.intent !== 'qr' && blocks.length === 0) {
      notice = 'テキスト未検出。範囲を再選択。';
    } else if (allSameLanguage(blocks)) {
      flash(`翻訳不要（${languageLabel(r.target_language)}）。翻訳先はツールバーで変更可能`, 4000);
    }
  }

  /** Toolbar language switch: re-translate the same OCR result. */
  async function changeLanguage(code: string) {
    if (!result || code === targetLang) return;
    chosenLang = code;
    try {
      localStorage.setItem(LANG_KEY, code);
    } catch {
      /* storage unavailable: keep the choice for this session only */
    }
    const id = ++runId;
    streamed = '';
    explanation = null;
    notice = '';
    showOriginal = false;
    phase = 'processing';
    try {
      const r = await retranslateSelection(code);
      if (id !== runId) return;
      showResult(r);
    } catch (e) {
      if (id === runId) fail(e);
    }
  }

  // ---- pointer handling -------------------------------------------------

  function pointerdown(e: PointerEvent) {
    if (e.button !== 0 || phase === 'loading') return;
    drag = { x0: e.clientX, y0: e.clientY, x1: e.clientX, y1: e.clientY };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function pointermove(e: PointerEvent) {
    if (!drag) return;
    drag = { ...drag, x1: e.clientX, y1: e.clientY };
  }
  async function pointerup() {
    if (!drag) return;
    const r = dragRect(drag.x0, drag.y0, drag.x1, drag.y1);
    drag = null;
    if (r.width < 6 && r.height < 6) {
      // A click (no drag): translate the whole screen, like Lens'
      // "translate" mode. While a result is shown, a click outside the
      // selection just dismisses it so the user can select again.
      if (phase === 'result' || phase === 'error') {
        reset();
        phase = 'select';
        return;
      }
      await run(null);
      return;
    }
    await run(r);
  }

  const onclose = () => close();

  async function close() {
    reset();
    phase = 'loading';
    if (frameUrl) URL.revokeObjectURL(frameUrl);
    frameUrl = null;
    await closeSelector().catch(() => {});
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    } else if (e.key === 'Tab' && phase === 'select') {
      e.preventDefault();
      toggleContinuous();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c' && phase === 'result') {
      // Copy the user's text selection if any, else everything.
      if (!window.getSelection()?.toString()) {
        e.preventDefault();
        copyAll();
      }
    }
  }

  // ---- actions ----------------------------------------------------------

  function flash(msg: string, ms = 1800) {
    notice = msg;
    setTimeout(() => {
      if (notice === msg) notice = '';
    }, ms);
  }

  async function copyAll() {
    const text =
      result?.intent === 'qr'
        ? qrPayload
        : joinedText(blocks, showOriginal ? 'source' : 'translation');
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      flash('コピー完了');
    } catch {
      flash('コピー失敗');
    }
  }

  async function explain() {
    const src = joinedText(blocks, 'source');
    if (!src) return;
    explanation = '説明を生成中…';
    showPanel = true;
    try {
      const r = await runExplain(`次のテキストの内容を${languageLabel(targetLang) || '日本語'}で簡潔に説明してください:\n\n${src}`);
      explanation = extractText(r);
    } catch (e) {
      explanation = `説明の生成に失敗：${errorText(e)}`;
    }
  }

  /** 常時: keep translating `rect` in place on the live screen as it
   * changes (a window-translation window over it), and get out of the way. */
  async function keepTranslating(rect = result?.source_rect) {
    if (!rect) return;
    // Language shown now; none picked yet: the configured target.
    const lang = result?.target_language ?? chosenLang;
    try {
      // Close first: the window must not open under the frozen frame.
      await close();
      await openLens(rect, lang);
    } catch (e) {
      fail(e);
    }
  }

  async function openQr() {
    if (!/^https?:\/\//i.test(qrPayload)) return;
    try {
      await openExternalUrl(qrPayload);
      await close();
    } catch (e) {
      flash(errorText(e), 3000);
    }
  }

  const qrPayload = $derived(
    result?.intent === 'qr' && result ? extractText(result.result) : ''
  );
  const translating = $derived(phase === 'processing' && blocks.length > 0);

  // ---- lifecycle --------------------------------------------------------

  onMount(async () => {
    const onResize = () => {
      vw = window.innerWidth;
      vh = window.innerHeight;
    };
    window.addEventListener('resize', onResize);
    window.addEventListener('keydown', keydown);
    unlisteners.push(() => window.removeEventListener('resize', onResize));
    unlisteners.push(() => window.removeEventListener('keydown', keydown));
    try {
      unlisteners.push(
        await listen<FrameReady>('moonlens://selector-frame', (e) => loadFrame(e.payload))
      );
      unlisteners.push(
        await listen<CaptureFlowResult>('moonlens://selector-analysis', (e) => {
          if (phase !== 'processing') return;
          blocks = e.payload.blocks ?? [];
          streamBase = blocks;
          streamInto = undefined;
          streamed = '';
        })
      );
      unlisteners.push(
        await listen<TranslationProgress>('moonlens://selector-progress', (e) => {
          if (phase !== 'processing') return;
          blocks = e.payload.blocks;
          streamBase = blocks;
          streamInto = e.payload.streaming;
          streamed = '';
        })
      );
      unlisteners.push(
        await onOverlay((ev) => {
          if (ev.kind !== 'chunk' || phase !== 'processing' || blocks.length === 0) return;
          streamed += ev.delta;
          blocks = applyStreaming(streamBase, streamed, streamInto);
        })
      );
    } catch {
      /* not running inside Tauri (pure frontend dev) */
    }
  });

  onDestroy(() => {
    unlisteners.forEach((u) => u());
    if (frameUrl) URL.revokeObjectURL(frameUrl);
  });

  $effect(() => {
    if (toolbarEl && phase === 'result') {
      tick().then(() => {
        if (toolbarEl) toolbarSize = { width: toolbarEl.offsetWidth, height: toolbarEl.offsetHeight };
      });
    }
  });

  const activeRect = $derived(
    drag ? dragRect(drag.x0, drag.y0, drag.x1, drag.y1) : selection
  );
  const toolbarPos = $derived(
    selection ? toolbarPosition(selection, toolbarSize, { width: vw, height: vh }) : null
  );
  const cropCss = $derived(crop ? toCssRect(crop, scale) : null);
</script>

<div
  role="application"
  aria-label="領域を選択"
  class="fixed inset-0 select-none overflow-hidden"
  class:cursor-crosshair={phase !== 'loading'}
  style="background: #000"
  onpointerdown={pointerdown}
  onpointermove={pointermove}
  onpointerup={pointerup}
>
  {#if frameUrl}
    <img
      src={frameUrl}
      alt=""
      draggable="false"
      class="pointer-events-none absolute inset-0 h-full w-full"
    />
  {/if}

  <!-- Dim everything except the active rect. -->
  {#if activeRect}
    {@const r = activeRect}
    <div class="dim absolute inset-x-0 top-0" style="height: {r.top}px"></div>
    <div class="dim absolute left-0" style="top: {r.top}px; width: {r.left}px; height: {r.height}px"></div>
    <div
      class="dim absolute right-0"
      style="top: {r.top}px; width: calc(100% - {r.left + r.width}px); height: {r.height}px"
    ></div>
    <div class="dim absolute inset-x-0 bottom-0" style="height: calc(100% - {r.top + r.height}px)"></div>
    <div
      class="frame pointer-events-none absolute"
      class:scanning={phase === 'processing'}
      style="left: {r.left}px; top: {r.top}px; width: {r.width}px; height: {r.height}px"
    ></div>
    {#if drag}
      <div
        class="pointer-events-none absolute rounded-md bg-black/75 px-2 py-0.5 font-mono text-xs text-white"
        style="left: {r.left}px; top: {Math.max(0, r.top - 24)}px"
      >
        {Math.round(r.width * scale)} × {Math.round(r.height * scale)}
      </div>
    {/if}
  {:else}
    <div class="dim absolute inset-0"></div>
  {/if}

  <!-- In-place translation blocks. -->
  {#if cropCss && blocks.length > 0 && !showOriginal}
    <!-- Clipped to the selection so nothing paints over the dimmed area. -->
    <div
      class="pointer-events-none absolute overflow-hidden"
      style="left: {cropCss.left}px; top: {cropCss.top}px; width: {cropCss.width}px; height: {cropCss.height}px"
    >
    {#each blocks as b, i (i)}
      {#if b.bbox[2] > 0 && crop && !b.same_language}
        {@const r = blockCssRect(b, crop, scale)}
        <div
          role="presentation"
          class="block pointer-events-auto absolute"
          class:pending={b.translation == null}
          style="left: {r.left - cropCss.left}px; top: {r.top - cropCss.top}px; width: {r.width}px; height: {r.height}px;
                 background: {rgb(b.colors?.background, 'rgba(255,255,255,0.96)')};
                 color: {rgb(b.colors?.foreground, '#111')};"
          onpointerdown={(e) => e.stopPropagation()}
          onpointerup={(e) => e.stopPropagation()}
          title={b.source}
        >
          {#if b.translation != null}
            <FlowText
              block={b}
              text={b.translation}
              {scale}
              grow={growRoom(blocks, i, crop.w) / scale}
              background={rgb(b.colors?.background, 'rgba(255,255,255,0.96)')}
            />
          {/if}
        </div>
      {/if}
    {/each}
    </div>
  {/if}

  <!-- Hint pill. -->
  {#if phase === 'select' && !drag}
    <div
      class="pill absolute inset-x-0 top-6 mx-auto flex w-fit items-center gap-3"
      role="toolbar"
      tabindex="-1"
      onpointerdown={(e) => e.stopPropagation()}
      onpointerup={(e) => e.stopPropagation()}
    >
      <span>ドラッグ：範囲 · クリック：全画面</span>
      <button
        class="mode"
        aria-pressed={continuous}
        onclick={toggleContinuous}
        title="常時翻訳：画面変化へ追従して再翻訳（Tab で切替）"
      >
        <Radio class="inline h-3.5 w-3.5" />常時 {continuous ? 'ON' : 'OFF'}
      </button>
      <span>Esc：終了</span>
    </div>
  {:else if phase === 'processing'}
    <div class="pill pointer-events-none absolute inset-x-0 top-6 mx-auto w-fit">
      <Sparkles class="inline h-3.5 w-3.5 animate-pulse" />
      {translating ? '翻訳中…' : '文字認識中…'}
    </div>
  {/if}

  <!-- Result toolbar. -->
  {#if (phase === 'result' || phase === 'error') && toolbarPos}
    <div
      bind:this={toolbarEl}
      class="toolbar absolute flex items-center gap-1"
      style="left: {toolbarPos.left}px; top: {toolbarPos.top}px"
      onpointerdown={(e) => e.stopPropagation()}
      onpointerup={(e) => e.stopPropagation()}
      role="toolbar"
      tabindex="-1"
    >
      {#if phase === 'error'}
        <span class="px-2 text-sm text-red-400">{failure?.message}</span>
        {#if failure?.fix}
          {@const fix = failure.fix}
          <button class="tb" onclick={() => applyFix(fix.route)}>{fix.label}</button>
        {/if}
      {:else if result?.intent === 'qr'}
        <QrCode class="ml-1 h-4 w-4 opacity-80" />
        <span class="max-w-[420px] truncate px-1 text-sm" title={qrPayload}>{qrPayload}</span>
        {#if /^https?:\/\//i.test(qrPayload)}
          <button class="tb" onclick={openQr} title="ブラウザで開く"><ExternalLink class="h-4 w-4" />開く</button>
        {/if}
        <button class="tb" onclick={copyAll} title="コピー"><ClipboardCopy class="h-4 w-4" />コピー</button>
      {:else if blocks.length > 0}
        {#if result}
          <PrivacyBadge viaCloud={result.result.via_cloud} provider={result.result.used_provider} />
        {/if}
        <LensActions
          continuous={false}
          {showOriginal}
          showText={showPanel}
          sourceLanguage={result?.source_language}
          targetLanguage={targetLang}
          oncontinuous={() => keepTranslating()}
          onoriginal={() => (showOriginal = !showOriginal)}
          onlanguage={changeLanguage}
          oncopy={copyAll}
          ontext={() => (showPanel = !showPanel)}
          onexplain={explain}
          {onclose}
        />
      {:else}
        <span class="px-2 text-sm">{notice}</span>
      {/if}
      {#if !(phase === 'result' && result?.intent !== 'qr' && blocks.length > 0)}
        <button class="tb" onclick={close} title="閉じる (Esc)"><X class="h-4 w-4" /></button>
      {/if}
    </div>
  {/if}

  {#if phase === 'error' && !toolbarPos && failure}
    <!-- Failed before anything was selected (e.g. the frame could not be loaded). -->
    <div
      class="pill absolute inset-x-0 top-6 mx-auto flex w-fit items-center gap-2"
      role="toolbar"
      tabindex="-1"
      onpointerdown={(e) => e.stopPropagation()}
      onpointerup={(e) => e.stopPropagation()}
    >
      <span class="text-red-300" role="alert">{failure.message}</span>
      {#if failure.fix}
        {@const fix = failure.fix}
        <button class="underline" onclick={() => applyFix(fix.route)}>{fix.label}</button>
      {/if}
    </div>
  {/if}

  {#if notice && phase === 'result' && blocks.length > 0}
    <div class="pill pointer-events-none absolute inset-x-0 top-6 mx-auto w-fit">{notice}</div>
  {/if}

  <!-- Text panel: selectable source + translation, plus explanations. -->
  {#if showPanel && phase === 'result'}
    <aside
      class="panel absolute right-4 top-4 bottom-4 flex w-[380px] max-w-[45vw] flex-col"
      onpointerdown={(e) => e.stopPropagation()}
      onpointerup={(e) => e.stopPropagation()}
    >
      <header class="flex items-center justify-between border-b border-white/10 px-4 py-2">
        <span class="text-sm font-medium">テキスト</span>
        <button class="tb" onclick={() => (showPanel = false)} title="閉じる"><X class="h-4 w-4" /></button>
      </header>
      <div class="flex-1 select-text overflow-auto px-4 py-3 text-sm leading-relaxed">
        {#if explanation}
          <section class="mb-4 rounded-lg bg-white/5 p-3 whitespace-pre-wrap">{explanation}</section>
        {/if}
        {#each blocks as b, i (i)}
          {#if b.same_language}
            <p class="mb-4 whitespace-pre-wrap">{b.source}</p>
          {:else}
            <p class="mb-1 whitespace-pre-wrap">{b.translation ?? '（翻訳失敗）'}</p>
            <p class="mb-4 whitespace-pre-wrap text-xs opacity-60">{b.source}</p>
          {/if}
        {/each}
      </div>
    </aside>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
    margin: 0;
    padding: 0;
    overflow: hidden;
  }
  .dim {
    background: rgba(0, 0, 0, 0.45);
    pointer-events: none;
  }
  .frame {
    border-radius: 6px;
    box-shadow:
      0 0 0 2px rgba(255, 255, 255, 0.95),
      0 0 0 4px rgba(139, 92, 246, 0.55),
      0 8px 32px rgba(0, 0, 0, 0.35);
  }
  .frame.scanning {
    background: linear-gradient(
      100deg,
      transparent 20%,
      rgba(167, 139, 250, 0.28) 45%,
      rgba(255, 255, 255, 0.35) 50%,
      rgba(167, 139, 250, 0.28) 55%,
      transparent 80%
    );
    background-size: 250% 100%;
    animation: scan 1.2s linear infinite;
  }
  @keyframes scan {
    from {
      background-position: 120% 0;
    }
    to {
      background-position: -120% 0;
    }
  }
  .block {
    border-radius: 3px;
    user-select: text;
    cursor: text;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.04);
  }
  .block.pending {
    background-image: linear-gradient(
      100deg,
      transparent 30%,
      rgba(139, 92, 246, 0.18) 50%,
      transparent 70%
    ) !important;
    background-size: 250% 100%;
    animation: scan 1.1s linear infinite;
  }
  .pill {
    border-radius: 999px;
    background: rgba(20, 20, 24, 0.82);
    color: #fff;
    padding: 6px 16px;
    font-size: 13px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(12px);
  }
  .toolbar {
    z-index: 20;
  }
  .mode {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border-radius: 999px;
    padding: 2px 10px;
    background: rgba(255, 255, 255, 0.1);
  }
  .mode[aria-pressed='true'] {
    background: rgba(139, 92, 246, 0.55);
  }
  .panel {
    z-index: 10;
  }
  .toolbar,
  .panel {
    border-radius: 14px;
    background: rgba(24, 24, 28, 0.9);
    color: #f4f4f5;
    padding: 4px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(16px) saturate(1.6);
    border: 1px solid rgba(255, 255, 255, 0.08);
    cursor: default;
  }
  .panel {
    padding: 0;
  }
  .tb {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 10px;
    padding: 6px 10px;
    font-size: 13px;
    white-space: nowrap;
    transition: background 120ms;
  }
  .tb:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
  }
  .tb:disabled {
    opacity: 0.4;
  }
</style>
