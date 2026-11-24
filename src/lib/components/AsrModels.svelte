<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Check, Download, Trash2, X } from 'lucide-svelte';
  import {
    cancelAsrDownload,
    deleteAsrModel,
    installAsrModel,
    listAsrModels,
    onAsrProgress,
    selectAsrModel,
    type AsrModels,
    type LlmProgress
  } from '$lib/ipc';
  import { errorText } from '$lib/errors';

  let data = $state<AsrModels>({ selected: '', dir: '', models: [] });
  let progress = $state<Record<string, LlmProgress>>({});
  let busy = $state<Record<string, boolean>>({});
  let error = $state<string | null>(null);
  /** Model whose delete button was pressed once (asks to confirm). */
  let confirmDelete = $state<string | null>(null);
  let unlisten: (() => void) | undefined;

  const size = (n: number) =>
    n >= 1_073_741_824 ? `${(n / 1_073_741_824).toFixed(1)} GB` : `${Math.round(n / 1_048_576)} MB`;

  async function refresh() {
    try {
      data = await listAsrModels();
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(async () => {
    await refresh();
    unlisten = await onAsrProgress((p) => {
      progress = { ...progress, [p.id]: p };
    });
  });
  onDestroy(() => unlisten?.());

  async function download(id: string) {
    error = null;
    busy = { ...busy, [id]: true };
    try {
      await installAsrModel(id);
      // Put the first downloaded model to work.
      if (!data.models.find((x) => x.id === data.selected)?.installed) {
        await selectAsrModel(id);
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

  async function remove(id: string) {
    if (confirmDelete !== id) {
      confirmDelete = id;
      return;
    }
    confirmDelete = null;
    error = null;
    try {
      await deleteAsrModel(id);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }

  async function choose(id: string) {
    error = null;
    try {
      await selectAsrModel(id);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }
</script>

<section class="rounded-ml border border-border bg-surface-elevated p-4 shadow-ml sm:p-5">
  <h3 class="text-base font-semibold">音声認識モデル</h3>
  <p class="text-sm text-ink-muted">音声翻訳用。内蔵エンジンでローカル実行（外部サーバー不要）。</p>

  <ul class="mt-3 flex flex-col gap-2">
    {#each data.models as m (m.id)}
      {@const p = progress[m.id]}
      {@const selected = m.id === data.selected}
      <li
        class="rounded-ml-sm border p-3 {selected
          ? 'border-accent/60 bg-accent/5'
          : 'border-border bg-surface'}"
      >
        <div class="flex flex-col gap-2 md:flex-row md:items-center md:justify-between md:gap-3">
          <div class="min-w-0">
            <div class="flex flex-wrap items-center gap-2">
              <span class="font-medium">{m.label}</span>
              {#if selected}
                <span class="rounded-full bg-accent/15 px-2 py-0.5 text-xs text-accent">使用中</span>
              {/if}
            </div>
            <p class="text-xs text-ink-muted">
              {m.note} ・ {size(m.size)} ・
              <a class="underline" href={m.license_url} target="_blank" rel="noreferrer"
                >{m.license}</a
              >
            </p>
          </div>
          <div class="flex shrink-0 flex-wrap items-center gap-2">
            {#if m.installed}
              {#if !selected}
                <button
                  class="inline-flex items-center gap-1 rounded-ml-sm border border-border px-3 py-1.5 text-sm text-ink-muted hover:border-accent/50 hover:text-ink"
                  onclick={() => choose(m.id)}
                >
                  <Check class="h-4 w-4" /> 使用
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
                onclick={() => cancelAsrDownload(m.id)}
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
              {size(p.downloaded)} / {size(p.total)}
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

  {#if error}
    <p class="mt-3 text-xs break-words text-red-500">{error}</p>
  {/if}
  {#if data.dir}
    <p class="mt-3 text-xs break-all text-ink-faint">
      保存先: <code class="font-mono">{data.dir}</code>
    </p>
  {/if}
</section>
