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

  /** 文言のキー ID（表示時に `t()` で取得）。 */
  const ADD_HELP =
    'plugins:use_add_file_to_load_a_moonlens';

  function help(p: PluginDescriptor): string {
    const u = updateOf(p.id);
    return [
      t('plugins:id_kind_v_version_author_license', {
        id: p.id,
        kind: p.kind,
        version: p.version,
        author: p.author,
        license: p.license
      }),
      p.provides.length ? t('plugins:provides_list', { list: p.provides.join(t('common:list_separator')) }) : '',
      p.permissions_summary.length ? t('plugins:permissions_list', { list: p.permissions_summary.join(t('common:choice_separator')) }) : '',
      u
        ? u.notes
          ? t('plugins:update_available_v_version_notes', { version: u.latest_version, notes: u.notes })
          : t('plugins:update_available_v_version', { version: u.latest_version })
        : ''
    ]
      .filter(Boolean)
      .join(t('plugins:text'));
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
        notify(t('plugins:added_name', { name: result }));
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
      notify(updates.length === 0 ? t('plugins:all_extensions_are_up_to_date') : t('plugins:updates_available_n', { n: updates.length }));
    } catch (e) {
      error = t('plugins:update_check_failed_error', { error: errorText(e) });
    } finally {
      checking = false;
    }
  }
</script>

<SettingRow id="plugins:add" help={t(ADD_HELP)}>
  <div class="srow">
    <span class="row-label">{t('plugins:add_extension')}</span>
    <span class="row-ctl">
      {#if plugins.length > 0}
        <button class="sbtn" onclick={checkUpdates} disabled={checking}>
          {checking ? t('common:checking') : t('plugins:check_for_updates')}
        </button>
      {/if}
      <button class="sbtn on" onclick={install} disabled={installing}>
        {installing ? t('plugins:adding') : t('plugins:add_file')}
      </button>
    </span>
  </div>
</SettingRow>

{#if error}
  <div class="srow error px-pop" role="alert"><p class="row-label">{error}</p></div>
{/if}

{#if loading}
  <p class="srow"><span class="row-label">{t('common:loading')}</span></p>
{:else if plugins.length === 0}
  <p class="srow"><span class="row-label empty">{t('plugins:no_extensions')}</span></p>
{:else}
  {#each plugins as p (p.id)}
    {@const u = updateOf(p.id)}
    <SettingRow id="plugins:{p.id}" help={help(p)}>
      <div class="srow">
        <span class="row-label">{p.name}</span>
        <span class="row-ctl">
          {#if u}
            <span class="stag rec">{t('plugins:update_v_version', { version: u.latest_version })}</span>
            {#if u.download_url}
              <a
                class="sbtn icon"
                href={u.download_url}
                target="_blank"
                rel="noreferrer"
                title={t('plugins:download_update')}
                aria-label={t('plugins:name_download_update', { name: p.name })}
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
              aria-label={t('plugins:name_enable', { name: p.name })}
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
