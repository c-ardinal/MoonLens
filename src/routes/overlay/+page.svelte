<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Pin, Sparkles, X } from 'lucide-svelte';
  import PrivacyBadge from '$lib/components/PrivacyBadge.svelte';
  import ActionRibbon from '$lib/components/ActionRibbon.svelte';
  import { hideOverlay, onOverlay, type OverlayEvent, type TaskResult } from '$lib/ipc';
  import { extractText } from '$lib/result';

  let source = $state<string | null>(null);
  let translation = $state('');
  let viaCloud = $state(false);
  let provider = $state('');
  let pinned = $state(false);
  let status = $state<'idle' | 'processing' | 'result' | 'error'>('idle');
  let statusMessage = $state('');
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
        class="rounded-ml-sm p-1 transition hover:bg-surface-muted"
        aria-pressed={pinned}
        onclick={() => (pinned = !pinned)}
        title="ピン留め"
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
    <ActionRibbon onAction={(a) => a === 'copy' && copy()} />
  {:else}
    <p class="text-sm text-ink-faint">⌘⇧Space でキャプチャ…</p>
  {/if}
</section>
