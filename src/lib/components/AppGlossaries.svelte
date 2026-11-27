<script lang="ts">
  import { Plus, Trash2 } from 'lucide-svelte';
  import { addApp, setGlossary, type AppGlossary } from '$lib/app-glossary';
  import { t } from '$lib/i18n';
  import GlossaryEditor from './GlossaryEditor.svelte';

  interface Props {
    label: string;
    value: AppGlossary[] | null | undefined;
    /** 追加候補（実行中アプリの実行ファイル名）。 */
    suggestions?: string[];
    /** Language a new entry starts with (the configured target). */
    defaultLang: string;
    onChange: (v: AppGlossary[]) => void;
    /** 行の削除。取消（通知）は呼出し側の責務：本部品の破棄後も有効。 */
    onRemoveRow: (app: string, index: number) => void;
    /** アプリ単位の削除。取消は呼出し側の責務。 */
    onRemoveApp: (app: string) => void;
  }
  let { label, value, suggestions = [], defaultLang, onChange, onRemoveRow, onRemoveApp }: Props = $props();

  const apps = $derived(Array.isArray(value) ? value : []);
  let chosen = $state<string | null>(null);
  /** 編集対象：選択中のアプリ、削除後は先頭。 */
  const current = $derived(apps.find((a) => a.app === chosen) ?? apps[0] ?? null);
  let name = $state('');
  let rejected = $state(false);

  const input =
    'min-w-0 rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm placeholder:text-ink-faint focus:border-accent/60 focus:outline-none';

  function add() {
    const { list, key } = addApp(apps, name);
    rejected = key === null;
    if (key === null) return;
    if (list !== apps) onChange(list);
    chosen = key;
    name = '';
  }
</script>

<div class="flex flex-col gap-2 py-3">
  <p class="row-label min-w-0 text-sm font-medium text-ink">{label}</p>
  <div class="mr-10 flex flex-wrap items-center gap-2">
    {#if current}
      <select
        class="{input} flex-[1_1_10rem] px-1"
        value={current.app}
        aria-label={t('app-glossaries:target_app')}
        onchange={(ev) => (chosen = (ev.currentTarget as HTMLSelectElement).value)}
      >
        {#each apps as a (a.app)}
          <option value={a.app}>{t('app-glossaries:app_with_count', { name: a.name || a.app, n: a.glossary.length })}</option>
        {/each}
      </select>
      <button
        type="button"
        class="row-value rounded-ml-sm p-1 text-ink-faint transition hover:bg-surface-muted hover:text-ink"
        title={t('app-glossaries:delete_app_glossary')}
        aria-label={t('app-glossaries:delete_glossary_of', { name: current.name || current.app })}
        onclick={() => onRemoveApp(current.app)}
      >
        <Trash2 class="h-3.5 w-3.5" />
      </button>
    {:else}
      <p class="row-value min-w-0 flex-[1_1_10rem] rounded-ml-sm border border-dashed border-border px-3 py-1 text-xs text-ink-faint">
        {t('common:no_entries')}
      </p>
    {/if}
    <form
      class="flex min-w-0 flex-[1_1_14rem] items-center gap-2"
      onsubmit={(e) => {
        e.preventDefault();
        add();
      }}
    >
      <input
        class="{input} w-0 flex-1 {rejected ? 'border-red-500' : ''}"
        bind:value={name}
        list="app-glossary-candidates"
        placeholder={t('app-glossaries:exe_example')}
        aria-label={t('app-glossaries:exe_name')}
        aria-invalid={rejected}
        oninput={() => (rejected = false)}
      />
      <datalist id="app-glossary-candidates">
        {#each suggestions as s (s)}
          <option value={s}></option>
        {/each}
      </datalist>
      <button
        type="submit"
        class="inline-flex shrink-0 items-center gap-1 rounded-ml-sm border border-border bg-surface px-2.5 py-1 text-sm text-ink"
      >
        <Plus class="h-3.5 w-3.5" />{t('app-glossaries:add_app')}
      </button>
    </form>
  </div>

  {#if current}
    {#key current.app}
      <GlossaryEditor
        label={t('app-glossaries:terms_for', { name: current.name || current.app })}
        value={current.glossary}
        {defaultLang}
        onChange={(v) => onChange(setGlossary(apps, current.app, v))}
        onRemove={(i) => onRemoveRow(current.app, i)}
      />
    {/key}
  {/if}
</div>
