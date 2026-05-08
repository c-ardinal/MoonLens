<script lang="ts">
  /**
   * Pill / badge primitive. `accent` flips it to the soft accent
   * palette; `dot` prepends a small status dot in the current colour
   * so the pill doubles as a "live / paused" indicator.
   */
  import type { Snippet } from 'svelte';

  interface Props {
    accent?: boolean;
    dot?: boolean;
    title?: string;
    children?: Snippet;
  }

  let { accent = false, dot = false, title, children }: Props = $props();
</script>

<span
  class="pill"
  class:accent
  class:dot
  {title}
  data-testid="pill"
>
  {#if children}{@render children()}{/if}
</span>
