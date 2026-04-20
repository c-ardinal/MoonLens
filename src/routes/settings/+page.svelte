<script lang="ts">
  import { onMount } from 'svelte';
  import { getSettings, getSettingsSchema, setSettings } from '$lib/ipc';
  import SchemaForm from '$lib/components/SchemaForm.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import AppHeader from '$lib/components/AppHeader.svelte';

  let schema = $state<any>(null);
  let settings = $state<any>(null);
  let dirty = $state(false);
  let query = $state('');

  onMount(async () => {
    try {
      schema = await getSettingsSchema();
      settings = await getSettings();
    } catch (e) {
      console.warn('Backend unavailable in pure frontend dev mode', e);
    }
  });

  async function save() {
    if (!settings) return;
    await setSettings(settings);
    dirty = false;
  }
</script>

<main class="mx-auto flex max-w-4xl flex-col gap-6 p-8">
  <AppHeader title="設定" subtitle="すべての挙動を設定ファイルから制御できます。">
    {#snippet actions()}
      <input
        bind:value={query}
        placeholder="項目を検索"
        class="w-56 rounded-ml-sm border border-border bg-surface-elevated px-3 py-1.5 text-sm placeholder:text-ink-faint focus:border-accent/60 focus:outline-none"
      />
      <button
        class="rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-40"
        onclick={save}
        disabled={!dirty}
      >
        保存
      </button>
    {/snippet}
  </AppHeader>

  {#if schema && settings}
    <SchemaForm {schema} bind:value={settings} {query} onChange={() => (dirty = true)} />
  {:else}
    <p class="text-ink-faint">読み込み中…</p>
  {/if}

  <CommandPalette />
</main>
