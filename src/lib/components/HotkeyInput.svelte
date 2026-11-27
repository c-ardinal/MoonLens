<script lang="ts">
  import { t } from '$lib/i18n';
  import { acceleratorFromEvent, displayAccelerator } from '$lib/hotkey';

  interface Props {
    value: string;
    label: string;
    onChange: (accel: string) => void;
  }
  let { value, label, onChange }: Props = $props();

  let recording = $state(false);

  function keydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      recording = false;
      return;
    }
    if ((e.key === 'Backspace' || e.key === 'Delete') && !e.ctrlKey && !e.metaKey && !e.altKey) {
      onChange('');
      recording = false;
      return;
    }
    const accel = acceleratorFromEvent(e);
    // A global shortcut needs a modifier, or it would swallow normal typing.
    if (!accel || !accel.includes('+')) return;
    onChange(accel);
    recording = false;
  }
</script>

<button
  type="button"
  class="inline-flex min-h-8 min-w-36 max-w-full items-center justify-center rounded-ml-sm border px-3 py-1 font-mono text-sm transition {recording
    ? 'border-accent bg-accent-soft text-ink'
    : 'border-border bg-surface text-ink hover:border-accent/60'}"
  aria-label={t('hotkey-input:shortcut_for_label', { label })}
  onclick={() => (recording = !recording)}
  onkeydown={keydown}
  onblur={() => (recording = false)}
>
  {#if recording}
    <span class="font-sans text-xs text-ink-muted">{t('hotkey-input:press_keys_esc_cancel_backspace_disable')}</span>
  {:else if value}
    {displayAccelerator(value)}
  {:else}
    <span class="font-sans text-xs text-ink-faint">{t('common:not_set')}</span>
  {/if}
</button>
