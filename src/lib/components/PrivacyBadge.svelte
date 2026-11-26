<script lang="ts">
  import { Cloud, Cpu } from 'lucide-svelte';

  interface Props {
    viaCloud: boolean;
    provider: string;
  }
  let { viaCloud, provider }: Props = $props();
</script>

<span
  class="inline-flex items-center gap-1 rounded-full px-2 py-1 text-[11px] font-medium"
  class:bg-surface-muted={!viaCloud}
  class:text-ink-muted={!viaCloud}
  class:bg-accent-soft={viaCloud}
  class:text-accent={viaCloud}
  title={provider}
>
  {#if viaCloud}
    <Cloud class="h-3 w-3" />
    <span>Cloud</span>
  {:else}
    <Cpu class="h-3 w-3" />
    <span>Local</span>
  {/if}
</span>
