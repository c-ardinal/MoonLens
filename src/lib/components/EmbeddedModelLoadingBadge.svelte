<!--
  Top-right toast that surfaces while the embedded LLM model is
  loading. Fires whenever `Model::build` runs — first translation
  after launch, after a model swap, after a settings change that
  altered `model_path`. Without this, the user sees only a stalled
  translation request and has no way to tell whether the app is
  loading the LLM, hung, or crashed.

  Auto-dismisses 1.5s after a successful "finished" event so the
  badge does not linger past usefulness. Failure events stick around
  longer (5s) and surface the error so the user can act on it.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Sparkles, AlertCircle } from 'lucide-svelte';
  import { onEmbeddedModelLoading, type EmbeddedModelLoadingEvent } from '$lib/ipc';

  type Phase = 'idle' | 'loading' | 'success' | 'error';

  let phase = $state<Phase>('idle');
  let providerId = $state('');
  let errorMessage = $state<string | null>(null);
  let unlisten: (() => void) | null = null;
  let dismissTimer: ReturnType<typeof setTimeout> | null = null;

  function clearTimer() {
    if (dismissTimer) {
      clearTimeout(dismissTimer);
      dismissTimer = null;
    }
  }

  function handle(event: EmbeddedModelLoadingEvent) {
    clearTimer();
    if (event.phase === 'started') {
      phase = 'loading';
      providerId = event.provider_id;
      errorMessage = null;
      return;
    }
    providerId = event.provider_id;
    if (event.success) {
      phase = 'success';
      errorMessage = null;
      dismissTimer = setTimeout(() => {
        phase = 'idle';
      }, 1500);
    } else {
      phase = 'error';
      errorMessage = event.error;
      dismissTimer = setTimeout(() => {
        phase = 'idle';
      }, 5000);
    }
  }

  onMount(async () => {
    try {
      unlisten = await onEmbeddedModelLoading(handle);
    } catch (e) {
      console.warn('embedded loading listener failed:', e);
    }
  });

  onDestroy(() => {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
    clearTimer();
  });
</script>

{#if phase !== 'idle'}
  <div
    class="pointer-events-none fixed right-4 top-12 z-40 flex items-center gap-2 rounded-ml border border-border bg-surface-elevated px-3 py-2 text-xs shadow-ml"
    role="status"
    aria-live="polite"
  >
    {#if phase === 'loading'}
      <span class="ml-spinner" aria-hidden="true"></span>
      <span class="text-ink">ローカル LLM をロード中…</span>
      {#if providerId}
        <span class="text-ink-faint">({providerId})</span>
      {/if}
    {:else if phase === 'success'}
      <Sparkles class="h-3.5 w-3.5 text-accent" aria-hidden="true" />
      <span class="text-ink">ローカル LLM 準備完了</span>
    {:else if phase === 'error'}
      <AlertCircle class="h-3.5 w-3.5 text-red-500" aria-hidden="true" />
      <span class="text-ink">
        ロード失敗{#if errorMessage}: {errorMessage}{/if}
      </span>
    {/if}
  </div>
{/if}

<style>
  .ml-spinner {
    width: 0.875rem;
    height: 0.875rem;
    border: 2px solid var(--accent-soft);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: ml-spinner-rot 0.8s linear infinite;
  }

  @keyframes ml-spinner-rot {
    to {
      transform: rotate(360deg);
    }
  }
</style>
