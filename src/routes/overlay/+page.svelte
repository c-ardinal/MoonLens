<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Pin, Sparkles, X } from 'lucide-svelte';
  import PrivacyBadge from '$lib/components/PrivacyBadge.svelte';
  import ActionRibbon from '$lib/components/ActionRibbon.svelte';
  import {
    getDefaultLanguagePair,
    hideOverlay,
    onOverlay,
    pinOverlay,
    unpinOverlay,
    runExplain,
    runReverseImageSearch,
    runTranslate,
    type CaptureFlowRect,
    type Intent,
    type OverlayEvent,
    type TaskResult
  } from '$lib/ipc';
  import { currentSettings } from '$lib/stores/settingsStore';
  import { hotkeyFor } from '$lib/hotkey';

  const captureHint = $derived(hotkeyFor('capture_region', $currentSettings));
  import { extractText } from '$lib/result';

  let source = $state<string | null>(null);
  let translation = $state('');
  let viaCloud = $state(false);
  let provider = $state('');
  let pinned = $state(false);
  let status = $state<'idle' | 'processing' | 'result' | 'error'>('idle');
  let statusMessage = $state('');
  let intent = $state<Intent | null>(null);
  let sourceRect = $state<CaptureFlowRect | null>(null);
  let unlisten: (() => void) | undefined;

  onMount(async () => {
    unlisten = await onOverlay((event: OverlayEvent) => {
      if (event.kind === 'processing') {
        status = 'processing';
        statusMessage = event.message;
      } else if (event.kind === 'result') {
        status = 'result';
        source = event.source_text;
        translation = extractText(event.result);
        viaCloud = event.result.via_cloud;
        provider = event.result.used_provider;
        intent = event.intent;
        sourceRect = event.source_rect;
      } else if (event.kind === 'chunk') {
        // Ollama streaming: append deltas as they arrive so the user
        // sees tokens live. The final `Result` event will replace the
        // accumulated string with the authoritative output.
        if (status !== 'result') {
          status = 'processing';
          translation = '';
        }
        translation += event.delta;
      } else if (event.kind === 'error') {
        status = 'error';
        statusMessage = event.message;
      } else {
        status = 'idle';
      }
    });
  });

  onDestroy(() => unlisten?.());

  function copy() {
    if (translation) navigator.clipboard.writeText(translation).catch(() => {});
  }

  async function togglePin() {
    if (pinned) {
      await unpinOverlay();
      pinned = false;
    } else if (sourceRect) {
      // The pin UI stays disabled until we have a rect from the last
      // capture event, so this branch only runs when pinning is
      // actually meaningful.
      await pinOverlay(sourceRect);
      pinned = true;
    }
  }

  /** Best-effort URL extractor: if the translated / decoded payload
   * looks like a URL, return it for the open-in-browser action. */
  function detectUrl(): string | null {
    const text = translation.trim();
    if (!text) return null;
    try {
      const u = new URL(text);
      if (u.protocol === 'http:' || u.protocol === 'https:') return text;
    } catch {
      /* fall through */
    }
    const m = text.match(/https?:\/\/\S+/);
    return m ? m[0] : null;
  }

  async function onRibbonAction(
    a: 'translate' | 'copy' | 'search' | 'explain' | 'pin' | 'open_url'
  ) {
    if (a === 'copy') {
      copy();
      return;
    }
    if (a === 'pin') {
      await togglePin();
      return;
    }
    if (a === 'open_url') {
      const url = detectUrl();
      if (url) {
        // Route through the backend opener so it opens in the user's
        // default browser on every platform.
        await runReverseImageSearch(url);
      }
      return;
    }
    if (a === 'translate') {
      // Re-run translation on the currently displayed source_text.
      // Useful when the user changed target language via settings.
      if (source) {
        status = 'processing';
        statusMessage = '翻訳中…';
        try {
          // Read the user's preferred pair fresh on every re-translate
          // so a settings change between the original capture and the
          // re-translate click actually takes effect (the whole point
          // of the action is to re-run with the new target).
          const pair = await getDefaultLanguagePair();
          const r = await runTranslate({
            text: source,
            source_language: pair.source,
            target_language: pair.target
          });
          translation = extractText(r);
          viaCloud = r.via_cloud;
          provider = r.used_provider;
          status = 'result';
        } catch (e) {
          status = 'error';
          statusMessage = String(e);
        }
      }
      return;
    }
    if (a === 'explain') {
      const prompt = source
        ? `次のテキストを簡潔に説明してください:\n\n${source}`
        : `次の翻訳結果について背景を簡潔に説明してください:\n\n${translation}`;
      status = 'processing';
      statusMessage = '説明中…';
      try {
        const r = await runExplain(prompt);
        translation = extractText(r);
        viaCloud = r.via_cloud;
        provider = r.used_provider;
        status = 'result';
      } catch (e) {
        status = 'error';
        statusMessage = String(e);
      }
      return;
    }
    if (a === 'search') {
      // Reverse image search from the overlay is only useful if we
      // can produce a URL; we would need to upload the captured
      // image to get a URL first. For now, redirect the user to the
      // browser search for the source text as a graceful fallback.
      if (source) {
        const q = encodeURIComponent(source);
        await runReverseImageSearch(`https://www.google.com/search?q=${q}`);
      }
    }
  }

  async function dismiss() {
    await hideOverlay();
  }
</script>

<section
  class="pointer-events-auto m-2 max-w-[480px] rounded-ml border border-border bg-surface-elevated/90 p-4 shadow-ml-lg backdrop-blur-2xl backdrop-saturate-150"
>
  <header class="mb-2 flex items-center justify-between gap-2">
    <PrivacyBadge {viaCloud} {provider} />
    <div class="flex items-center gap-1 text-ink-faint">
      {#if status === 'processing'}
        <Sparkles class="h-3.5 w-3.5 animate-pulse" />
      {/if}
      <button
        class="rounded-ml-sm p-1 transition hover:bg-surface-muted disabled:opacity-40"
        aria-pressed={pinned}
        disabled={!sourceRect && !pinned}
        onclick={togglePin}
        title={pinned ? 'ピン解除' : 'ピン留め: 同じ領域を自動再翻訳'}
      >
        <Pin class="h-4 w-4 {pinned ? 'text-accent' : 'text-ink-faint'}" />
      </button>
      <button
        class="rounded-ml-sm p-1 transition hover:bg-surface-muted"
        onclick={dismiss}
        title="閉じる"
      >
        <X class="h-4 w-4 text-ink-faint" />
      </button>
    </div>
  </header>

  {#if status === 'processing'}
    <p class="text-sm text-ink-muted">{statusMessage || '処理中…'}</p>
  {:else if status === 'error'}
    <p class="text-sm text-red-500">{statusMessage}</p>
  {:else if status === 'result'}
    {#if source}
      <p class="font-mono text-xs leading-relaxed text-ink-faint">{source}</p>
      <hr class="my-2 border-border" />
    {/if}
    <p class="whitespace-pre-wrap text-sm leading-relaxed">{translation}</p>
    <ActionRibbon {intent} onAction={onRibbonAction} />
  {:else}
    <p class="text-sm text-ink-faint">
      {#if captureHint}{captureHint}{:else}キャプチャ翻訳{/if} でキャプチャ…
    </p>
  {/if}
</section>
