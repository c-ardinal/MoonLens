<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Check, Download, Trash2, X } from 'lucide-svelte';
  import {
    cancelLlmDownload,
    deleteLlmModel,
    deleteOrphanLlmFile,
    installLlmModel,
    listLlmModels,
    listOrphanLlmFiles,
    onLlmProgress,
    selectLlmModel,
    type LlmModels,
    type LlmProgress,
    type LlmRole,
    type OrphanFile
  } from '$lib/ipc';
  import { errorText } from '$lib/errors';

  let data = $state<LlmModels>({ selected: '', assistant: '', dir: '', models: [] });
  let progress = $state<Record<string, LlmProgress>>({});
  let busy = $state<Record<string, boolean>>({});
  let error = $state<string | null>(null);
  /** Model whose delete button was pressed once (asks to confirm). */
  let confirmDelete = $state<string | null>(null);
  /** Model files no current catalog entry owns (older versions). */
  let orphans = $state<OrphanFile[]>([]);
  let unlisten: (() => void) | undefined;

  const gb = (n: number) => (n / 1_073_741_824).toFixed(1);

  async function refresh() {
    try {
      [data, orphans] = await Promise.all([listLlmModels(), listOrphanLlmFiles()]);
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(async () => {
    await refresh();
    unlisten = await onLlmProgress((p) => {
      progress = { ...progress, [p.id]: p };
    });
  });
  onDestroy(() => unlisten?.());

  async function download(id: string) {
    error = null;
    busy = { ...busy, [id]: true };
    try {
      await installLlmModel(id);
      // Put a freshly downloaded model to work when its role has no
      // usable model yet.
      const m = data.models.find((x) => x.id === id);
      const installed = (sel: string) => data.models.find((x) => x.id === sel)?.installed;
      if (m?.style === 'pfml3') {
        if (!installed(data.selected)) await selectLlmModel(id, 'translate');
      } else if (!installed(data.assistant)) {
        await selectLlmModel(id, 'assistant');
      }
    } catch (e) {
      if (!String(e).includes('cancelled')) error = errorText(e);
    } finally {
      busy = { ...busy, [id]: false };
      const { [id]: _done, ...rest } = progress;
      progress = rest;
      await refresh();
    }
  }

  async function cancel(id: string) {
    await cancelLlmDownload(id);
  }

  async function remove(id: string) {
    // Gigabytes to download again: ask once more before deleting.
    if (confirmDelete !== id) {
      confirmDelete = id;
      return;
    }
    confirmDelete = null;
    error = null;
    try {
      await deleteLlmModel(id);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }

  async function removeOrphan(name: string) {
    error = null;
    try {
      await deleteOrphanLlmFile(name);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }

  async function choose(id: string, role: LlmRole) {
    error = null;
    try {
      await selectLlmModel(id, role);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }
</script>

<section class="rounded-ml border border-border bg-surface-elevated p-4 shadow-ml sm:p-5">
  <h3 class="text-base font-semibold">翻訳・説明用モデル</h3>
  <p class="text-sm text-ink-muted">
    内蔵エンジンでローカル実行（Ollama 等は不要）。
    翻訳用：PLaMo 3 Translate／説明・要約用：Gemma 4
  </p>

  <ul class="mt-3 flex flex-col gap-2">
    {#each data.models as m (m.id)}
      {@const p = progress[m.id]}
      {@const forTranslate = m.id === data.selected}
      {@const forAssist = m.id === data.assistant}
      {@const selected = forTranslate || forAssist}
      <li
        class="rounded-ml-sm border p-3 {selected
          ? 'border-accent/60 bg-accent/5'
          : 'border-border bg-surface'}"
      >
        <div class="flex flex-col gap-2 md:flex-row md:items-center md:justify-between md:gap-3">
          <div class="min-w-0">
            <div class="flex flex-wrap items-center gap-2">
              <span class="font-medium">{m.label}</span>
              {#if forTranslate}
                <span class="rounded-full bg-accent/15 px-2 py-0.5 text-xs text-accent">翻訳</span>
              {/if}
              {#if forAssist}
                <span class="rounded-full bg-accent/15 px-2 py-0.5 text-xs text-accent"
                  >説明・要約</span
                >
              {/if}
            </div>
            <p class="text-xs text-ink-muted">
              {m.note} ・ {gb(m.size)} GB ・
              <a class="underline" href={m.license_url} target="_blank" rel="noreferrer"
                >{m.license}</a
              >
            </p>
          </div>
          <div class="flex shrink-0 flex-wrap items-center gap-2">
            {#if m.installed}
              {#if !forTranslate}
                <button
                  class="inline-flex items-center gap-1 rounded-ml-sm border border-border px-3 py-1.5 text-sm text-ink-muted hover:border-accent/50 hover:text-ink"
                  onclick={() => choose(m.id, 'translate')}
                >
                  <Check class="h-4 w-4" /> 翻訳用に設定
                </button>
              {/if}
              {#if m.style === 'chat_template' && !forAssist}
                <button
                  class="inline-flex items-center gap-1 rounded-ml-sm border border-border px-3 py-1.5 text-sm text-ink-muted hover:border-accent/50 hover:text-ink"
                  onclick={() => choose(m.id, 'assistant')}
                >
                  <Check class="h-4 w-4" /> 説明用に設定
                </button>
              {/if}
              {#if confirmDelete === m.id}
                <button
                  class="inline-flex items-center gap-1 rounded-ml-sm border border-red-500/60 bg-red-500/10 px-3 py-1.5 text-sm text-red-500"
                  onclick={() => remove(m.id)}
                  onblur={() => (confirmDelete = null)}
                >
                  <Trash2 class="h-4 w-4" />
                  {selected ? '使用中。削除確定' : '削除確定'}
                </button>
              {:else}
                <button
                  class="rounded-ml-sm border border-border p-1.5 text-ink-faint hover:border-red-500/60 hover:text-red-500"
                  title="削除"
                  aria-label="{m.label} を削除"
                  onclick={() => remove(m.id)}
                >
                  <Trash2 class="h-4 w-4" />
                </button>
              {/if}
            {:else if busy[m.id] || m.downloading}
              <button
                class="inline-flex items-center gap-1 rounded-ml-sm border border-border px-3 py-1.5 text-sm text-ink-muted hover:text-ink"
                onclick={() => cancel(m.id)}
              >
                <X class="h-4 w-4" /> 中止
              </button>
            {:else}
              <button
                class="inline-flex items-center gap-1 rounded-ml-sm border border-border px-3 py-1.5 text-sm text-ink-muted hover:border-accent/50 hover:text-ink"
                onclick={() => download(m.id)}
              >
                <Download class="h-4 w-4" />
                {m.partial_bytes > 0 ? '再開' : 'ダウンロード'}
              </button>
            {/if}
          </div>
        </div>
        {#if p}
          <div class="mt-2">
            <div class="flex justify-end text-xs text-ink-faint">
              {gb(p.downloaded)} / {gb(p.total)} GB
            </div>
            <div class="mt-1 h-1.5 overflow-hidden rounded-full bg-surface-muted">
              <div
                class="h-full rounded-full bg-accent transition-[width]"
                style="width: {(p.fraction * 100).toFixed(1)}%"
              ></div>
            </div>
          </div>
        {/if}
      </li>
    {/each}
  </ul>

  {#if orphans.length > 0}
    <div class="mt-3 rounded-ml-sm border border-dashed border-border p-3">
      <p class="text-sm font-medium">旧モデル・不要ファイル</p>
      <p class="text-xs text-ink-muted">現行版で未使用。削除で容量を解放。</p>
      <ul class="mt-2 flex flex-col gap-1">
        {#each orphans as o (o.name)}
          <li class="flex items-center justify-between gap-2 text-xs">
            <code class="min-w-0 truncate font-mono text-ink-muted" title={o.name}>{o.name}</code>
            <span class="flex shrink-0 items-center gap-2">
              <span class="text-ink-faint">{gb(o.size)} GB</span>
              <button
                class="rounded-ml-sm border border-border p-1 text-ink-faint hover:border-red-500/60 hover:text-red-500"
                title="削除"
                aria-label="{o.name} を削除"
                onclick={() => removeOrphan(o.name)}
              >
                <Trash2 class="h-3.5 w-3.5" />
              </button>
            </span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if error}
    <p class="mt-3 text-xs break-words text-red-500">{error}</p>
  {/if}
  {#if data.dir}
    <p class="mt-3 text-xs break-all text-ink-faint">
      保存先: <code class="font-mono">{data.dir}</code>
    </p>
  {/if}
</section>
