<script lang="ts">
  /**
   * Record-mode hotkey input. While focused and "listening", captures
   * the next keydown and converts it to a Tauri accelerator string
   * (e.g. "Ctrl+Shift+P"). SchemaForm mounts this when a field has the
   * JSON Schema hint `format: "hotkey"`.
   */
  import { Keyboard, X } from 'lucide-svelte';
  import { formatAccelerator, isValidAccelerator } from '$lib/hotkey';

  interface Props {
    value: string;
    onChange: (next: string) => void;
    /** Optional placeholder shown when empty and idle. */
    placeholder?: string;
  }

  let { value, onChange, placeholder = '未割当' }: Props = $props();

  let listening = $state(false);
  let buttonEl = $state<HTMLButtonElement | null>(null);

  const display = $derived(value ? formatAccelerator(value) : '');
  const invalid = $derived(!!value && !isValidAccelerator(value));

  function modifiersFromEvent(e: KeyboardEvent): string[] {
    const mods: string[] = [];
    if (e.ctrlKey) mods.push('Ctrl');
    if (e.altKey) mods.push('Alt');
    if (e.shiftKey) mods.push('Shift');
    if (e.metaKey) mods.push('Meta');
    return mods;
  }

  function keyName(e: KeyboardEvent): string | null {
    const k = e.key;
    if (k === 'Control' || k === 'Alt' || k === 'Shift' || k === 'Meta') return null;
    if (k === ' ') return 'Space';
    if (k === 'Escape') return 'Escape';
    if (k === 'Enter') return 'Return';
    if (k.length === 1) return k.toUpperCase();
    return k;
  }

  function onKey(e: KeyboardEvent) {
    if (!listening) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      // Cancel recording without overwriting.
      listening = false;
      return;
    }
    const key = keyName(e);
    if (!key) return; // still waiting for a non-modifier keystroke
    const mods = modifiersFromEvent(e);
    if (mods.length === 0) return; // bare keys aren't valid global accelerators
    const accel = [...mods, key].join('+');
    onChange(accel);
    listening = false;
  }

  function startListening() {
    listening = true;
  }

  function clear() {
    onChange('');
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="flex items-center gap-2">
  <button
    bind:this={buttonEl}
    type="button"
    onclick={startListening}
    class="inline-flex min-w-[9rem] items-center justify-between gap-2 rounded-ml-sm border border-border bg-surface px-2.5 py-1.5 text-sm transition-colors focus-visible:border-accent hover:border-accent/60"
    class:listening
    class:border-red-500={invalid}
  >
    <Keyboard class="h-3.5 w-3.5 text-ink-faint" aria-hidden="true" />
    {#if listening}
      <span class="text-xs text-accent">キーを押してください…</span>
    {:else if display}
      <kbd class="font-mono text-xs tracking-wide text-ink">{display}</kbd>
    {:else}
      <span class="text-xs text-ink-faint">{placeholder}</span>
    {/if}
  </button>

  {#if value && !listening}
    <button
      type="button"
      onclick={clear}
      aria-label="クリア"
      title="クリア"
      class="inline-flex h-6 w-6 items-center justify-center rounded-ml-sm text-ink-faint transition-colors hover:bg-surface-muted hover:text-ink"
    >
      <X class="h-3 w-3" aria-hidden="true" />
    </button>
  {/if}

  {#if invalid}
    <span class="text-xs text-red-500">無効な書式</span>
  {/if}
</div>

<style>
  button.listening {
    border-color: var(--ml-accent);
    box-shadow: 0 0 0 2px var(--ml-accent-soft);
  }
</style>
