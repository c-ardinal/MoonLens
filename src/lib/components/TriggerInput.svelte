<script module lang="ts">
  import { triggerSupported } from '$lib/ipc';

  // One question for all fields of the screen.
  let supportedOnce: Promise<boolean> | undefined;
  const askSupported = () => (supportedOnce ??= triggerSupported().catch(() => false));
</script>

<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { t } from '$lib/i18n';
  import { cancelTriggerCapture, captureTrigger } from '$lib/ipc';
  import { displayTrigger } from '$lib/trigger';

  interface Props {
    value: string;
    label: string;
    onChange: (trigger: string) => void;
  }
  let { value, label, onChange }: Props = $props();

  let supported = $state(false);
  let recording = $state(false);
  /** Counts recordings: a result that arrives after a cancel is dropped. */
  let run = 0;

  onMount(() => {
    askSupported().then((ok) => (supported = ok));
  });
  onDestroy(stop);

  async function start() {
    const id = ++run;
    recording = true;
    const got = await captureTrigger().catch(() => null);
    if (id !== run) return;
    recording = false;
    if (got) onChange(got);
  }

  function stop() {
    if (!recording) return;
    run++;
    recording = false;
    void cancelTriggerCapture();
  }

  function keydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') stop();
    else if (e.key === 'Backspace' || e.key === 'Delete') {
      stop();
      onChange('');
    }
  }

  // While recording, the middle and side buttons must not scroll, move
  // the focus or go back a page: the backend is reading them.
  function swallow(e: MouseEvent) {
    if (recording && e.button !== 0 && e.button !== 2) e.preventDefault();
  }
</script>

<svelte:window onmousedown={swallow} onmouseup={swallow} onauxclick={swallow} />

<button
  type="button"
  class="inline-flex min-h-8 min-w-36 max-w-full items-center justify-center rounded-ml-sm border px-3 py-1 text-sm transition {recording
    ? 'border-accent bg-accent-soft text-ink'
    : 'border-border bg-surface text-ink hover:border-accent/60'} disabled:cursor-not-allowed disabled:opacity-60"
  aria-label={t('trigger-input:trigger_for_label', { label })}
  disabled={!supported}
  onclick={() => (recording ? stop() : start())}
  onkeydown={keydown}
  onblur={stop}
>
  {#if recording}
    <span class="text-xs text-ink-muted">{t('trigger-input:waiting_for_input_esc_cancel_backspace_clear')}</span>
  {:else if value}
    {displayTrigger(value)}
  {:else if supported}
    <span class="text-xs text-ink-faint">{t('common:not_set')}</span>
  {:else}
    <span class="text-xs text-ink-faint">{t('trigger-input:windows_only')}</span>
  {/if}
</button>
