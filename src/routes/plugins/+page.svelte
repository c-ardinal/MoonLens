<script lang="ts">
  import { onMount } from 'svelte';
  import { Puzzle, Shield, Check } from 'lucide-svelte';
  import { listPlugins, setPluginEnabled, type PluginDescriptor } from '$lib/ipc';

  let plugins = $state<PluginDescriptor[]>([]);
  let loading = $state(true);

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
</script>

<main class="mx-auto flex max-w-4xl flex-col gap-6 p-8">
  <header class="flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold tracking-tight">プラグイン</h1>
      <p class="text-sm text-ink-muted">ユーザ拡張の有効化・権限確認・設定編集。</p>
    </div>
    <a
      href="/plugins/install/"
      class="rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white"
    >
      追加
    </a>
  </header>

  {#if loading}
    <p class="text-ink-faint">読み込み中…</p>
  {:else if plugins.length === 0}
    <div
      class="rounded-ml border border-dashed border-border bg-surface-elevated p-8 text-center text-ink-muted"
    >
      まだプラグインがインストールされていません。
      <br />
      <code class="font-mono text-xs">$APPDATA/plugins/&lt;id&gt;/plugin.toml</code>
      に配置するか、上の「追加」から.moonlens-pluginファイルを読み込んでください。
    </div>
  {:else}
    <ul class="flex flex-col gap-3">
      {#each plugins as p}
        <li
          class="flex items-start justify-between gap-4 rounded-ml border border-border bg-surface-elevated p-4 shadow-ml"
        >
          <div class="flex items-start gap-3">
            <Puzzle class="mt-0.5 h-5 w-5 text-ink-muted" />
            <div>
              <div class="flex items-center gap-2">
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
                  提供: {p.provides.join(', ')}
                </p>
              {/if}
              {#if p.permissions_summary.length}
                <p class="mt-1 flex items-center gap-1 text-xs text-ink-muted">
                  <Shield class="h-3 w-3" />
                  {p.permissions_summary.join(' / ')}
                </p>
              {/if}
            </div>
          </div>
          <label class="relative inline-flex cursor-pointer items-center">
            <input
              type="checkbox"
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
