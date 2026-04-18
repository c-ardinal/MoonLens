<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';

  type SubtitleEvent =
    | { kind: 'started' }
    | { kind: 'stopped' }
    | { kind: 'chunk'; original: string; translated: string }
    | { kind: 'error'; message: string };

  let original = $state('');
  let translated = $state('');
  let status = $state<'idle' | 'listening' | 'error'>('idle');
  let unlisten: UnlistenFn | undefined;

  onMount(async () => {
    unlisten = await listen<SubtitleEvent>('moonlens://subtitle', (e) => {
      const ev = e.payload;
      if (ev.kind === 'started') status = 'listening';
      else if (ev.kind === 'stopped') status = 'idle';
      else if (ev.kind === 'chunk') {
        original = ev.original;
        translated = ev.translated || ev.original;
      } else if (ev.kind === 'error') {
        status = 'error';
        translated = ev.message;
      }
    });
  });
  onDestroy(() => unlisten?.());
</script>

<section
  class="pointer-events-none fixed inset-x-0 bottom-0 mx-auto max-w-3xl p-4"
  aria-live="polite"
>
  <div class="rounded-ml bg-black/70 px-5 py-3 text-center text-white shadow-ml backdrop-blur-xl">
    {#if original}
      <p class="text-xs opacity-70">{original}</p>
    {/if}
    <p class="mt-1 text-xl font-semibold leading-snug">
      {#if status === 'idle'}
        音声ループバック待機中…
      {:else if status === 'error'}
        エラー: {translated}
      {:else}
        {translated || '音声を聴き取り中…'}
      {/if}
    </p>
  </div>
</section>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
</style>
