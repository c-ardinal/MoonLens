<script lang="ts">
  import { t } from '$lib/i18n';
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
      const n = count.toLocaleString();
      notify(
        scope === undefined
          ? t('訳文キャッシュ {n} 件：消去完了', { n })
          : t('訳文キャッシュ（{scope}） {n} 件：消去完了', { scope: scopeLabel(scope), n })
      );
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }
</script>

<SettingRow
  id="info:translation-cache"
  help={t('保存済の訳文の件数と容量。消去後は次回の表示時に再翻訳。上限 20 万件、超過時は最長未使用分から自動削除。')}
>
  <div class="srow">
    <span class="row-label">{t('訳文キャッシュ使用量')}</span>
    <span class="row-ctl">
      <span class="row-value">
        {#if usage}
          {t('{n} 件・{size}', { n: usage.entries.toLocaleString(), size: cacheSize(usage.bytes) })}
        {:else if error}
          {t('取得不可')}
        {:else}
          {t('確認中…')}
        {/if}
      </span>
      <button
        class="sbtn icon"
        title={t('全消去')}
        aria-label={t('訳文キャッシュを全消去')}
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
    help={t('{scope}の訳文キャッシュ。消去の対象は当該分のみ。', { scope: scopeLabel(s.scope) })}
  >
    <div class="srow">
      <span class="row-label" title={scopeLabel(s.scope)}>{t('内訳：{scope}', { scope: scopeLabel(s.scope) })}</span>
      <span class="row-ctl">
        <span class="row-value">{t('{n} 件・{size}', { n: s.entries.toLocaleString(), size: cacheSize(s.bytes) })}</span>
        <button
          class="sbtn icon"
          title={t('消去')}
          aria-label={t('{scope}の訳文キャッシュを消去', { scope: scopeLabel(s.scope) })}
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
