<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { Play, Save, Trash2 } from 'lucide-svelte';
  import {
    deleteLayoutSet,
    listLayoutSets,
    onLayoutSets,
    putLayoutSet,
    restoreLayoutSet,
    saveLayoutSet,
    updateLayoutSet
  } from '$lib/ipc';
  import { setHelp, setSummary, type LayoutSet } from '$lib/layout-sets';
  import { errorText } from '$lib/errors';
  import { t } from '$lib/i18n';
  import { notify, notifyError, undoable } from '$lib/toast.svelte';
  import SettingRow from './SettingRow.svelte';

  // 配置セットの管理：保存・復元・上書・名称変更・自動復元の切替・削除。

  let sets = $state<LayoutSet[]>([]);
  let loaded = $state(false);
  let busy = $state(false);
  let unlisten: UnlistenFn | undefined;

  async function refresh() {
    try {
      sets = await listLayoutSets();
    } catch (e) {
      notifyError(t('layout-sets:load_failed', { detail: errorText(e) }));
    }
    loaded = true;
  }

  onMount(async () => {
    await refresh();
    unlisten = await onLayoutSets(refresh).catch(() => undefined);
  });
  onDestroy(() => unlisten?.());

  /** 操作を実行し、失敗時は通知。多重実行は不可。 */
  async function run(failure: string, action: () => Promise<void>) {
    if (busy) return;
    busy = true;
    try {
      await action();
    } catch (e) {
      notifyError(t('toast:label_detail', { label: t(failure), detail: errorText(e) }));
    } finally {
      busy = false;
      await refresh();
    }
  }

  const save = (target?: LayoutSet) =>
    run('layout-sets:save_failed', async () => {
      const saved = await saveLayoutSet(target?.id);
      notify(t('common:layout_saved', { name: saved.name, n: saved.windows.length }));
    });

  const restore = (set: LayoutSet) =>
    run('common:restore_failed', async () => {
      const count = await restoreLayoutSet(set.id);
      notify(t('common:layout_restored', { name: set.name, n: count }));
    });

  const remove = (set: LayoutSet) =>
    run('layout-sets:delete_failed', async () => {
      const removed = await deleteLayoutSet(set.id);
      if (!removed) return;
      const [index, old] = removed;
      undoable({
        text: t('common:name_deleted', { name: set.name }),
        failure: t('common:restore_failed'),
        undo: async () => {
          await putLayoutSet(old, index);
          await refresh();
        }
      });
    });

  const setAuto = (set: LayoutSet, on: boolean) =>
    run('layout-sets:change_failed', () => updateLayoutSet(set.id, { autoRestore: on }));

  function rename(set: LayoutSet, value: string) {
    const name = value.trim();
    if (!name || name === set.name) return;
    run('layout-sets:rename_failed', () => updateLayoutSet(set.id, { name }));
  }
</script>

<SettingRow
  id="layout:new"
  help={t('layout-sets:new_help')}
>
  <div class="srow">
    <span class="row-label">{t('layout-sets:current_layout')}</span>
    <span class="row-ctl">
      <button class="sbtn" disabled={busy} onclick={() => save()}>{t('layout-sets:save_new')}</button>
    </span>
  </div>
</SettingRow>
{#each sets as set (set.id)}
  <SettingRow id="layout:{set.id}" help={setHelp(set)}>
    <div class="srow set">
      <span class="row-label name">
        <input
          class="name-input"
          value={set.name}
          aria-label={t('layout-sets:name')}
          maxlength="40"
          onchange={(e) => rename(set, (e.currentTarget as HTMLInputElement).value)}
          onkeydown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()}
        />
        <span class="row-value summary">{setSummary(set)}</span>
      </span>
      <span class="row-ctl">
        {#if set.app}
          <label class="toggle-chips" title={t('layout-sets:auto_restore_on_launch', { name: set.app.name })}>
            <input
              type="checkbox"
              class="sr-only"
              aria-label={t('layout-sets:auto_restore_of', { name: set.name })}
              checked={set.auto_restore}
              disabled={busy}
              onchange={(e) => setAuto(set, (e.currentTarget as HTMLInputElement).checked)}
            />
            <span class="tc" class:on={set.auto_restore}>{t('settings:auto')}</span>
            <span class="tc" class:on={!set.auto_restore}>{t('layout-sets:manual')}</span>
          </label>
        {/if}
        <button
          class="sbtn icon neutral"
          title={t('layout-sets:restore')}
          aria-label={t('layout-sets:restore_name', { name: set.name })}
          disabled={busy}
          onclick={() => restore(set)}
        >
          <Play class="h-4 w-4" />
        </button>
        <button
          class="sbtn icon neutral"
          title={t('layout-sets:overwrite')}
          aria-label={t('layout-sets:overwrite_name', { name: set.name })}
          disabled={busy}
          onclick={() => save(set)}
        >
          <Save class="h-4 w-4" />
        </button>
        <button
          class="sbtn icon"
          title={t('common:delete')}
          aria-label={t('layout-sets:delete_name', { name: set.name })}
          disabled={busy}
          onclick={() => remove(set)}
        >
          <Trash2 class="h-4 w-4" />
        </button>
      </span>
    </div>
  </SettingRow>
{/each}
{#if loaded && sets.length === 0}
  <p class="srow"><span class="row-label row-value">{t('common:no_entries')}</span></p>
{/if}

<style>
  /* 操作部の横幅不足時（文字サイズ拡大・狭幅）：操作部を次行へ */
  .set {
    flex-wrap: wrap;
  }
  .set .name {
    flex: 1 1 16em;
  }
  .set .row-ctl {
    margin-left: auto;
  }
  .name {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: var(--ml-sp-1);
  }
  .name-input {
    min-width: 0;
    width: 100%;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: inherit;
    font: inherit;
    padding: 0;
  }
  .name-input:hover,
  .name-input:focus {
    border-bottom-color: currentColor;
    outline: none;
  }
</style>
