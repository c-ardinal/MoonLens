<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { Camera, X, RefreshCw, Clipboard, TextSelect } from 'lucide-svelte';
  import PrivacyBadge from '$lib/components/PrivacyBadge.svelte';
  import FlowText from '$lib/components/FlowText.svelte';
  import type { CaptureFlowResult, LensBlock } from '$lib/ipc';
  import { blockCssRect, growRoom, rgb } from '$lib/lens-overlay';
  import { liveText, type LensLiveEvent } from '$lib/live';
  import { extractText } from '$lib/result';
  import { errorText } from '$lib/errors';

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
  /** The last capture ran but found no text in the region. */
  let empty = $state(false);
  /** Read the region continuously and re-translate on change (default). */
  let continuous = $state(true);
  /** Continuous reading: new text is with the model. */
  let translating = $state(false);
  /** Blocks of the last capture, painted in place over the region. */
  let blocks = $state<LensBlock[]>([]);
  /** Blocks whose source changed or moved away; hidden until re-read. */
  let hidden = $state<Set<number>>(new Set());
  /** Show the source / translation as text instead of in place. */
  let textView = $state(false);
  let dpr = $state(typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1);
  const origin = { x: 0, y: 0, w: 0, h: 0 };
  /** Blocks painted over the region (hidden ones keep their place, so a
   * re-read does not flip the lens to the text view meanwhile). */
  const placed = $derived(blocks.filter((b) => b.bbox[2] > 0 && b.bbox[3] > 0 && !b.same_language));
  const inPlace = $derived(!textView && placed.length > 0);
  let headerEl: HTMLElement;
  let unlisten: UnlistenFn | undefined;
  let unlistenLive: UnlistenFn | undefined;
  let unlistenMove: UnlistenFn | undefined;
  let unlistenResize: UnlistenFn | undefined;
  let reportedHeaderHeight = 0;

  const win = getCurrentWebviewWindow();

  async function reportInsets() {
    await tick();
    if (!headerEl) return;
    const top = Math.ceil(headerEl.getBoundingClientRect().height);
    if (top === reportedHeaderHeight) return;
    reportedHeaderHeight = top;
    try {
      await invoke('set_lens_insets', {
        id,
        top,
        bottom: 0,
        left: 0,
        right: 0
      });
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
    empty = !source?.trim() && !translation.trim();
    blocks = at === geometry ? (r.blocks ?? []) : [];
    hidden = new Set();
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
      if (ev.provider) provider = ev.provider;
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
    } else {
      translating = false;
      lastError = errorText(ev.message);
    }
  }

  async function capture() {
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
    if (translation) navigator.clipboard.writeText(translation).catch(() => {});
  }

  $effect(() => {
    invoke('set_lens_continuous', { id, on: continuous }).catch(() => {});
    if (!continuous) translating = false;
  });

  /** Last window position / size, to ignore move events that move nothing. */
  let lastPos = '';
  let lastSize = '';
  function moved(key: string, prev: string): boolean {
    if (key === prev) return false;
    geometry++;
    // Boxes no longer line up: hide them until the region is read again.
    hidden = new Set(blocks.map((_, i) => i));
    // Have the reader look now: it waits for the lens to hold still, and
    // shows the boxes again even if the move turns out to be none.
    if (continuous) invoke('refresh_lens', { id }).catch(() => {});
    return true;
  }

  onMount(async () => {
    reportInsets();
    // One-off captures (continuous reading off) also arrive here.
    unlisten = await listen<CaptureFlowResult>('moonlens://lens-result', (e) => apply(e.payload));
    unlistenLive = await listen<LensLiveEvent>('moonlens://lens-live', (e) => {
      if (continuous) applyLive(e.payload);
    });
    const ro = new ResizeObserver(() => reportInsets());
    if (headerEl) ro.observe(headerEl);
    // Boxes from the last capture no longer line up once the lens moves
    // or is resized; hide them until the next reading.
    try {
      unlistenMove = await win.onMoved(({ payload: p }) => {
        const key = `${p.x},${p.y}`;
        if (moved(key, lastPos)) lastPos = key;
      });
      unlistenResize = await win.onResized(({ payload: p }) => {
        const key = `${p.width}x${p.height}`;
        if (moved(key, lastSize)) lastSize = key;
        dpr = window.devicePixelRatio || 1;
      });
      const pos = await win.outerPosition();
      const size = await win.innerSize();
      lastPos = `${pos.x},${pos.y}`;
      lastSize = `${size.width}x${size.height}`;
    } catch {
      /* not running inside Tauri */
    }
  });

  onDestroy(() => {
    unlisten?.();
    unlistenLive?.();
    unlistenMove?.();
    unlistenResize?.();
    invoke('set_lens_continuous', { id, on: false }).catch(() => {});
  });
</script>

<main
  class="pointer-events-auto flex h-screen flex-col overflow-hidden rounded-ml border border-border/70 shadow-ml-lg"
