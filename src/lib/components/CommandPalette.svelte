<script lang="ts">
  import { onMount } from 'svelte';
  import { Languages, Camera, Puzzle, History } from 'lucide-svelte';

  let open = $state(false);
  let q = $state('');

  const commands = [
    { id: 'capture', label: '領域をキャプチャ', hint: '⌘⇧Space', icon: Camera },
    { id: 'translate', label: 'クリップボードを翻訳', hint: '', icon: Languages },
    { id: 'history', label: '履歴を開く', hint: '⌘⇧H', icon: History },
    { id: 'plugins', label: 'プラグインを管理', hint: '', icon: Puzzle }
  ];

  onMount(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        open = !open;
      } else if (e.key === 'Escape') {
        open = false;
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  });

  const filtered = $derived(
    commands.filter((c) => c.label.toLowerCase().includes(q.toLowerCase()))
  );
</script>

{#if open}
  <div
    class="fixed inset-0 z-50 flex items-start justify-center bg-black/30 p-16 backdrop-blur-sm"
    onclick={() => (open = false)}
    role="presentation"
  >
    <div
      role="dialog"
      class="w-full max-w-lg rounded-ml border border-border bg-surface-elevated shadow-ml-lg"
      onclick={(e) => e.stopPropagation()}
    >
      <input
        bind:value={q}
        placeholder="コマンドを検索…"
        class="w-full rounded-t-ml border-b border-border bg-transparent px-4 py-3 text-sm focus:outline-none"
        autofocus
      />
      <ul class="max-h-80 overflow-auto scrollbar-minimal">
        {#each filtered as c}
          <li>
            <button
              class="flex w-full items-center justify-between px-4 py-2 text-left text-sm hover:bg-surface-muted"
            >
              <span class="flex items-center gap-2">
                <c.icon class="h-4 w-4 text-ink-muted" />
                {c.label}
              </span>
              {#if c.hint}
                <kbd class="font-mono text-xs text-ink-faint">{c.hint}</kbd>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    </div>
  </div>
{/if}
