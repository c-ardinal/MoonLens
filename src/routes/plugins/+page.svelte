<script lang="ts">
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import { onMount } from 'svelte';
  import { Puzzle, Shield, Check } from 'lucide-svelte';
  import {
    checkPluginUpdates,
    listPlugins,
    setPluginEnabled,
    pickAndInstallPlugin,
    type PluginDescriptor
  } from '$lib/ipc';
  import { RefreshCw, Download } from 'lucide-svelte';

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

  async function checkUpdates() {
    if (checkingUpdates) return;
    checkingUpdates = true;
    try {
      updates = await checkPluginUpdates();
      updatesChecked = true;
    } catch (e) {
      installError = `更新確認に失敗：${e}`;
    } finally {
      checkingUpdates = false;
    }
  }

  async function refresh() {
    try {
      plugins = await listPlugins();
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

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
        installMsg = `インストール完了：${result}`;
        await refresh();
      } else {
        installMsg = 'キャンセル済み。';
      }
    } catch (e) {
      installError = String(e);
    } finally {
      installing = false;
    }
  }
</script>

<!-- 画面高へ固定：一覧のみスクロール（画面全体のはみ出し防止） -->
<main class="plugins mx-auto flex max-w-4xl flex-col gap-4 px-4 py-6 sm:gap-6 sm:px-6">
  <header class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <div class="page-title"><PixelTitle text="拡張機能" tag="h1" height={40} fit /></div>
      <p class="text-sm text-ink-muted">拡張機能の有効化・権限確認。</p>
    </div>
    <div class="flex items-center gap-2">
      {#if plugins.length > 0}
        <button
          class="inline-flex items-center gap-1.5 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted hover:border-accent/60 hover:text-ink disabled:opacity-40"
          onclick={checkUpdates}
          disabled={checkingUpdates}
        >
          <RefreshCw class="h-4 w-4 {checkingUpdates ? 'animate-spin' : ''}" />
          更新確認
        </button>
      {/if}
      <button
        class="rounded-ml-sm border border-accent bg-accent px-3 py-1.5 text-sm font-medium text-white disabled:opacity-40"
        onclick={install}
        disabled={installing}
      >
        {installing ? 'インストール中…' : 'ファイル追加'}
      </button>
    </div>
  </header>

  {#if installMsg}
    <p class="rounded-ml border border-accent/40 bg-accent-soft p-2 text-sm text-accent">
      {installMsg}
    </p>
  {/if}

  {#if updatesChecked}
    {#if updates.length === 0}
      <p
        class="rounded-ml border border-border bg-surface-elevated p-2 text-xs text-ink-muted"
      >
        全拡張機能最新。
      </p>
    {:else}
      <section class="rounded-ml border border-accent/40 bg-accent-soft p-3">
        <h3 class="text-sm font-semibold text-accent">
          更新あり：{updates.length} 件
        </h3>
        <ul class="mt-2 space-y-2">
          {#each updates as u (u.plugin_id)}
            <li class="flex flex-wrap items-center justify-between gap-3 text-sm">
              <div class="min-w-0">
                <span class="font-mono">{u.plugin_id}</span>
                <span class="ml-2 text-ink-muted">
                  {u.installed_version} → <span class="text-accent">{u.latest_version}</span>
                </span>
                {#if u.notes}
                  <p class="mt-0.5 text-xs text-ink-muted">{u.notes}</p>
                {/if}
              </div>
              {#if u.download_url}
                <a
                  href={u.download_url}
                  target="_blank"
                  rel="noreferrer"
                  class="inline-flex items-center gap-1 rounded-ml-sm border border-border bg-surface-elevated px-2 py-1 text-xs text-ink-muted hover:border-accent/60 hover:text-ink"
                >
                  <Download class="h-3.5 w-3.5" />
                  ダウンロード
                </a>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/if}
  {#if installError}
    <p class="rounded-ml border border-red-500/50 bg-red-500/10 p-2 text-sm break-words text-red-500">
      エラー: {installError}
    </p>
  {/if}

  {#if loading}
    <p class="text-ink-faint">読込中…</p>
  {:else if plugins.length === 0}
    <div
      class="rounded-ml border border-dashed border-border bg-surface-elevated p-8 text-center text-ink-muted"
    >
      拡張機能なし。
      <br />
      「ファイル追加」で .moonlens-plugin を読込、または
      <code class="font-mono text-xs break-all">$APPDATA/plugins/&lt;id&gt;/plugin.toml</code>
      に配置。
    </div>
  {:else}
    <!-- 下・右の余白：窓の段影（4px）を見切れさせない -->
    <ul class="scrollbar-minimal flex min-h-0 flex-col gap-3 overflow-y-auto pr-1 pb-1">
      {#each plugins as p}
        <li
          class="flex items-start justify-between gap-4 rounded-ml border border-border bg-surface-elevated p-4 shadow-ml"
        >
          <div class="flex min-w-0 items-start gap-3">
            <Puzzle class="mt-0.5 h-5 w-5 shrink-0 text-ink-muted" />
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-2">
                <h2 class="font-medium">{p.name}</h2>
                <code class="font-mono text-xs text-ink-faint">{p.id}</code>
                <span class="rounded-full bg-surface-muted px-2 py-0.5 text-[10px] uppercase tracking-wide text-ink-faint">
                  {p.kind}
                </span>
              </div>
              <p class="text-xs text-ink-muted">
                v{p.version} · {p.author} · {p.license}
              </p>
              {#if p.provides.length}
                <p class="mt-1 text-xs text-ink-muted">
                  提供機能：{p.provides.join(', ')}
                </p>
              {/if}
              {#if p.permissions_summary.length}
                <p class="mt-1 flex items-center gap-1 text-xs text-ink-muted">
                  <Shield class="h-3 w-3" />
                  <span class="break-words">{p.permissions_summary.join(' / ')}</span>
                </p>
              {/if}
            </div>
          </div>
          <label class="relative inline-flex shrink-0 cursor-pointer items-center">
            <input
              type="checkbox"
              aria-label="{p.name}：有効化"
              class="peer sr-only"
              checked={p.enabled}
              onchange={(e) => toggle(p, (e.target as HTMLInputElement).checked)}
            />
            <div
              class="h-5 w-9 rounded-full bg-surface-muted transition peer-checked:bg-accent"
            ></div>
            <div
              class="absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white shadow transition peer-checked:translate-x-4"
            ></div>
          </label>
        </li>
      {/each}
    </ul>
  {/if}
</main>

<style>
  .plugins {
    height: calc(100vh - var(--ml-nav-h, 64px));
  }
  .plugins > ul > li {
    flex-shrink: 0;
  }
</style>
