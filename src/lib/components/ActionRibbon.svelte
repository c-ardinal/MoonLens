<script lang="ts">
  import { Copy, Sparkles, Languages, Pin, ExternalLink } from 'lucide-svelte';
  import type { Intent } from '$lib/ipc';

  export type ActionId =
    | 'translate'
    | 'copy'
    | 'explain'
    | 'pin'
    | 'open_url';

  interface Props {
    /** Content intent detected by the backend. The ribbon adapts its
     *  default action order based on this so the most likely action
     *  for the captured content sits first. */
    intent?: Intent | null;
    onAction?: (id: ActionId) => void;
  }
  let { intent = null, onAction }: Props = $props();

  type Action = { id: ActionId; label: string; icon: any };

  // Base catalogue keyed by id so intent-specific lists can reuse labels.
  const all: Record<ActionId, Action> = {
    translate: { id: 'translate', label: '翻訳', icon: Languages },
    copy: { id: 'copy', label: 'コピー', icon: Copy },
    explain: { id: 'explain', label: '説明', icon: Sparkles },
    pin: { id: 'pin', label: 'ピン', icon: Pin },
    open_url: { id: 'open_url', label: 'URL を開く', icon: ExternalLink }
  };

  const order: ActionId[] = $derived.by(() => {
    switch (intent) {
      case 'qr':
        // For QR payloads the URL-opening affordance is the primary
        // action; translate is irrelevant but keep copy + explain.
        return ['open_url', 'copy', 'explain', 'pin'];
      case 'photo':
        // Explain first; translate is unlikely but cheap to offer in
        // case OCR found text the classifier missed. (No reverse image
        // search: it would send the capture to a web service.)
        return ['explain', 'translate', 'copy', 'pin'];
      case 'mixed':
      case 'text':
      default:
        return ['translate', 'copy', 'explain', 'pin'];
    }
  });

  const actions = $derived(order.map((id) => all[id]));

  function click(a: Action) {
    onAction?.(a.id);
  }
</script>

<nav class="mt-3 flex flex-wrap gap-1.5" data-intent={intent ?? 'none'}>
  {#each actions as a (a.id)}
    <button
      class="inline-flex items-center gap-1 rounded-ml-sm border border-border bg-surface px-2.5 py-1 text-[11px] text-ink-muted transition hover:border-accent/50 hover:text-ink"
      onclick={() => click(a)}
      data-action={a.id}
    >
      <a.icon class="h-3.5 w-3.5" />
      <span>{a.label}</span>
    </button>
  {/each}
</nav>
