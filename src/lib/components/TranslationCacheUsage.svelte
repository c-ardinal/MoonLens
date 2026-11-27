<script lang="ts">
  // 訳文キャッシュの使用量と消去（設定「訳文キャッシュ保存」の直下）。
  import { onMount } from 'svelte';
  import { Trash2 } from 'lucide-svelte';
  import {
    clearTranslationCache,
    translationCacheUsage,
    type TranslationCacheUsage
  } from '$lib/ipc';
  import { errorText } from '$lib/errors';
  import { notify } from '$lib/toast.svelte';
  import { cacheSize, scopeLabel } from '$lib/translation-cache';
  import SettingRow from './SettingRow.svelte';

  let usage = $state<TranslationCacheUsage | null>(null);
  let error = $state<string | null>(null);

  /** アプリ単位の行：追従先アプリの指定分のみ（共通分のみの場合は合計行で充足）。 */
  const scopes = $derived(usage && usage.scopes.some((s) => s.scope) ? usage.scopes : []);

  async function refresh() {
    try {
      usage = await translationCacheUsage();
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(refresh);

  async function clear(scope?: string) {
    error = null;
    try {
      const count = await clearTranslationCache(scope);
      const name = scope === undefined ? '訳文キャッシュ' : `訳文キャッシュ（${scopeLabel(scope)}）`;
      notify(`${name} ${count.toLocaleString()} 件：消去完了`);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }
</script>

<SettingRow
  id="info:translation-cache"
  help="保存済の訳文の件数と容量。消去後は次回の表示時に再翻訳。上限 20 万件、超過時は最長未使用分から自動削除。"
>
  <div class="srow">
    <span class="row-label">訳文キャッシュ使用量</span>
    <span class="row-ctl">
      <span class="row-value">
        {#if usage}
          {usage.entries.toLocaleString()} 件・{cacheSize(usage.bytes)}
        {:else if error}
          取得不可
        {:else}
          確認中…
        {/if}
      </span>
      <button
        class="sbtn icon"
        title="全消去"
        aria-label="訳文キャッシュを全消去"
        disabled={!usage || usage.entries === 0}
        onclick={() => clear()}
      >
        <Trash2 class="h-4 w-4" />
      </button>
    </span>
  </div>
</SettingRow>
{#each scopes as s (s.scope)}
  <SettingRow
    id="info:translation-cache:{s.scope}"
    help="{scopeLabel(s.scope)}の訳文キャッシュ。消去の対象は当該分のみ。"
  >
    <div class="srow">
      <span class="row-label" title={scopeLabel(s.scope)}>内訳：{scopeLabel(s.scope)}</span>
      <span class="row-ctl">
        <span class="row-value">{s.entries.toLocaleString()} 件・{cacheSize(s.bytes)}</span>
        <button
          class="sbtn icon"
          title="消去"
          aria-label="{scopeLabel(s.scope)}の訳文キャッシュを消去"
          onclick={() => clear(s.scope)}
        >
          <Trash2 class="h-4 w-4" />
        </button>
      </span>
    </div>
  </SettingRow>
{/each}
{#if error}
  <div class="srow error"><p class="row-label">{error}</p></div>
{/if}
