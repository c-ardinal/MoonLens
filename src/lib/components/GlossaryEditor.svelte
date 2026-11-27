<script lang="ts">
  import { t } from '$lib/i18n';
  import { tick } from 'svelte';
  import { Plus, X } from 'lucide-svelte';
  import { LANGUAGES } from '$lib/lens-overlay';
  import { addEntry, updateEntry, type GlossaryEntry } from '$lib/glossary';

  interface Props {
    label: string;
    value: GlossaryEntry[] | null | undefined;
    /** Language a new entry starts with (the configured target). */
    defaultLang: string;
    onChange: (v: GlossaryEntry[]) => void;
    /** 行の削除。取消（通知）は呼出し側の責務：本部品の破棄後も有効。 */
    onRemove: (index: number) => void;
  }
  let { label, value, defaultLang, onChange, onRemove }: Props = $props();

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
    <!-- 説明文は設定画面の説明欄へ委譲。row-label／row-value：選択帯上で黒へ反転 -->
    <p class="row-label min-w-0 text-sm font-medium text-ink">{label}</p>
    <button
      type="button"
      class="mr-10 inline-flex shrink-0 items-center gap-1 rounded-ml-sm border border-border bg-surface px-2.5 py-1 text-sm text-ink"
      onclick={add}
    >
      <Plus class="h-3.5 w-3.5" />{t('追加')}
    </button>
  </div>

  {#if entries.length === 0}
    <p class="rounded-ml-sm border border-dashed border-border px-3 py-2 text-xs text-ink-faint row-value">{t('登録なし')}</p>
  {:else}
    <div class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_7.5rem_1.75rem] items-center gap-x-2 gap-y-1.5">
      <span class="row-value text-xs text-ink-faint">{t('原語')}</span>
      <span class="row-value text-xs text-ink-faint">{t('訳語')}</span>
      <span class="row-value text-xs text-ink-faint">{t('翻訳先')}</span>
      <span></span>
      {#each entries as e, i (i)}
        <input
          data-glossary-source
          class={input}
          value={e.source}
          placeholder={t('例：Dashboard')}
          aria-label={t('原語 {n}', { n: i + 1 })}
          oninput={(ev) => onChange(updateEntry(entries, i, { source: (ev.currentTarget as HTMLInputElement).value }))}
        />
        <input
          class={input}
          value={e.target}
          placeholder={t('例：管理画面')}
          aria-label={t('訳語 {n}', { n: i + 1 })}
          oninput={(ev) => onChange(updateEntry(entries, i, { target: (ev.currentTarget as HTMLInputElement).value }))}
        />
        <select
          class="{input} px-1"
          value={e.lang ?? ''}
          aria-label={t('翻訳先 {n}', { n: i + 1 })}
          onchange={(ev) => onChange(updateEntry(entries, i, { lang: (ev.currentTarget as HTMLSelectElement).value }))}
        >
          <option value="">{t('全言語')}</option>
          {#if e.lang && !LANGUAGES.some((l) => l.code === e.lang)}
            <option value={e.lang}>{e.lang}</option>
          {/if}
          {#each LANGUAGES as l (l.code)}
            <option value={l.code}>{t(l.label)}</option>
          {/each}
        </select>
        <button
          type="button"
          class="row-value rounded-ml-sm p-1 text-ink-faint transition hover:bg-surface-muted hover:text-ink"
          title={t('削除')}
          aria-label={t('用語 {n} を削除', { n: i + 1 })}
          onclick={() => onRemove(i)}
        >
          <X class="h-3.5 w-3.5" />
        </button>
      {/each}
    </div>
  {/if}
</div>