>
  <header
    bind:this={headerEl}
    data-tauri-drag-region
    class="glass flex select-none items-center justify-between gap-2 border-b border-border/60 px-3 py-2"
  >
    <div class="flex items-center gap-2">
      <span class="pointer-events-none text-[11px] font-mono text-ink-faint"
        >{id.replace('lens-', '')}</span
      >
      {#if inPlace}
        <PrivacyBadge {viaCloud} {provider} />
      {/if}
    </div>
    <div class="flex items-center gap-1">
      <button
        class="rounded-ml-sm px-1.5 py-0.5 text-[11px] hover:bg-surface-muted {continuous
          ? 'text-accent'
          : 'text-ink-muted'}"
        aria-pressed={continuous}
        onclick={() => (continuous = !continuous)}
        title={continuous ? '常時翻訳：ON（画面変化時に再翻訳）' : '常時翻訳：OFF（手動のみ）'}
      >
        常時
      </button>
      <button
        class="rounded-ml-sm p-1 text-ink-muted hover:bg-surface-muted"
        onclick={capture}
        disabled={busy}
        title="再キャプチャ"
      >
        <RefreshCw class="h-4 w-4 {busy || translating ? 'animate-spin' : ''}" />
      </button>
      <button
        class="rounded-ml-sm p-1 text-ink-muted hover:bg-surface-muted disabled:opacity-40"
        aria-pressed={textView}
        disabled={placed.length === 0 && !textView}
        onclick={() => (textView = !textView)}
        title={textView ? '重畳表示' : 'テキスト表示'}
      >
        <TextSelect class="h-4 w-4 {textView ? 'text-accent' : ''}" />
      </button>
      <button
        class="rounded-ml-sm p-1 text-ink-muted hover:bg-surface-muted"
        onclick={copy}
        title="コピー"
      >
        <Clipboard class="h-4 w-4" />
      </button>
      <button
        class="rounded-ml-sm p-1 text-ink-muted hover:bg-surface-muted"
        onclick={close}
        title="閉じる"
      >
        <X class="h-4 w-4" />
      </button>
    </div>
  </header>

  {#if inPlace}
    <!-- In place over the observed region (Lens style). The region starts
         right below the header, so block boxes map 1:1 (÷ dpr). -->
    <section class="relative flex-1 overflow-hidden" aria-label="訳文">
      {#each blocks as b, i (i)}
        {#if !hidden.has(i) && b.bbox[2] > 0 && b.bbox[3] > 0 && !b.same_language}
        {@const r = blockCssRect(b, origin, dpr)}
        <div
          class="lens-box absolute"
          class:pending={b.translation == null}
          style="left: {r.left}px; top: {r.top}px; width: {r.width}px; height: {r.height}px;
                 background: {rgb(b.colors?.background, 'rgba(255,255,255,0.96)')};
                 color: {rgb(b.colors?.foreground, '#111')};"
          title={b.source}
        >
          {#if b.translation != null}
            <FlowText
              block={b}
              text={b.translation}
              scale={dpr}
              grow={growRoom(blocks, i, window.innerWidth * dpr) / dpr}
              background={rgb(b.colors?.background, 'rgba(255,255,255,0.96)')}
            />
          {/if}
        </div>
        {/if}
      {/each}
      {#if busy || translating || lastError}
        <div class="glass absolute bottom-2 right-2 rounded-ml-sm px-2 py-1 text-[11px]">
          {#if lastError}<span class="text-red-500">{lastError}</span>{:else}処理中…{/if}
        </div>
      {/if}
    </section>
  {:else}
  <section class="glass flex-1 overflow-auto scrollbar-minimal p-3">
    {#if !translation && !source && !lastError}
      <div class="flex h-full flex-col items-center justify-center gap-2 text-ink-muted">
        <Camera class="h-6 w-6 text-ink-faint" />
        {#if empty}
          <p class="text-sm">範囲内に文字なし</p>
          <p class="text-xs text-ink-faint">
            {continuous ? '文字の表示時に自動翻訳' : '位置・サイズ調整後、再キャプチャ ↻'}
          </p>
        {:else if continuous}
          <p class="text-sm">対象範囲に配置</p>
          <p class="text-xs text-ink-faint">画面変化時に自動再翻訳</p>
        {:else}
          <p class="text-sm">対象範囲に配置して再キャプチャ ↻</p>
        {/if}
      </div>
    {:else}
      <div class="mb-2 flex items-center justify-between">
        <PrivacyBadge {viaCloud} {provider} />
        {#if busy || translating}
          <span class="text-[11px] text-ink-faint">処理中…</span>
        {/if}
      </div>
      {#if source}
        <p class="font-mono text-[11px] leading-relaxed text-ink-faint">{source}</p>
        <hr class="my-2 border-border/60" />
      {/if}
      <p class="whitespace-pre-wrap text-sm leading-relaxed">{translation}</p>
      {#if lastError}
        <p class="mt-2 text-xs text-red-500">{lastError}</p>
      {/if}
    {/if}
  </section>
  {/if}
</main>

<style>
  .glass {
    background: color-mix(in oklab, var(--ml-surface-elevated) 70%, transparent);
    backdrop-filter: blur(40px) saturate(1.5);
    -webkit-backdrop-filter: blur(40px) saturate(1.5);
  }
  .lens-box {
    border-radius: 3px;
    user-select: text;
  }
  .lens-box.pending {
    background-image: linear-gradient(100deg, transparent 30%, rgba(139, 92, 246, 0.18) 50%, transparent 70%) !important;
  }
  :global(html),
  :global(body) {
    background: transparent;
    margin: 0;
  }
</style>
