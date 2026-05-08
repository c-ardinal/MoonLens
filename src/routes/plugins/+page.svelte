<script lang="ts">
  import { onMount } from 'svelte';
  import {
    Search,
    Upload,
    RefreshCw,
    Download,
    Shield,
    Trash2,
    Folder,
    Puzzle
  } from 'lucide-svelte';
  import {
    checkPluginUpdates,
    listPlugins,
    setPluginEnabled,
    pickAndInstallPlugin,
    revealPluginFolder,
    uninstallPlugin,
    type PluginDescriptor
  } from '$lib/ipc';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Pill from '$lib/components/Pill.svelte';
  import Toggle from '$lib/components/Toggle.svelte';
  import Segmented from '$lib/components/Segmented.svelte';

  type Tab = 'installed' | 'marketplace';

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
  let installMsg = $state<string | null>(null);
  let installError = $state<string | null>(null);
  let checkingUpdates = $state(false);
  let updates = $state<PluginUpdate[]>([]);
  let updatesChecked = $state(false);
  let tab = $state<Tab>('installed');
  let selectedId = $state<string | null>(null);
  let query = $state('');
  let confirmUninstall = $state(false);
  let uninstalling = $state(false);

  onMount(refresh);

  async function refresh() {
    try {
      plugins = await listPlugins();
      if (plugins.length > 0 && (selectedId === null || !plugins.find((p) => p.id === selectedId))) {
        selectedId = plugins[0].id;
      }
    } finally {
      loading = false;
    }
  }

  async function toggle(p: PluginDescriptor, enabled: boolean) {
    await setPluginEnabled(p.id, enabled);
    await refresh();
  }

  async function install() {
    if (installing) return;
    installing = true;
    installMsg = null;
    installError = null;
    try {
      const result = await pickAndInstallPlugin();
      if (result) {
        installMsg = `インストール完了: ${result}`;
        await refresh();
      } else {
        installMsg = 'キャンセルされました。';
      }
    } catch (e) {
      installError = String(e);
    } finally {
      installing = false;
    }
  }

  async function reveal(id: string) {
    try {
      await revealPluginFolder(id);
    } catch (e) {
      installError = `フォルダを開けませんでした: ${e}`;
    }
  }

  async function uninstall(id: string) {
    if (uninstalling) return;
    uninstalling = true;
    installError = null;
    try {
      await uninstallPlugin(id);
      confirmUninstall = false;
      // Drop the selection so the detail pane doesn't render the
      // ghost of a plugin we just removed.
      if (selectedId === id) selectedId = null;
      await refresh();
    } catch (e) {
      installError = String(e);
    } finally {
      uninstalling = false;
    }
  }

  async function checkUpdates() {
    if (checkingUpdates) return;
    checkingUpdates = true;
    try {
      updates = await checkPluginUpdates();
      updatesChecked = true;
    } catch (e) {
      installError = `更新チェック失敗: ${e}`;
    } finally {
      checkingUpdates = false;
    }
  }

  const filtered = $derived(
    plugins.filter((p) => {
      if (!query.trim()) return true;
      const q = query.toLowerCase();
      return (
        p.name.toLowerCase().includes(q) ||
        p.id.toLowerCase().includes(q) ||
        p.author.toLowerCase().includes(q)
      );
    })
  );

  const current = $derived(plugins.find((p) => p.id === selectedId) ?? null);

  const permLabel: Record<string, string> = {
    network: 'ネットワーク通信',
    'history.read': '翻訳履歴の読み取り',
    'filesystem.write': 'ファイルへの書き込み',
    'translation.hook': '翻訳パイプラインへの介入',
    'audio.subscribe': '音声ストリームの受信'
  };
</script>

