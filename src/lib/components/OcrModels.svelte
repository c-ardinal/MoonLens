<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    installOcrModels,
    ocrModelsStatus,
    onModelProgress,
    type ModelProgress,
    type OcrModelsStatus
  } from '$lib/ipc';
  import { errorText } from '$lib/errors';
  import SettingRow from './SettingRow.svelte';

  let status = $state<OcrModelsStatus | null>(null);
  let installing = $state(false);
  let progress = $state<ModelProgress | null>(null);
  let error = $state<string | null>(null);
  let unlisten: (() => void) | undefined;

  /** 両エンジンは一括取得（片方のみの取得は不可）。 */
  const ENGINES = [
    { key: 'ppocr', label: 'PP-OCRv5', size: '26 MB', help: '標準エンジン。日本語・中国語・英語に対応。' },
    { key: 'ocrs', label: 'ocrs', size: '12 MB', help: '予備エンジン。英語のみ対応。' }
  ] as const;

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

{#if status}
  {#each ENGINES as e (e.key)}
    {@const ready = status[e.key]}
    <SettingRow id="ocr:{e.key}" help={e.help}>
      <div class="srow">
        <span class="row-label">{e.label}</span>
        <span class="row-ctl">
          <span class="row-value">
            {installing && !ready && progress ? `${Math.round((progress.fraction ?? 0) * 100)}%` : e.size}
          </span>
          {#if ready}
            <span class="stag">導入済み</span>
          {:else}
            <button class="sbtn" onclick={install} disabled={installing}>{installing ? '取得中…' : '取得'}</button>
          {/if}
        </span>
      </div>
      {#if installing && !ready && progress}
        <div class="sprogress" aria-hidden="true">
          <span style="width: {((progress.fraction ?? 0) * 100).toFixed(1)}%"></span>
        </div>
      {/if}
    </SettingRow>
  {/each}
{/if}
{#if error}
  <div class="srow error"><p class="row-label">{error}</p></div>
{/if}
