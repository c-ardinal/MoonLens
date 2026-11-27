<!--
  拡張機能の管理（設定画面「拡張」）。追加・更新確認・有効化。
  操作は即時反映。詳細（提供機能・権限）は説明欄へ表示。
-->
<script lang="ts">
  import { t } from '$lib/i18n';
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

  /** 日本語の原文（表示時に `t()` で翻訳）。 */
  const ADD_HELP =
    '「ファイル追加」で .moonlens-plugin を読込。手動配置：$APPDATA/plugins/<id>/plugin.toml';

  function help(p: PluginDescriptor): string {
    const u = updateOf(p.id);
    return [
      t('{id}（{kind}）v{version}・{author}・{license}', {
        id: p.id,
        kind: p.kind,
        version: p.version,
        author: p.author,
        license: p.license
      }),
      p.provides.length ? t('提供機能：{list}', { list: p.provides.join(t('、')) }) : '',
      p.permissions_summary.length ? t('権限：{list}', { list: p.permissions_summary.join(t('／')) }) : '',
      u
        ? u.notes
          ? t('更新あり：v{version}（{notes}）', { version: u.latest_version, notes: u.notes })
          : t('更新あり：v{version}', { version: u.latest_version })
        : ''
    ]
      .filter(Boolean)
      .join(t('。'));
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
        notify(t('追加完了：{name}', { name: result }));
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
      notify(updates.length === 0 ? t('全拡張機能最新') : t('更新あり：{n} 件', { n: updates.length }));
    } catch (e) {
      error = t('更新確認失敗：{error}', { error: errorText(e) });
    } finally {
      checking = false;
    }
  }
</script>

<SettingRow id="plugins:add" help={t(ADD_HELP)}>
  <div class="srow">
    <span class="row-label">{t('拡張機能の追加')}</span>
    <span class="row-ctl">
      {#if plugins.length > 0}
        <button class="sbtn" onclick={checkUpdates} disabled={checking}>
          {checking ? t('確認中…') : t('更新確認')}
        </button>
      {/if}
      <button class="sbtn on" onclick={install} disabled={installing}>
        {installing ? t('追加中…') : t('ファイル追加')}
      </button>
    </span>
  </div>
</SettingRow>

{#if error}
  <div class="srow error px-pop" role="alert"><p class="row-label">{error}</p></div>
{/if}

{#if loading}
  <p class="srow"><span class="row-label">{t('読込中…')}</span></p>
{:else if plugins.length === 0}
  <p class="srow"><span class="row-label empty">{t('拡張機能なし')}</span></p>
{:else}
  {#each plugins as p (p.id)}
    {@const u = updateOf(p.id)}
    <SettingRow id="plugins:{p.id}" help={help(p)}>
      <div class="srow">
        <span class="row-label">{p.name}</span>
        <span class="row-ctl">
          {#if u}
            <span class="stag rec">{t('更新 v{version}', { version: u.latest_version })}</span>
            {#if u.download_url}
              <a
                class="sbtn icon"
                href={u.download_url}
                target="_blank"
                rel="noreferrer"
                title={t('更新版ダウンロード')}
                aria-label={t('{name}：更新版ダウンロード', { name: p.name })}
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
              aria-label={t('{name}：有効化', { name: p.name })}
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
