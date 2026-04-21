<script lang="ts">
  /**
   * Status dot for the auto-save flow. Shows one of three states:
   *   - "saved"    : steady check icon
   *   - "dirty"    : pulsing dot (unsaved edits pending debounce)
   *   - "saving"   : spinner
   *   - "error"    : red alert with hover message
   */
  import { Check, Loader2, AlertCircle, Circle } from 'lucide-svelte';

  export type SaveState = 'saved' | 'dirty' | 'saving' | 'error';

  interface Props {
    state: SaveState;
    /** Error message shown as title when state === 'error'. */
    errorMessage?: string | null;
  }

  let { state, errorMessage }: Props = $props();
</script>

<div
  class="inline-flex items-center gap-1.5 rounded-ml-sm border border-border bg-surface px-2.5 py-1 text-xs"
  title={state === 'error' ? errorMessage ?? '保存エラー' : undefined}
  aria-live="polite"
>
  {#if state === 'saved'}
    <Check class="h-3.5 w-3.5 text-green-500" aria-hidden="true" />
    <span class="text-ink-muted">保存済み</span>
  {:else if state === 'dirty'}
    <Circle
      class="h-3.5 w-3.5 animate-pulse fill-accent text-accent"
      aria-hidden="true"
    />
    <span class="text-ink-muted">未保存…</span>
  {:else if state === 'saving'}
    <Loader2 class="h-3.5 w-3.5 animate-spin text-accent" aria-hidden="true" />
    <span class="text-ink-muted">保存中…</span>
  {:else}
    <AlertCircle class="h-3.5 w-3.5 text-red-500" aria-hidden="true" />
    <span class="text-red-500">保存失敗</span>
  {/if}
</div>
