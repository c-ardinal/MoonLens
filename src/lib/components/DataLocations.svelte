<script lang="ts">
  import { onMount } from 'svelte';
  import { dataLocations, type DataLocations } from '$lib/ipc';
  import { errorText } from '$lib/errors';

  let info = $state<DataLocations | null>(null);
  let error = $state<string | null>(null);

  const size = (n: number) =>
    n >= 1_073_741_824 ? `${(n / 1_073_741_824).toFixed(1)} GB` : `${(n / 1_048_576).toFixed(1)} MB`;

  onMount(async () => {
    try {
      info = await dataLocations();
    } catch (e) {
      error = errorText(e);
    }
  });
</script>

<section class="rounded-ml border border-border bg-surface-elevated p-4 shadow-ml sm:p-5">
  {#if error}
    <p class="text-xs break-words text-red-500">{error}</p>
  {:else if !info}
    <p class="text-sm text-ink-faint">読込中…</p>
  {:else}
    <dl class="grid gap-3 text-sm">
      <div>
        <dt class="font-medium">モデル <span class="font-normal text-ink-muted">{size(info.models_bytes)}</span></dt>
        <dd class="text-xs break-all text-ink-faint"><code class="font-mono">{info.models_dir}</code></dd>
      </div>
      <div>
        <dt class="font-medium">履歴・プラグイン <span class="font-normal text-ink-muted">{size(info.data_bytes)}</span></dt>
        <dd class="text-xs break-all text-ink-faint"><code class="font-mono">{info.data_dir}</code></dd>
      </div>
      <div>
        <dt class="font-medium">設定ファイル</dt>
        <dd class="text-xs break-all text-ink-faint"><code class="font-mono">{info.config_dir}</code></dd>
      </div>
    </dl>
    <p class="mt-3 text-xs text-ink-muted">
      完全削除：アンインストール後、上記フォルダーを削除。
    </p>
  {/if}
</section>
