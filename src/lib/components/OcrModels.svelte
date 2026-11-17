<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Check, Download } from 'lucide-svelte';
  import {
    installOcrModels,
    ocrModelsStatus,
    onModelProgress,
    type ModelProgress,
    type OcrModelsStatus
  } from '$lib/ipc';
  import { errorText } from '$lib/errors';

  let status = $state<OcrModelsStatus | null>(null);
  let installing = $state(false);
  let progress = $state<ModelProgress | null>(null);
  let error = $state<string | null>(null);
  let unlisten: (() => void) | undefined;

  const ready = $derived(!!status?.ppocr && !!status?.ocrs);

  async function refresh() {
    try {
      status = await ocrModelsStatus();
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(async () => {
    await refresh();
    unlisten = await onModelProgress((p) => (progress = p));
  });
  onDestroy(() => unlisten?.());

  async function install() {
    installing = true;
    error = null;
    try {
      await installOcrModels();
    } catch (e) {
      error = errorText(e);
    } finally {
      installing = false;
      progress = null;
      await refresh();
    }
  }
</script>

<section class="rounded-ml border border-border bg-surface-elevated p-4 shadow-ml sm:p-5">
  <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
    <div class="min-w-0">
      <h3 class="flex flex-wrap items-center gap-2 text-base font-semibold">
        OCR モデル
        {#if status}
          {#if ready}
            <span class="inline-flex items-center gap-1 rounded-full bg-accent/15 px-2 py-0.5 text-xs font-normal text-accent">
              <Check class="h-3 w-3" /> 導入済み
            </span>
          {:else}
            <span class="rounded-full bg-red-500/10 px-2 py-0.5 text-xs font-normal text-red-500">未導入</span>
          {/if}
        {/if}
      </h3>
      <p class="text-sm text-ink-muted">
        画面上の文字認識用。PP-OCRv5（日中英、約 26 MB）／ocrs（英語、約 12 MB）
      </p>
      {#if status && !ready}
        <ul class="mt-1 text-xs text-ink-muted">
          <li>PP-OCRv5（日中英）: {status.ppocr ? '導入済み' : '未導入'}</li>
          <li>ocrs（英語）: {status.ocrs ? '導入済み' : '未導入'}</li>
        </ul>
      {/if}
    </div>
    {#if !ready}
      <button
        class="inline-flex shrink-0 items-center gap-2 self-start rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-40"
        onclick={install}
        disabled={installing}
      >
        <Download class="h-4 w-4" />
        {installing ? 'ダウンロード中…' : 'ダウンロード'}
      </button>
    {/if}
  </div>
  {#if progress}
    <div class="mt-3">
      <div class="flex items-center justify-between gap-2 text-xs text-ink-faint">
        <span class="min-w-0 truncate font-mono">{progress.filename}</span>
        <span class="shrink-0">
          {(progress.downloaded / 1_048_576).toFixed(1)} MB
          {#if progress.total}/ {(progress.total / 1_048_576).toFixed(1)} MB{/if}
        </span>
      </div>
      <div class="mt-1 h-1.5 overflow-hidden rounded-full bg-surface-muted">
        <div
          class="h-full rounded-full bg-accent transition-[width]"
          style="width: {((progress.fraction ?? 0) * 100).toFixed(1)}%"
        ></div>
      </div>
    </div>
  {/if}
  {#if error}
    <p class="mt-3 text-xs break-words text-red-500">{error}</p>
  {/if}
  {#if status?.dir}
    <p class="mt-3 text-xs break-all text-ink-faint">
      保存先: <code class="font-mono">{status.dir}</code>
    </p>
  {/if}
</section>
