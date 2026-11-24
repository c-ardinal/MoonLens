<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import type { LensBlock } from '$lib/ipc';
  import { rgb } from '$lib/lens-overlay';
  import FlowText from '$lib/components/FlowText.svelte';
  import { liveBoxes, type LiveEvent } from '$lib/live';

  // Live translation window. It sits exactly over the pinned region,
  // ignores the mouse and is excluded from screen capture; the backend
  // reads the region underneath and sends the blocks to paint here.

  let blocks = $state<LensBlock[]>([]);
  let hidden = $state<Set<number>>(new Set());
  let dpr = $state(typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1);
  let unlisten: UnlistenFn | undefined;
  const onResize = () => (dpr = window.devicePixelRatio || 1);

  function apply(ev: LiveEvent) {
    if (ev.kind === 'blocks') {
      blocks = ev.blocks;
      hidden = new Set();
    } else if (ev.kind === 'stale') {
      hidden = new Set([...hidden, ...ev.indices]);
    } else if (ev.kind === 'clear') {
      blocks = [];
      hidden = new Set();
    }
  }

  onMount(async () => {
    window.addEventListener('resize', onResize);
    try {
      unlisten = await listen<LiveEvent>('moonlens://live', (e) => apply(e.payload));
    } catch {
      /* not running inside Tauri (pure frontend dev) */
    }
  });
  onDestroy(() => {
    unlisten?.();
    window.removeEventListener('resize', onResize);
  });

  const boxes = $derived(liveBoxes(blocks, hidden, dpr, window.innerWidth * dpr));
</script>

<div class="live" aria-hidden="true">
  {#each boxes as { block: b, rect: r, pending, grow } (r.key)}
    <div
      class="box"
      class:pending
      style="left: {r.left}px; top: {r.top}px; width: {r.width}px; height: {r.height}px;
             background: {rgb(b.colors?.background, 'rgba(255,255,255,0.96)')};
             color: {rgb(b.colors?.foreground, '#111')};"
    >
      {#if b.translation != null}
        <FlowText
          block={b}
          text={b.translation}
          scale={dpr}
          {grow}
          background={rgb(b.colors?.background, 'rgba(255,255,255,0.96)')}
        />
      {/if}
    </div>
  {/each}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
    padding: 0;
    overflow: hidden;
  }
  .live {
    position: fixed;
    inset: 0;
    pointer-events: none;
    user-select: none;
  }
  .box {
    position: absolute;
    border-radius: 3px;
  }
  .box.pending {
    /* Hide the source under the box while its translation is on the way. */
    background-image: linear-gradient(
      100deg,
      transparent 30%,
      rgba(139, 92, 246, 0.18) 50%,
      transparent 70%
    ) !important;
    background-size: 250% 100%;
    animation: scan 1.1s linear infinite;
  }
  @keyframes scan {
    from {
      background-position: 120% 0;
    }
    to {
      background-position: -120% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .box.pending {
      animation: none;
    }
  }
</style>
