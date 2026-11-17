<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getSettings, onSettingsChanged, type Settings } from '$lib/ipc';

  type SubtitleEvent =
    | { kind: 'started' }
    | { kind: 'stopped' }
    | { kind: 'chunk'; original: string; translated: string }
    | { kind: 'error'; message: string };

  let original = $state('');
  let translated = $state('');
  let status = $state<'idle' | 'listening' | 'error'>('idle');
  let unlisten: UnlistenFn | undefined;
  let unlistenSettings: UnlistenFn | undefined;
  // Settings → ライブ字幕.
  let showOriginal = $state(true);
  let fontSize = $state(22);

  function applySettings(s: Settings) {
    showOriginal = s?.subtitle?.show_original ?? true;
    fontSize = s?.subtitle?.font_size_px ?? 22;
  }

  onMount(async () => {
    getSettings().then(applySettings).catch(() => {});
    unlistenSettings = await onSettingsChanged(applySettings);
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
  onDestroy(() => {
    unlisten?.();
    unlistenSettings?.();
  });
</script>

<section
  class="pointer-events-none fixed inset-x-0 bottom-0 mx-auto max-w-3xl p-4"
  aria-live="polite"
>
  <div class="rounded-ml bg-black/70 px-5 py-3 text-center text-white shadow-ml backdrop-blur-xl">
    {#if original && showOriginal}
      <p class="opacity-70" style="font-size: {Math.round(fontSize * 0.55)}px">{original}</p>
    {/if}
    <p class="mt-1 font-semibold leading-snug break-words" style="font-size: {fontSize}px">
      {#if status === 'idle'}
        音声入力待機中…
      {:else if status === 'error'}
        エラー：{translated}
      {:else}
        {translated || '音声認識中…'}
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
