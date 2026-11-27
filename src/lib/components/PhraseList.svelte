<script lang="ts">
  import { t } from '$lib/i18n';
  import { tick } from 'svelte';
  import { Plus, X } from 'lucide-svelte';

  // 文の一覧の編集（音声翻訳の遮断文）。空行は保存対象外（settings-meta の savable）。

  interface Props {
    label: string;
    value: string[] | null | undefined;
    placeholder?: string;
    onChange: (v: string[]) => void;
  }
  let { label, value, placeholder = '', onChange }: Props = $props();

  const rows = $derived(Array.isArray(value) ? value : []);
  let list = $state<HTMLElement>();
  const input =
    'min-w-0 w-full rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm placeholder:text-ink-faint focus:border-accent/60 focus:outline-none';

  async function add() {
    onChange([...rows, '']);
    // 描画後、追加行へフォーカス。
    await tick();
    requestAnimationFrame(() => {
      const boxes = list?.querySelectorAll<HTMLInputElement>('input');
      boxes?.[boxes.length - 1]?.focus();
    });
  }
</script>

<div class="flex flex-col gap-2 py-3">
  <div class="flex items-start justify-between gap-4">
    <p class="row-label min-w-0 text-sm font-medium text-ink">{label}</p>
    <button
      type="button"
      class="mr-10 inline-flex shrink-0 items-center gap-1 rounded-ml-sm border border-border bg-surface px-2.5 py-1 text-sm text-ink"
      onclick={add}
    >
      <Plus class="h-3.5 w-3.5" />{t('追加')}
    </button>
  </div>

  {#if rows.length === 0}
    <p class="rounded-ml-sm border border-dashed border-border px-3 py-2 text-xs text-ink-faint row-value">{t('登録なし')}</p>
  {:else}
    <div bind:this={list} class="grid grid-cols-[minmax(0,1fr)_1.75rem] items-center gap-x-2 gap-y-1.5">
      {#each rows as phrase, i (i)}
        <input
          class={input}
          value={phrase}
          {placeholder}
          aria-label="{label} {i + 1}"
          oninput={(ev) => onChange(rows.map((r, j) => (j === i ? (ev.currentTarget as HTMLInputElement).value : r)))}
        />
        <button
          type="button"
          class="row-value rounded-ml-sm p-1 text-ink-faint transition hover:bg-surface-muted hover:text-ink"
          title={t('削除')}
          aria-label={t('{label} {n} を削除', { label, n: i + 1 })}
          onclick={() => onChange(rows.filter((_, j) => j !== i))}
        >
          <X class="h-3.5 w-3.5" />
        </button>
      {/each}
    </div>
  {/if}
</div>