<PageHeader title="プラグイン" subtitle="ユーザ拡張の有効化・権限確認・設定編集">
  {#snippet actions()}
    <button
      class="btn ghost"
      type="button"
      onclick={checkUpdates}
      disabled={checkingUpdates}
    >
      <RefreshCw
        class="h-3.5 w-3.5 {checkingUpdates ? 'animate-spin' : ''}"
        aria-hidden="true"
      />
      更新を確認
    </button>
    <button
      class="btn primary"
      type="button"
      onclick={install}
      disabled={installing}
    >
      <Upload class="h-3.5 w-3.5" aria-hidden="true" />
      {installing ? 'インストール中…' : '追加'}
    </button>
  {/snippet}
</PageHeader>

<div class="ml-content plugins">
  {#if installMsg}
    <p class="banner accent">{installMsg}</p>
  {/if}
  {#if installError}
    <p class="banner error" role="alert">エラー: {installError}</p>
  {/if}

  {#if updatesChecked}
    {#if updates.length === 0}
      <p class="banner muted">全てのプラグインが最新です。</p>
    {:else}
      <section class="card update-banner">
        <h3>{updates.length} 件の更新が利用可能</h3>
        <ul>
          {#each updates as u (u.plugin_id)}
            <li>
              <span class="mono">{u.plugin_id}</span>
              <span class="muted">
                {u.installed_version} → <span class="accent-text">{u.latest_version}</span>
              </span>
              {#if u.notes}<p class="muted small">{u.notes}</p>{/if}
              {#if u.download_url}
                <a class="btn" href={u.download_url} target="_blank" rel="noreferrer">
                  <Download class="h-3.5 w-3.5" aria-hidden="true" /> ダウンロード
                </a>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/if}

  <div class="plugins-grid fadein">
    <!-- List pane -->
    <div class="list-pane">
      <Segmented
        value={tab}
        onChange={(v) => {
          tab = v;
        }}
        options={[
          { value: 'installed', label: `インストール済み (${plugins.length})` },
          { value: 'marketplace', label: 'マーケットプレイス' }
        ]}
        aria-label="プラグインタブ"
      />

      <div class="list-search">
        <div class="search-wrap">
          <Search class="search-ico h-3.5 w-3.5" aria-hidden="true" />
          <input
            class="input search-input"
            placeholder="プラグインを検索…"
            bind:value={query}
          />
        </div>
        <button class="btn" type="button" onclick={install}>
          <Upload class="h-3.5 w-3.5" aria-hidden="true" />
          ローカルから追加
        </button>
      </div>

      <div class="scroll list-scroll">
        {#if loading}
          <p class="muted">読み込み中…</p>
        {:else if tab === 'installed' && filtered.length === 0}
          <div class="empty">
            まだプラグインがインストールされていません。<br />
            <code class="mono small"
              >$APPDATA/plugins/&lt;id&gt;/plugin.toml</code
            > に配置するか「追加」から読み込んでください。
          </div>
        {:else if tab === 'marketplace'}
          <div class="empty">
            マーケットプレイスは準備中です。<br />
            ローカルパッケージのインストールには「追加」をご利用ください。
          </div>
        {:else}
          {#each filtered as p (p.id)}
            <button
              class="card-soft list-row"
              class:active={selectedId === p.id}
              type="button"
              onclick={() => (selectedId = p.id)}
            >
              <div class="row-icon">
                <Puzzle class="h-4 w-4" aria-hidden="true" />
              </div>
              <div class="row-body">
                <div class="row-head">
                  <span class="row-name">{p.name}</span>
                  {#if p.enabled}
                    <Pill accent dot>有効</Pill>
                  {:else}
                    <Pill dot>無効</Pill>
                  {/if}
                </div>
                <div class="row-author">
                  {p.author} · v{p.version} · <span class="mono small">{p.kind}</span>
                </div>
                <div class="row-desc">{p.id}</div>
              </div>
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Detail pane -->
    <div class="card detail-pane">
      {#if current}
        <div class="detail-head">
          <div class="detail-row">
            <div class="detail-icon">
              <Puzzle class="h-6 w-6" aria-hidden="true" />
            </div>
            <div style="flex: 1">
              <div class="detail-name">{current.name}</div>
              <div class="muted small">{current.author} · v{current.version}</div>
            </div>
            <div class="detail-toggle">
              <span class="muted small">{current.enabled ? '有効' : '無効'}</span>
              <Toggle
                checked={current.enabled}
                onChange={(next) => toggle(current!, next)}
                aria-label="プラグインを有効化"
              />
            </div>
          </div>
          <p class="detail-id muted">
            <span class="mono small">{current.id}</span>
          </p>
        </div>

        <div class="scroll detail-body">
          <div>
            <div class="kicker">提供する機能</div>
            {#if current.provides.length === 0}
              <p class="muted small">特になし</p>
            {:else}
              <div class="provides">
                {#each current.provides as p}
                  <Pill>{p}</Pill>
                {/each}
              </div>
            {/if}
          </div>

          <div>
            <div class="kicker">アクセス権限</div>
            {#if current.permissions_summary.length === 0}
              <p class="muted small">権限なし</p>
            {:else}
              <div class="perms">
                {#each current.permissions_summary as p}
                  <div class="perm-row">
                    <Shield class="h-3.5 w-3.5 accent-text" aria-hidden="true" />
                    <span class="mono small">{p}</span>
                    <span class="muted small">{permLabel[p] ?? p}</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>

          <div>
            <div class="kicker">パス</div>
            <code class="path mono small">{current.path}</code>
          </div>
        </div>

        <div class="detail-foot">
          <button class="btn" type="button" onclick={() => refresh()}>
            <RefreshCw class="h-3.5 w-3.5" aria-hidden="true" /> 再読込
          </button>
          <button class="btn" type="button" onclick={() => reveal(current!.id)}>
            <Folder class="h-3.5 w-3.5" aria-hidden="true" /> フォルダを開く
          </button>
          <div style="flex: 1"></div>
          <button
            class="btn danger"
            type="button"
            onclick={() => (confirmUninstall = true)}
          >
            <Trash2 class="h-3.5 w-3.5" aria-hidden="true" /> アンインストール
          </button>
        </div>
      {:else}
        <div class="detail-empty">
          <p>プラグインを選択すると詳細が表示されます。</p>
        </div>
      {/if}
    </div>
  </div>
</div>

{#if confirmUninstall && current}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={() => (confirmUninstall = false)}
  >
    <div
      class="card modal"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <h2>{current.name} をアンインストール</h2>
      <p class="muted">
        プラグインのファイル一式が削除されます。設定や履歴データは保持されます。
      </p>
      <div class="modal-actions">
        <button
          class="btn"
          type="button"
          onclick={() => (confirmUninstall = false)}
          disabled={uninstalling}
        >
          キャンセル
        </button>
        <button
          class="btn danger"
          type="button"
          onclick={() => uninstall(current!.id)}
          disabled={uninstalling}
        >
          {uninstalling ? '削除中…' : 'アンインストール'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .plugins {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .plugins-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
    gap: 16px;
    height: 100%;
    min-height: 0;
  }

  .banner {
    margin: 0 0 12px;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    font-size: 12px;
    color: var(--ink-1);
  }
  .banner.accent {
    background: var(--accent-soft);
    color: var(--accent-strong);
    border-color: transparent;
  }
  .banner.error {
    background: rgba(216, 122, 122, 0.1);
    color: #d87a7a;
    border-color: rgba(216, 122, 122, 0.3);
  }
  .banner.muted {
    color: var(--ink-2);
    background: var(--bg-2);
  }

  .update-banner {
    margin-bottom: 16px;
    background: var(--accent-soft);
    border-color: transparent;
  }
  .update-banner h3 {
    margin: 0 0 12px;
    font-size: 13px;
    color: var(--accent-strong);
  }
  .update-banner ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .update-banner li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }

  .list-pane {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    min-width: 0;
  }
  .list-search {
    display: flex;
    gap: 8px;
  }
  .search-wrap {
    position: relative;
    flex: 1;
  }
  :global(.search-ico) {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--ink-3);
  }
  .search-input {
    padding-left: 30px;
  }
  .list-scroll {
    flex: 1;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .list-row {
    padding: 12px 14px;
    display: flex;
    gap: 12px;
    align-items: center;
    cursor: pointer;
    text-align: left;
    transition: all var(--dur-fast);
  }
  .list-row.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .row-icon {
    width: 36px;
    height: 36px;
    border-radius: 9px;
    background: var(--bg-3);
    color: var(--ink-1);
    display: grid;
    place-items: center;
    flex: 0 0 36px;
  }
  .list-row.active .row-icon {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .row-body {
    flex: 1;
    min-width: 0;
  }
  .row-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 2px;
  }
  .row-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-0);
  }
  .row-author {
    font-size: 11px;
    color: var(--ink-2);
    margin-bottom: 3px;
  }
  .row-desc {
    font-size: 11.5px;
    color: var(--ink-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    padding: 24px;
    border: 1px dashed var(--line);
    border-radius: var(--r-md);
    color: var(--ink-2);
    font-size: 12px;
    text-align: center;
    line-height: 1.7;
  }

  .detail-pane {
    padding: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
  }
  .detail-head {
    padding: 20px;
    border-bottom: 1px solid var(--line);
  }
  .detail-row {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 8px;
  }
  .detail-icon {
    width: 56px;
    height: 56px;
    border-radius: 14px;
    background: var(--bg-3);
    color: var(--accent);
    display: grid;
    place-items: center;
    border: 1px solid var(--line);
    flex: 0 0 56px;
  }
  .detail-name {
    font-size: 18px;
    font-weight: 600;
    margin-bottom: 3px;
  }
  .detail-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .detail-id {
    margin: 8px 0 0;
    font-size: 12px;
  }
  .detail-body {
    flex: 1;
    overflow: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .kicker {
    font-size: 10.5px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-bottom: 8px;
    font-weight: 500;
  }
  .provides {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .perms {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .perm-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    font-size: 11.5px;
    color: var(--ink-1);
  }
  .perm-row span:last-child {
    margin-left: auto;
    color: var(--ink-2);
  }
  .accent-text {
    color: var(--accent);
  }
  .path {
    display: block;
    padding: 10px 12px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    color: var(--ink-1);
    word-break: break-all;
  }
  .small {
    font-size: 11px;
  }
  .detail-foot {
    padding: 14px;
    border-top: 1px solid var(--line);
    display: flex;
    gap: 8px;
  }
  .detail-empty {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--ink-2);
    font-size: 13px;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(0, 0, 0, 0.4);
  }
  .modal {
    width: 100%;
    max-width: 360px;
  }
  .modal h2 {
    font-size: 14px;
    font-weight: 600;
    margin: 0 0 4px;
  }
  .modal-actions {
    margin-top: 16px;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
