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
          ? t('translation-cache-usage:translation_cache_n_entries_cleared', { n })
          : t('translation-cache-usage:translation_cache_scope_n_entries_cleared', { scope: scopeLabel(scope), n })
      );
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }
</script>

<SettingRow
  id="info:translation-cache"
  help={t('translation-cache-usage:number_and_size_of_saved_translations_after')}
>
  <div class="srow">
    <span class="row-label">{t('translation-cache-usage:translation_cache_usage')}</span>
    <span class="row-ctl">
      <span class="row-value">
        {#if usage}
          {t('translation-cache-usage:n_entries_size', { n: usage.entries.toLocaleString(), size: cacheSize(usage.bytes) })}
        {:else if error}
          {t('common:unavailable')}
        {:else}
          {t('common:checking')}
        {/if}
      </span>
      <button
        class="sbtn icon"
        title={t('common:clear_all')}
        aria-label={t('translation-cache-usage:clear_all_translation_cache')}
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
    help={t('translation-cache-usage:translation_cache_for_scope_clearing_affects', { scope: scopeLabel(s.scope) })}
  >
    <div class="srow">
      <span class="row-label" title={scopeLabel(s.scope)}>{t('translation-cache-usage:breakdown_scope', { scope: scopeLabel(s.scope) })}</span>
      <span class="row-ctl">
        <span class="row-value">{t('translation-cache-usage:n_entries_size', { n: s.entries.toLocaleString(), size: cacheSize(s.bytes) })}</span>
        <button
          class="sbtn icon"
          title={t('common:clear')}
          aria-label={t('translation-cache-usage:clear_translation_cache_for_scope', { scope: scopeLabel(s.scope) })}
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
