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
  let autoRefreshMs = $state(0);
  /** Blocks of the last capture, painted in place over the region. */
  let blocks = $state<LensBlock[]>([]);
  /** Show the source / translation as text instead of in place. */
  let textView = $state(false);
  let dpr = $state(typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1);
  const origin = { x: 0, y: 0, w: 0, h: 0 };
  const placed = $derived(blocks.filter((b) => b.bbox[2] > 0 && b.bbox[3] > 0 && !b.same_language));
  const inPlace = $derived(!textView && placed.length > 0);
  let headerEl: HTMLElement;
  let unlisten: UnlistenFn | undefined;
  let unlistenMove: UnlistenFn | undefined;
  let unlistenResize: UnlistenFn | undefined;
  let refreshTimer: ReturnType<typeof setInterval> | undefined;
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

  function apply(r: CaptureFlowResult) {
    source = r.source_text;
    // No text found: show the empty state, not "(photo detected)".
    translation = 'Intent' in (r.result.output as object) ? '' : extractText(r.result);
    viaCloud = r.result.via_cloud;
    provider = r.result.used_provider;
    empty = !source?.trim() && !translation.trim();
    blocks = r.blocks ?? [];
  }

  async function capture() {
    if (busy) return;
    busy = true;
    lastError = null;
    try {
      const r = (await invoke('capture_lens_now', { id })) as CaptureFlowResult;
      apply(r);
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
    // Reset the interval whenever the user changes autoRefreshMs.
    if (refreshTimer) clearInterval(refreshTimer);
    if (autoRefreshMs > 0) {
      refreshTimer = setInterval(() => capture(), autoRefreshMs);
      invoke('set_lens_auto_refresh', { id, ms: autoRefreshMs }).catch(() => {});
    } else {
      invoke('set_lens_auto_refresh', { id, ms: null }).catch(() => {});
    }
  });

  onMount(async () => {
    reportInsets();
    // Listen for the pipeline result in case the backend drives a capture
    // (e.g. from auto-refresh rules we add later).
    unlisten = await listen<CaptureFlowResult>('moonlens://lens-result', (e) => apply(e.payload));
    const ro = new ResizeObserver(() => reportInsets());
    if (headerEl) ro.observe(headerEl);
    // Boxes from the last capture no longer line up once the lens moves
    // or is resized; drop them until the next capture.
    try {
      unlistenMove = await win.onMoved(() => (blocks = []));
      unlistenResize = await win.onResized(() => {
        blocks = [];
        dpr = window.devicePixelRatio || 1;
      });
    } catch {
      /* not running inside Tauri */
    }
  });

  onDestroy(() => {
    unlisten?.();
    unlistenMove?.();
    unlistenResize?.();
    if (refreshTimer) clearInterval(refreshTimer);
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
      <label class="flex items-center gap-1 text-[11px] text-ink-muted">
        自動
        <select
          bind:value={autoRefreshMs}
          class="rounded-ml-sm border border-border bg-surface px-1 py-0.5 text-[11px]"
        >
          <option value={0}>OFF</option>
          <option value={1500}>1.5s</option>
          <option value={3000}>3s</option>
          <option value={10000}>10s</option>
        </select>
      </label>
      <button
        class="rounded-ml-sm p-1 text-ink-muted hover:bg-surface-muted"
        onclick={capture}
        disabled={busy}
        title="再キャプチャ"
      >
        <RefreshCw class="h-4 w-4 {busy ? 'animate-spin' : ''}" />
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
        {#if b.bbox[2] > 0 && b.bbox[3] > 0 && !b.same_language}
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
      {#if busy || lastError}
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
          <p class="text-xs text-ink-faint">位置・サイズ調整後、再キャプチャ ↻</p>
        {:else}
          <p class="text-sm">対象範囲に配置して再キャプチャ ↻</p>
        {/if}
      </div>
    {:else}
      <div class="mb-2 flex items-center justify-between">
        <PrivacyBadge {viaCloud} {provider} />
        {#if busy}
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
