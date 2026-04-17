<script lang="ts">
  import { Search } from 'lucide-svelte';
  let q = $state('');
  const placeholder = [
    { when: '10分前', kind: 'translate', source: 'The quick brown fox', result: '素早い茶色の狐' },
    { when: '2時間前', kind: 'qr', source: '', result: 'https://example.com/promo' }
  ];
</script>

<main class="mx-auto flex max-w-3xl flex-col gap-4 p-8">
  <header class="flex items-center justify-between">
    <h1 class="text-xl font-semibold tracking-tight">履歴</h1>
    <div class="relative">
      <Search class="pointer-events-none absolute left-2 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-faint" />
      <input
        bind:value={q}
        placeholder="検索"
        class="w-64 rounded-ml-sm border border-border bg-surface-elevated py-1.5 pl-7 pr-3 text-sm"
      />
    </div>
  </header>

  <ul class="flex flex-col gap-2">
    {#each placeholder as e}
      <li class="rounded-ml border border-border bg-surface-elevated p-3 shadow-ml">
        <div class="flex items-center justify-between text-xs text-ink-faint">
          <span>{e.when}</span>
          <span class="uppercase tracking-wide">{e.kind}</span>
        </div>
        {#if e.source}
          <p class="mt-1 font-mono text-xs text-ink-faint">{e.source}</p>
        {/if}
        <p class="mt-1 text-sm">{e.result}</p>
      </li>
    {/each}
  </ul>
</main>
