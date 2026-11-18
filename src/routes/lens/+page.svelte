<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { Camera, Pin, X, RefreshCw, Clipboard } from 'lucide-svelte';
  import PrivacyBadge from '$lib/components/PrivacyBadge.svelte';
  import type { CaptureFlowResult } from '$lib/ipc';
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
  let autoRefreshMs = $state(0);
  let headerEl: HTMLElement;
  let unlisten: UnlistenFn | undefined;
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

  async function capture() {
    if (busy) return;
    busy = true;
    lastError = null;
    try {
      const r = (await invoke('capture_lens_now', { id })) as CaptureFlowResult;
      source = r.source_text;
      translation = extractText(r.result);
      viaCloud = r.result.via_cloud;
      provider = r.result.used_provider;
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
    unlisten = await listen<CaptureFlowResult>('moonlens://lens-result', (e) => {
      source = e.payload.source_text;
      translation = extractText(e.payload.result);
      viaCloud = e.payload.result.via_cloud;
      provider = e.payload.result.used_provider;
    });
    const ro = new ResizeObserver(() => reportInsets());
    if (headerEl) ro.observe(headerEl);
  });

  onDestroy(() => {
    unlisten?.();
    if (refreshTimer) clearInterval(refreshTimer);
  });
</script>

<main
  class="pointer-events-auto flex h-screen flex-col overflow-hidden rounded-ml border border-border/70 bg-surface-elevated/70 shadow-ml-lg backdrop-blur-2xl backdrop-saturate-150"
>
  <header
    bind:this={headerEl}
    data-tauri-drag-region
    class="flex select-none items-center justify-between gap-2 border-b border-border/60 px-3 py-2"
  >
    <span class="pointer-events-none text-[11px] font-mono text-ink-faint"
      >{id.replace('lens-', '')}</span
    >
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

  <section class="flex-1 overflow-auto scrollbar-minimal p-3">
    {#if !translation && !source && !lastError}
      <div class="flex h-full flex-col items-center justify-center gap-2 text-ink-muted">
        <Camera class="h-6 w-6 text-ink-faint" />
        <p class="text-sm">対象範囲に配置して再キャプチャ ↻</p>
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
</main>

<style>
  :global(html),
  :global(body) {
    background: transparent;
    margin: 0;
  }
</style>
