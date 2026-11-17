<script lang="ts">
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
  aria-label="{label}のショートカット"
  onclick={() => (recording = !recording)}
  onkeydown={keydown}
  onblur={() => (recording = false)}
>
  {#if recording}
    <span class="font-sans text-xs text-ink-muted">キー入力待ち（Esc：取消／Backspace：無効化）</span>
  {:else if value}
    {displayAccelerator(value)}
  {:else}
    <span class="font-sans text-xs text-ink-faint">未設定</span>
  {/if}
</button>
