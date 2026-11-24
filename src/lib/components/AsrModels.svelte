<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Trash2 } from 'lucide-svelte';
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
  import SettingRow from './SettingRow.svelte';

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

{#each data.models as m (m.id)}
  {@const p = progress[m.id]}
  {@const selected = m.id === data.selected}
  <SettingRow id="asr:{m.id}" help="{m.note}。" link={{ href: m.license_url, label: `ライセンス：${m.license}` }}>
    <div class="srow">
      <span class="row-label">{m.label}</span>
      <span class="row-ctl">
        <span class="row-value">{p ? `${Math.round(p.fraction * 100)}%` : size(m.size)}</span>
        {#if m.installed}
          {#if selected}
            <span class="stag">使用中</span>
          {:else}
            <button class="sbtn" onclick={() => choose(m.id)}>使用</button>
          {/if}
          {#if confirmDelete === m.id}
            <button class="sbtn danger" onclick={() => remove(m.id)} onblur={() => (confirmDelete = null)}>
              {selected ? '使用中。削除確定' : '削除確定'}
            </button>
          {:else}
            <button class="sbtn icon" title="削除" aria-label="{m.label} を削除" onclick={() => remove(m.id)}>
              <Trash2 class="h-4 w-4" />
            </button>
          {/if}
        {:else if busy[m.id] || m.downloading || p}
          <button class="sbtn" onclick={() => cancelAsrDownload(m.id)}>中止</button>
        {:else}
          <button class="sbtn" onclick={() => download(m.id)}>{m.partial_bytes > 0 ? '再開' : '取得'}</button>
        {/if}
      </span>
    </div>
    {#if p}
      <div class="sprogress" aria-hidden="true"><span style="width: {(p.fraction * 100).toFixed(1)}%"></span></div>
    {/if}
  </SettingRow>
{/each}
{#if error}
  <div class="srow error"><p class="row-label">{error}</p></div>
{/if}
