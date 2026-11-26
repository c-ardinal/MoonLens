<!--
  拡張機能の管理（設定画面「拡張」）。追加・更新確認・有効化。
  操作は即時反映。詳細（提供機能・権限）は説明窓へ表示。
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { Download } from 'lucide-svelte';
  import {
    checkPluginUpdates,
    listPlugins,
    pickAndInstallPlugin,
    setPluginEnabled,
    type PluginDescriptor
  } from '$lib/ipc';
  import { errorText } from '$lib/errors';
  import { notify } from '$lib/toast.svelte';
  import SettingRow from './SettingRow.svelte';

  type PluginUpdate = {
    plugin_id: string;
    installed_version: string;
    latest_version: string;
    download_url: string | null;
    notes: string | null;
  };

  let plugins = $state<PluginDescriptor[]>([]);
  let loading = $state(true);
  let installing = $state(false);
  let checking = $state(false);
  let error = $state<string | null>(null);
  /** 更新確認の結果（未確認：null）。 */
  let updates = $state<PluginUpdate[] | null>(null);

  const updateOf = (id: string) => updates?.find((u) => u.plugin_id === id);

  const ADD_HELP =
    '「ファイル追加」で .moonlens-plugin を読込。手動配置：$APPDATA/plugins/<id>/plugin.toml';

  function help(p: PluginDescriptor): string {
    const u = updateOf(p.id);
    return [
      `${p.id}（${p.kind}）v${p.version}・${p.author}・${p.license}`,
      p.provides.length ? `提供機能：${p.provides.join('、')}` : '',
      p.permissions_summary.length ? `権限：${p.permissions_summary.join('／')}` : '',
      u ? `更新あり：v${u.latest_version}${u.notes ? `（${u.notes}）` : ''}` : ''
    ]
      .filter(Boolean)
      .join('。');
  }

  async function refresh() {
    try {
      plugins = await listPlugins();
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

  async function toggle(p: PluginDescriptor, enabled: boolean) {
    error = null;
    try {
      await setPluginEnabled(p.id, enabled);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }

  async function install() {
    if (installing) return;
    installing = true;
    error = null;
    try {
      const result = await pickAndInstallPlugin();
      if (result) {
        notify(`追加完了：${result}`);
        await refresh();
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      installing = false;
    }
  }

  async function checkUpdates() {
    if (checking) return;
    checking = true;
    error = null;
    try {
      updates = await checkPluginUpdates();
      notify(updates.length === 0 ? '全拡張機能最新' : `更新あり：${updates.length} 件`);
    } catch (e) {
      error = `更新確認失敗：${errorText(e)}`;
    } finally {
      checking = false;
    }
  }
</script>

<SettingRow id="plugins:add" help={ADD_HELP}>
  <div class="srow">
    <span class="row-label">拡張機能の追加</span>
    <span class="row-ctl">
      {#if plugins.length > 0}
        <button class="sbtn" onclick={checkUpdates} disabled={checking}>
          {checking ? '確認中…' : '更新確認'}
        </button>
      {/if}
      <button class="sbtn on" onclick={install} disabled={installing}>
        {installing ? '追加中…' : 'ファイル追加'}
      </button>
    </span>
  </div>
</SettingRow>

{#if error}
  <div class="srow error px-pop" role="alert"><p class="row-label">{error}</p></div>
{/if}

{#if loading}
  <p class="srow"><span class="row-label">読込中…</span></p>
{:else if plugins.length === 0}
  <p class="srow"><span class="row-label empty">拡張機能なし</span></p>
{:else}
  {#each plugins as p (p.id)}
    {@const u = updateOf(p.id)}
    <SettingRow id="plugins:{p.id}" help={help(p)}>
      <div class="srow">
        <span class="row-label">{p.name}</span>
        <span class="row-ctl">
          {#if u}
            <span class="stag rec">更新 v{u.latest_version}</span>
            {#if u.download_url}
              <a
                class="sbtn icon"
                href={u.download_url}
                target="_blank"
                rel="noreferrer"
                title="更新版ダウンロード"
                aria-label="{p.name}：更新版ダウンロード"
              >
                <Download class="h-4 w-4" />
              </a>
            {/if}
          {:else}
            <span class="row-value">v{p.version}</span>
          {/if}
          <label class="toggle-chips">
            <input
              type="checkbox"
              class="sr-only"
              aria-label="{p.name}：有効化"
              checked={p.enabled}
              onchange={(e) => toggle(p, e.currentTarget.checked)}
            />
            <span class="tc" class:on={p.enabled}>ON</span>
            <span class="tc" class:on={!p.enabled}>OFF</span>
          </label>
        </span>
      </div>
    </SettingRow>
  {/each}
{/if}

<style>
  .empty {
    color: var(--ml-px-faint);
  }
</style>
