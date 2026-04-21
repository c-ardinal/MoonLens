<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Search, RotateCcw } from 'lucide-svelte';
  import { getSettings, getSettingsSchema, setSettings } from '$lib/ipc';
  import SchemaForm from '$lib/components/SchemaForm.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import SaveIndicator, { type SaveState } from '$lib/components/SaveIndicator.svelte';

  let schema = $state<any>(null);
  let settings = $state<any>(null);
  let initialSettings = $state<any>(null);
  let query = $state('');
  let saveState = $state<SaveState>('saved');
  let errorMessage = $state<string | null>(null);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let confirmReset = $state(false);

  onMount(async () => {
    try {
      schema = await getSettingsSchema();
      const current = await getSettings();
      settings = current;
      initialSettings = structuredClone(current);
    } catch (e) {
      errorMessage = `設定読み込み失敗: ${String(e)}`;
      saveState = 'error';
    }
  });

  onDestroy(() => {
    if (saveTimer) clearTimeout(saveTimer);
  });

  function scheduleSave() {
    saveState = 'dirty';
    if (saveTimer) clearTimeout(saveTimer);
    // 500ms debounce: long enough to coalesce keystrokes, short enough
    // that the user sees the "保存済み" flip while they're still
    // watching the form.
    saveTimer = setTimeout(save, 500);
  }

  async function save() {
    if (!settings) return;
    saveState = 'saving';
    errorMessage = null;
    try {
      await setSettings(structuredClone(settings));
      initialSettings = structuredClone(settings);
      saveState = 'saved';
    } catch (e) {
      errorMessage = String(e);
      saveState = 'error';
    }
  }

  async function resetAll() {
    if (!initialSettings) return;
    // For MVP, "restore defaults" reverts to the snapshot loaded on
    // mount; wiring to a true schema-default reset will follow once
    // the backend exposes a default_settings command.
    settings = structuredClone(initialSettings);
    confirmReset = false;
    saveState = 'dirty';
    scheduleSave();
  }
</script>

<div class="flex h-full flex-col overflow-hidden">
  <header
    class="flex shrink-0 items-center justify-between gap-3 border-b border-border bg-surface px-6 py-4"
  >
    <div>
      <h1 class="text-lg font-semibold tracking-tight">設定</h1>
      <p class="text-xs text-ink-faint">
        変更は自動で保存され、カテゴリによって即反映または次サイクル反映です。
      </p>
    </div>
    <div class="flex items-center gap-2">
      <div class="relative">
        <Search
          class="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-ink-faint"
          aria-hidden="true"
        />
        <input
          bind:value={query}
          placeholder="項目を検索"
          class="w-56 rounded-ml-sm border border-border bg-surface-elevated py-1.5 pl-7 pr-3 text-sm placeholder:text-ink-faint focus-visible:border-accent focus-visible:outline-none"
        />
      </div>
      <button
        type="button"
        onclick={() => (confirmReset = true)}
        class="inline-flex items-center gap-1 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-xs text-ink-muted transition-colors hover:border-accent/60 hover:text-ink"
        title="セッション開始時の値に戻す"
      >
        <RotateCcw class="h-3.5 w-3.5" aria-hidden="true" />
        既定に戻す
      </button>
      <SaveIndicator state={saveState} {errorMessage} />
    </div>
  </header>

  <main class="scrollbar-minimal flex-1 overflow-y-auto px-6 py-6">
    {#if schema && settings}
      <div class="mx-auto max-w-3xl">
        <SchemaForm {schema} bind:value={settings} {query} onChange={scheduleSave} />
      </div>
    {:else if saveState === 'error'}
      <p class="text-red-500">{errorMessage}</p>
    {:else}
      <p class="text-ink-faint">読み込み中…</p>
    {/if}
  </main>

  <CommandPalette />
</div>

{#if confirmReset}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
    role="presentation"
    onclick={() => (confirmReset = false)}
  >
    <div
      class="w-full max-w-sm rounded-ml border border-border bg-surface-elevated p-5 shadow-ml-lg"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <h2 class="text-base font-semibold">全項目を元に戻しますか？</h2>
      <p class="mt-1 text-sm text-ink-muted">
        このセッションで変更した全ての項目が、開いた時点の値に戻ります。
      </p>
      <div class="mt-4 flex justify-end gap-2">
        <button
          type="button"
          class="rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm"
          onclick={() => (confirmReset = false)}
        >
          キャンセル
        </button>
        <button
          type="button"
          class="rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white"
          onclick={resetAll}
        >
          戻す
        </button>
      </div>
    </div>
  </div>
{/if}
