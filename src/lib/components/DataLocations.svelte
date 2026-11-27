<script lang="ts">
  import { t } from '$lib/i18n';
  import { onMount } from 'svelte';
  import { Trash2 } from 'lucide-svelte';
  import {
    dataLocations,
    deleteOrphanLlmFile,
    listOrphanLlmFiles,
    type DataLocations,
    type OrphanFile
  } from '$lib/ipc';
  import { errorText } from '$lib/errors';
  import { notify } from '$lib/toast.svelte';
  import SettingRow from './SettingRow.svelte';

  let info = $state<DataLocations | null>(null);
  /** Model files no current catalog entry owns (older versions). */
  let orphans = $state<OrphanFile[]>([]);
  let error = $state<string | null>(null);

  const size = (n: number) =>
    n >= 1_073_741_824 ? `${(n / 1_073_741_824).toFixed(1)} GB` : `${(n / 1_048_576).toFixed(1)} MB`;

  /** 保存先の行：パスは説明欄へ表示（行内は項目名と容量のみ）。 */
  const places = $derived(
    info
      ? [
          { id: 'models', label: t('モデル'), value: size(info.models_bytes), path: info.models_dir },
          { id: 'data', label: t('履歴・拡張機能'), value: size(info.data_bytes), path: info.data_dir },
          { id: 'config', label: t('設定ファイル'), value: '', path: info.config_dir },
          ...(info.log_dir
            ? [{ id: 'log', label: t('ログ'), value: t('不具合報告時に添付'), path: info.log_dir }]
            : [])
        ]
      : []
  );

  async function refresh() {
    try {
      [info, orphans] = await Promise.all([dataLocations(), listOrphanLlmFiles()]);
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(refresh);

  async function removeOrphan(name: string) {
    error = null;
    try {
      await deleteOrphanLlmFile(name);
      notify(t('{name}：削除完了', { name }));
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }
</script>

{#if info}
  <SettingRow id="data:version" help={t('バージョン：{version}', { version: info.version })}>
    <div class="srow">
      <span class="row-label">{t('バージョン')}</span>
      <span class="row-ctl"><span class="row-value">{info.version}</span></span>
    </div>
  </SettingRow>
  {#each places as p (p.id)}
    <SettingRow id="data:{p.id}" help={t('保存先：{path}', { path: p.path })}>
      <div class="srow">
        <span class="row-label">{p.label}</span>
        <span class="row-ctl"><span class="row-value">{p.value}</span></span>
      </div>
    </SettingRow>
  {/each}
  {#each orphans as o (o.name)}
    <SettingRow id="data:orphan:{o.name}" help={t('旧版のモデル（現行版で未使用）。削除で容量を解放。{name}', { name: o.name })}>
      <div class="srow">
        <span class="row-label" title={o.name}>{t('旧モデル：{name}', { name: o.name })}</span>
        <span class="row-ctl">
          <span class="row-value">{size(o.size)}</span>
          <button class="sbtn icon" title={t('削除')} aria-label={t('{name} を削除', { name: o.name })} onclick={() => removeOrphan(o.name)}>
            <Trash2 class="h-4 w-4" />
          </button>
        </span>
      </div>
    </SettingRow>
  {/each}
{:else if !error}
  <p class="srow"><span class="row-label">{t('読込中…')}</span></p>
{/if}
{#if error}
  <div class="srow error"><p class="row-label">{error}</p></div>
{/if}
