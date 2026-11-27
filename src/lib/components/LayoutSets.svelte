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
      notifyError(`配置セットの読込失敗：${errorText(e)}`);
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
      notifyError(`${failure}：${errorText(e)}`);
    } finally {
      busy = false;
      await refresh();
    }
  }

  const save = (target?: LayoutSet) =>
    run('保存失敗', async () => {
      const saved = await saveLayoutSet(target?.id);
      notify(`${saved.name}：ウィンドウ ${saved.windows.length} 個を保存`);
    });

  const restore = (set: LayoutSet) =>
    run('復元失敗', async () => {
      const count = await restoreLayoutSet(set.id);
      notify(`${set.name}：ウィンドウ ${count} 個を復元`);
    });

  const remove = (set: LayoutSet) =>
    run('削除失敗', async () => {
      const removed = await deleteLayoutSet(set.id);
      if (!removed) return;
      const [index, old] = removed;
      undoable({
        text: `${set.name}：削除完了`,
        failure: '復元失敗',
        undo: async () => {
          await putLayoutSet(old, index);
          await refresh();
        }
      });
    });

  const setAuto = (set: LayoutSet, on: boolean) =>
    run('変更失敗', () => updateLayoutSet(set.id, { autoRestore: on }));

  function rename(set: LayoutSet, value: string) {
    const name = value.trim();
    if (!name || name === set.name) return;
    run('名称変更失敗', () => updateLayoutSet(set.id, { name }));
  }
</script>

<SettingRow
  id="layout:new"
  help="表示中のウィンドウ翻訳一式を保存。位置・寸法・吸着先を記録。"
>
  <div class="srow">
    <span class="row-label">表示中の配置</span>
    <span class="row-ctl">
      <button class="sbtn" disabled={busy} onclick={() => save()}>新規保存</button>
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
          aria-label="配置セット名"
          maxlength="40"
          onchange={(e) => rename(set, (e.currentTarget as HTMLInputElement).value)}
          onkeydown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()}
        />
        <span class="row-value summary">{setSummary(set)}</span>
      </span>
      <span class="row-ctl">
        {#if set.app}
          <label class="toggle-chips" title="{set.app.name} の起動時に自動復元">
            <input
              type="checkbox"
              class="sr-only"
              aria-label="{set.name} の自動復元"
              checked={set.auto_restore}
              disabled={busy}
              onchange={(e) => setAuto(set, (e.currentTarget as HTMLInputElement).checked)}
            />
            <span class="tc" class:on={set.auto_restore}>自動</span>
            <span class="tc" class:on={!set.auto_restore}>手動</span>
          </label>
        {/if}
        <button
          class="sbtn icon neutral"
          title="復元"
          aria-label="{set.name} を復元"
          disabled={busy}
          onclick={() => restore(set)}
        >
          <Play class="h-4 w-4" />
        </button>
        <button
          class="sbtn icon neutral"
          title="表示中の配置で上書"
          aria-label="{set.name} を表示中の配置で上書"
          disabled={busy}
          onclick={() => save(set)}
        >
          <Save class="h-4 w-4" />
        </button>
        <button
          class="sbtn icon"
          title="削除"
          aria-label="{set.name} を削除"
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
  <p class="srow"><span class="row-label row-value">登録なし</span></p>
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
