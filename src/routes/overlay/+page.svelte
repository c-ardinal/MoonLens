<script lang="ts">
  import { onMount } from 'svelte';
  import { Lock, Cloud, Cpu, Copy, Search, Pin, Sparkles } from 'lucide-svelte';
  import PrivacyBadge from '$lib/components/PrivacyBadge.svelte';
  import ActionRibbon from '$lib/components/ActionRibbon.svelte';

  let source = $state('');
  let translation = $state('');
  let viaCloud = $state(false);
  let provider = $state('');
  let pinned = $state(false);
  let streaming = $state(false);

  onMount(() => {
    /* In production the backend pushes results via Tauri events. */
  });
</script>

<section
  class="pointer-events-auto m-3 max-w-[var(--ml-max-w,480px)] rounded-ml border border-border bg-surface-elevated/80 p-4 shadow-ml backdrop-blur-2xl backdrop-saturate-150"
  style="--ml-max-w: 480px"
>
  <header class="mb-2 flex items-center justify-between gap-2">
    <PrivacyBadge {viaCloud} {provider} />
    <div class="flex items-center gap-1 text-ink-faint">
      {#if streaming}
        <Sparkles class="h-3.5 w-3.5 animate-pulse" />
      {/if}
      <button
        class="rounded-ml-sm p-1 hover:bg-surface-muted"
        aria-pressed={pinned}
        onclick={() => (pinned = !pinned)}
        title="ピン留め"
      >
        <Pin class="h-4 w-4 {pinned ? 'text-accent' : 'text-ink-faint'}" />
      </button>
    </div>
  </header>

  {#if source}
    <p class="text-xs font-mono leading-relaxed text-ink-faint">{source}</p>
    <hr class="my-2 border-border" />
  {/if}
  <p class="text-sm leading-relaxed whitespace-pre-wrap">
    {translation || '領域を選択してください…'}
  </p>

  <ActionRibbon />
</section>
