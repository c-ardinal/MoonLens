<script lang="ts">
  import { Copy, Search, Sparkles, Languages, Pin } from 'lucide-svelte';

  type ActionId = 'translate' | 'copy' | 'search' | 'explain' | 'pin';
  interface Props {
    onAction?: (id: ActionId) => void;
  }
  let { onAction }: Props = $props();

  const actions: { id: ActionId; label: string; icon: any }[] = [
    { id: 'translate', label: '翻訳', icon: Languages },
    { id: 'copy', label: 'コピー', icon: Copy },
    { id: 'search', label: '画像検索', icon: Search },
    { id: 'explain', label: '説明', icon: Sparkles },
    { id: 'pin', label: 'ピン', icon: Pin }
  ];
</script>

<nav class="mt-3 flex flex-wrap gap-1.5">
  {#each actions as a}
    <button
      class="inline-flex items-center gap-1 rounded-ml-sm border border-border bg-surface px-2.5 py-1 text-[11px] text-ink-muted transition hover:border-accent/50 hover:text-ink"
      onclick={() => onAction?.(a.id)}
      data-action={a.id}
    >
      <a.icon class="h-3.5 w-3.5" />
      <span>{a.label}</span>
    </button>
  {/each}
</nav>
