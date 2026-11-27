<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Trash2 } from 'lucide-svelte';
  import {
    deleteOcrScript,
    installOcrScript,
    ocrScriptFile,
    ocrScriptsStatus,
    onModelProgress,
    type OcrScript
  } from '$lib/ipc';
  import { formatBytes, OCR_SCRIPTS } from '$lib/model-setup';
  import { errorText } from '$lib/errors';
  import { notify } from '$lib/toast.svelte';
  import SettingRow from './SettingRow.svelte';

  /** 標準モデル（PP-OCR）が未取得の間は追加不可。 */
  let { ready }: { ready: boolean } = $props();

  let scripts = $state<OcrScript[]>([]);
  /** 取得中の文字体系と進捗（0〜1）。 */
  let progress = $state<Record<string, number>>({});
  let confirmDelete = $state<string | null>(null);
  let error = $state<string | null>(null);
  let unlisten: (() => void) | undefined;

  const label = (id: string) => OCR_SCRIPTS[id]?.label ?? id;
  const help = (s: OcrScript) =>
    `${OCR_SCRIPTS[s.id]?.note ?? ''}。標準モデルで読取不能な行のみ再認識（通常時の読取速度は不変）。${s.installed ? '' : '未取得。'}`;

  async function refresh() {
    try {
      scripts = await ocrScriptsStatus();
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(async () => {
    await refresh();
    unlisten = await onModelProgress((p) => {
      const id = scripts.find((s) => ocrScriptFile(s.id) === p.filename)?.id;
      if (id && id in progress) progress = { ...progress, [id]: p.fraction ?? 0 };
    });
  });
  onDestroy(() => unlisten?.());

  async function install(id: string) {
    error = null;
    progress = { ...progress, [id]: 0 };
    try {
      await installOcrScript(id);
      notify(`${label(id)}：取得完了`);
    } catch (e) {
      error = errorText(e);
    } finally {
      const { [id]: _done, ...rest } = progress;
      progress = rest;
      await refresh();
    }
  }

  async function remove(id: string) {
    confirmDelete = null;
    error = null;
    try {
      await deleteOcrScript(id);
      notify(`${label(id)}：削除完了`);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }
</script>

{#if scripts.length}
  <div class="srow head"><p class="row-label">追加の文字体系</p></div>
  <div role="group" aria-label="追加の文字体系">
    {#each scripts as s (s.id)}
      {@const p = progress[s.id]}
      <SettingRow id="ocr-script:{s.id}" help={help(s)}>
        <div class="srow">
          <span class="row-label">{label(s.id)}</span>
          <span class="row-ctl">
            <span class="row-value">{p !== undefined ? `${Math.round(p * 100)}%` : formatBytes(s.size)}</span>
            {#if p !== undefined}
              <span class="stag off">取得中</span>
            {:else if !s.installed}
              <button class="sbtn" disabled={!ready} onclick={() => install(s.id)} aria-label="{label(s.id)} を取得">
                取得
              </button>
            {:else if confirmDelete === s.id}
              <button class="sbtn danger" onclick={() => remove(s.id)} onblur={() => (confirmDelete = null)}>
                削除確定
              </button>
            {:else}
              <button
                class="sbtn icon"
                title="削除"
                aria-label="{label(s.id)} を削除"
                onclick={() => (confirmDelete = s.id)}
              >
                <Trash2 class="h-4 w-4" />
              </button>
            {/if}
          </span>
        </div>
        {#if p !== undefined}
          <div class="sprogress" aria-hidden="true"><span style="width: {(p * 100).toFixed(1)}%"></span></div>
        {/if}
      </SettingRow>
    {/each}
  </div>
{/if}
{#if error}
  <div class="srow error"><p class="row-label">{error}</p></div>
{/if}

<style>
  .head .row-label {
    color: var(--ml-px-dim);
  }
</style>
