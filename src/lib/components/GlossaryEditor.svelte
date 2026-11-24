<script lang="ts">
  import { tick } from 'svelte';
  import { Plus, X } from 'lucide-svelte';
  import { LANGUAGES } from '$lib/lens-overlay';
  import { addEntry, removeEntry, updateEntry, type GlossaryEntry } from '$lib/glossary';

  interface Props {
    label: string;
    help?: string;
    value: GlossaryEntry[] | null | undefined;
    /** Language a new entry starts with (the configured target). */
    defaultLang: string;
    onChange: (v: GlossaryEntry[]) => void;
  }
  let { label, help, value, defaultLang, onChange }: Props = $props();

  const entries = $derived(Array.isArray(value) ? value : []);
  const input =
    'min-w-0 w-full rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm placeholder:text-ink-faint focus:border-accent/60 focus:outline-none';

  async function add() {
    onChange(addEntry(entries, defaultLang));
    // Focus the new row's first box once it is rendered.
    await tick();
    requestAnimationFrame(() => {
      const boxes = document.querySelectorAll<HTMLInputElement>('[data-glossary-source]');
      boxes[boxes.length - 1]?.focus();
    });
  }
</script>

<div class="flex flex-col gap-2 py-3">
  <div class="flex items-start justify-between gap-4">
    <div class="min-w-0">
      <p class="text-sm font-medium text-ink">{label}</p>
      {#if help}<p class="mt-0.5 text-xs leading-relaxed text-ink-muted">{help}</p>{/if}
    </div>
    <button
      type="button"
      class="mr-10 inline-flex shrink-0 items-center gap-1 rounded-ml-sm border border-border bg-surface px-2.5 py-1 text-sm text-ink-muted hover:text-ink"
      onclick={add}
    >
      <Plus class="h-3.5 w-3.5" />追加
    </button>
  </div>

  {#if entries.length === 0}
    <p class="rounded-ml-sm border border-dashed border-border px-3 py-2 text-xs text-ink-faint">登録なし</p>
  {:else}
    <div class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_7.5rem_1.75rem] items-center gap-x-2 gap-y-1.5">
      <span class="text-xs text-ink-faint">原語</span>
      <span class="text-xs text-ink-faint">訳語</span>
      <span class="text-xs text-ink-faint">翻訳先</span>
      <span></span>
      {#each entries as e, i (i)}
        <input
          data-glossary-source
          class={input}
          value={e.source}
          placeholder="例：Dashboard"
          aria-label="原語 {i + 1}"
          oninput={(ev) => onChange(updateEntry(entries, i, { source: (ev.currentTarget as HTMLInputElement).value }))}
        />
        <input
          class={input}
          value={e.target}
          placeholder="例：管理画面"
          aria-label="訳語 {i + 1}"
          oninput={(ev) => onChange(updateEntry(entries, i, { target: (ev.currentTarget as HTMLInputElement).value }))}
        />
        <select
          class="{input} px-1"
          value={e.lang ?? ''}
          aria-label="翻訳先 {i + 1}"
          onchange={(ev) => onChange(updateEntry(entries, i, { lang: (ev.currentTarget as HTMLSelectElement).value }))}
        >
          <option value="">全言語</option>
          {#if e.lang && !LANGUAGES.some((l) => l.code === e.lang)}
            <option value={e.lang}>{e.lang}</option>
          {/if}
          {#each LANGUAGES as l (l.code)}
            <option value={l.code}>{l.label}</option>
          {/each}
        </select>
        <button
          type="button"
          class="rounded-ml-sm p-1 text-ink-faint transition hover:bg-surface-muted hover:text-ink"
          title="削除"
          aria-label="用語 {i + 1} を削除"
          onclick={() => onChange(removeEntry(entries, i))}
        >
          <X class="h-3.5 w-3.5" />
        </button>
      {/each}
    </div>
  {/if}
</div>
