<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Trash2 } from 'lucide-svelte';
  import {
    cancelLlmDownload,
    deleteLlmModel,
    selectLlmModel,
    type LlmModel,
    type LlmRole
  } from '$lib/ipc';
  import { llmModels } from '$lib/llm-models.svelte';
  import { errorText } from '$lib/errors';
  import SettingRow from './SettingRow.svelte';

  interface Props {
    /** 表示する用途：翻訳（全モデル）／説明・要約（汎用モデルのみ）。 */
    role?: LlmRole;
  }
  let { role = 'translate' }: Props = $props();

  const store = llmModels;
  const data = $derived(store.data);
  const progress = $derived(store.progress);
  const busy = $derived(store.busy);
  let error = $state<string | null>(null);
  /** Model whose delete button was pressed once (asks to confirm). */
  let confirmDelete = $state<string | null>(null);

  const gb = (n: number) => `${(n / 1_073_741_824).toFixed(1)} GB`;
  /** 説明・要約は汎用（チャット形式）モデルのみ対応。 */
  const shown = $derived(
    role === 'assistant' ? data.models.filter((m) => m.style === 'chat_template') : data.models
  );
  const current = $derived(role === 'assistant' ? data.assistant : data.selected);
  const inUse = (m: LlmModel) => m.id === data.selected || m.id === data.assistant;
  const help = (m: LlmModel) => `${m.note}。`;

  async function refresh() {
    try {
      await store.refresh();
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(async () => {
    await store.attach();
    await refresh();
  });
  onDestroy(() => store.detach());

  async function download(id: string) {
    error = null;
    try {
      await store.download(id, role);
    } catch (e) {
      if (!String(e).includes('cancelled')) error = errorText(e);
    }
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

  async function choose(id: string) {
    error = null;
    try {
      await selectLlmModel(id, role);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }
</script>

{#each shown as m (m.id)}
  {@const p = progress[m.id]}
  <SettingRow id="llm:{role}:{m.id}" help={help(m)} link={{ href: m.license_url, label: `ライセンス：${m.license}` }}>
    <div class="srow">
      <span class="row-label">{m.label}</span>
      <span class="row-ctl">
        {#if p}
          <span class="row-value">{Math.round(p.fraction * 100)}%</span>
        {:else}
          <span class="row-value">{gb(m.size)}</span>
        {/if}
        {#if m.installed}
          {#if m.id === current}
            <span class="stag">使用中</span>
          {:else}
            <button class="sbtn" onclick={() => choose(m.id)}>使用</button>
          {/if}
          {#if confirmDelete === m.id}
            <button class="sbtn danger" onclick={() => remove(m.id)} onblur={() => (confirmDelete = null)}>
              {inUse(m) ? '使用中。削除確定' : '削除確定'}
            </button>
          {:else}
            <button class="sbtn icon" title="削除" aria-label="{m.label} を削除" onclick={() => remove(m.id)}>
              <Trash2 class="h-4 w-4" />
            </button>
          {/if}
        {:else if busy[m.id] || m.downloading || p}
          <button class="sbtn" onclick={() => cancelLlmDownload(m.id)}>中止</button>
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
