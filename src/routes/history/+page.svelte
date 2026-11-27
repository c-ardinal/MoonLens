<script module lang="ts">
  /**
   * 一覧の再読込を要する事象（取消による復元・削除失敗）の発生回数。画面の実体間で共有：
   * 削除後に他画面へ遷移し再表示した場合も、取消の結果を表示中の一覧へ反映。
   */
  let restores = $state(0);
  /** 表示中の「取消」付き通知。全消去時に閉鎖（消去後の単独復元を防止）。 */
  const undoToasts = new Set<number>();
</script>

<script lang="ts">
  import { t } from '$lib/i18n';
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import { onDestroy, onMount, untrack } from 'svelte';
  import { replay, type Effect } from '$lib/pixel/transition';
  import { Search, Trash2, RefreshCw, Cloud, Cpu, Copy } from 'lucide-svelte';
  import { close as closeToast, copyText, notify, notifyError, undoable } from '$lib/toast.svelte';
  import { errorText } from '$lib/errors';
  import {
    HISTORY_FILTERS,
    applyFilter,
    countByFilter,
    kindLabel,
    nextSelection,
    type HistoryFilter
  } from '$lib/history';
  import {
    getSettings,
    listHistory,
    deleteHistoryEntry,
    restoreHistoryEntry,
    clearHistory,
    type HistoryEntry
  } from '$lib/ipc';

  let q = $state('');
  let entries = $state<HistoryEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let confirmClear = $state(false);
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;
  let keepHistory = $state<boolean | null>(null);
  let filter = $state<HistoryFilter>('all');
  let selectedId = $state<number | null>(null);

  const visible = $derived(applyFilter(entries, filter));
  const counts = $derived(countByFilter(entries));
  const current = $derived(visible.find((e) => e.id === nextSelection(visible, selectedId)) ?? null);

  function copy(which: 'source' | 'result') {
    const text = which === 'source' ? current?.source_text : current?.result_text;
    if (text) void copyText(text, which === 'source' ? t('原文') : resultLabel(current!));
  }

  /** 結果欄の名称（QR は翻訳なし）。 */
  const resultLabel = (e: HistoryEntry) => (e.kind === 'qr' ? t('内容') : t('訳文'));

  function dateLabel(created: number): string {
    return new Date(created * 1000).toLocaleString('ja-JP', {
      year: 'numeric',
      month: 'numeric',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }


  async function refresh(query = q) {
    loading = true;
    error = null;
    try {
      entries = await listHistory({ limit: 500, query });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // 初回表示・取消による復元の都度、一覧を読込（検索語の変更は入力側で処理）
  $effect(() => {
    void restores;
    untrack(() => refresh());
  });
  onMount(() => {
    getSettings()
      .then((s) => (keepHistory = s?.storage?.keep_history ?? true))
      .catch(() => {});
  });
  onDestroy(() => {
    if (refreshTimer) clearTimeout(refreshTimer);
  });

  function onQueryInput(value: string) {
    q = value;
    // Debounce so the backend is not hit on every keystroke.
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => refresh(value), 180);
  }

  /**
   * 個別削除：即時に削除し、通知の「取消」で元の id・日時のまま復元。
   * 削除は実行済みのため、通知の表示中にアプリを終了しても削除結果は維持。
   */
  async function remove(entry: HistoryEntry) {
    // 選択は隣接項目へ移動（先頭への移動を回避）
    const i = visible.findIndex((e) => e.id === entry.id);
    const neighbour = visible[i + 1] ?? visible[i - 1] ?? null;
    // 応答前に一覧から除外：連打時の同一項目への重複操作を防止
    entries = entries.filter((e) => e.id !== entry.id);
    selectedId = neighbour?.id ?? null;
    let deleted: number;
    try {
      deleted = await deleteHistoryEntry(entry.id);
    } catch (e) {
      notifyError(t('削除失敗：{error}', { error: errorText(e) }));
      restores += 1;
      return;
    }
    // 削除済みの項目（0 行）：取消の対象なし
    if (deleted === 0) return;
    const toast = undoable({
      text: t('履歴 1 件：削除完了'),
      failure: t('復元失敗'),
      undo: async () => {
        await restoreHistoryEntry($state.snapshot(entry));
        selectedId = entry.id;
        restores += 1;
      }
    });
    undoToasts.add(toast);
  }

  async function wipe() {
    try {
      // 件数は実際の削除行数（検索中・500 件超過時は表示件数と不一致）
      // 消去の発行前に「取消」付き通知を閉鎖：消去後の単独復元を防止
      for (const id of undoToasts) closeToast(id, 'dismiss');
      undoToasts.clear();
      const count = await clearHistory();
      entries = [];
      confirmClear = false;
      notify(t('履歴 {count} 件：全消去完了', { count }));
    } catch (e) {
      confirmClear = false;
      error = String(e);
    }
  }

  function when(created: number): string {
    const delta = Date.now() / 1000 - created;
    if (delta < 60) return t('{n}秒前', { n: Math.floor(delta) });
    if (delta < 3600) return t('{n}分前', { n: Math.floor(delta / 60) });
    if (delta < 86_400) return t('{n}時間前', { n: Math.floor(delta / 3600) });
    return t('{n}日前', { n: Math.floor(delta / 86_400) });
  }

  /** 種別の切替演出：タブの並び順で左右。 */
  const filterIndex = (v: string) => HISTORY_FILTERS.findIndex((f) => f.value === v);
  const filterMotion = (a: string, b: string): Effect => (filterIndex(b) > filterIndex(a) ? 'slide-left' : 'slide-right');
</script>

<!-- 画面高へ固定：一覧・詳細は各自でスクロール（画面全体のはみ出し防止） -->
<main class="px-body history mx-auto flex max-w-5xl flex-col gap-4 px-4 py-6 sm:px-6">
  <header class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <div class="page-title"><PixelTitle text={t('履歴')} tag="h1" height={40} fit /></div>
      <!-- 件数は種別タブ「全件」に表示済み -->
      {#if loading}<p class="text-sm text-ink-muted">{t('読込中…')}</p>{/if}
    </div>
    <div class="flex min-w-0 items-center gap-2">
      <div class="relative min-w-0 flex-1 sm:flex-none">
        <Search
          class="pointer-events-none absolute left-2 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-faint"
        />
        <input
          value={q}
          oninput={(e) => onQueryInput((e.target as HTMLInputElement).value)}
          placeholder={t('原文・訳文で検索')}
          aria-label={t('履歴を検索')}
          class="w-full rounded-ml-sm sm:w-72 border border-border bg-surface-elevated py-1.5 pl-7 pr-3 text-sm"
        />
      </div>
      <button
        class="inline-flex items-center gap-1 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted transition hover:border-accent/60 hover:text-ink"
        onclick={() => refresh()}
        title={t('再読込')}
        aria-label={t('再読込')}
      >
        <RefreshCw class="h-4 w-4" />
      </button>
      {#if entries.length > 0}
        <button
          class="inline-flex shrink-0 items-center gap-1 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-red-500 transition hover:border-red-500/60"
          onclick={() => (confirmClear = true)}
        >
          <Trash2 class="h-4 w-4" />
          {t('全消去')}
        </button>
      {/if}
    </div>
  </header>

  {#if error}
    <div
      class="rounded-ml border border-red-500/50 bg-red-500/10 p-3 text-sm text-red-500"
    >
      {t('エラー: {error}', { error })}
    </div>
  {/if}

  {#if !loading && entries.length === 0}
    <div
      class="rounded-ml border border-dashed border-border bg-surface-elevated p-8 text-center text-ink-muted"
    >
      {#if q.trim().length > 0}
        <p class="break-words">{t('「{q}」：該当なし', { q })}</p>
      {:else if keepHistory === false}
        <p>{t('履歴保存：オフ。')}<a href="/settings/#section-history" class="text-accent underline-offset-2 hover:underline">{t('設定で変更')}</a></p>
      {:else}
        <p>{t('履歴なし。翻訳の実行後に表示。')}</p>
      {/if}
    </div>
  {:else if entries.length > 0}
    <div
      class="scrollbar-minimal flex gap-1 overflow-x-auto"
      role="tablist"
      aria-label={t('種別')}
    >
      {#each HISTORY_FILTERS.filter((f) => f.value === 'all' || f.value === filter || counts[f.value] > 0) as f (f.value)}
        <button
          role="tab"
          aria-selected={filter === f.value}
          class="inline-flex shrink-0 items-center gap-1.5 rounded-ml-sm px-3 py-1.5 text-sm transition {filter === f.value
            ? 'bg-accent-soft text-ink'
            : 'text-ink-muted hover:bg-surface-muted hover:text-ink'}"
          onclick={() => (filter = f.value)}
        >
          {t(f.label)}
          <span class="text-xs tabular-nums text-ink-faint">{counts[f.value]}</span>
        </button>
      {/each}
    </div>

    <div class="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)] gap-3 sm:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
      <ul
        class="scrollbar-minimal flex min-h-0 flex-col gap-1.5 overflow-y-auto"
        aria-label={t('履歴一覧')}
        use:replay={{ key: filter, effect: filterMotion }}
      >
        {#if visible.length === 0}
          <li class="rounded-ml border border-dashed border-border p-6 text-center text-sm text-ink-muted">
            {t('該当なし')}
          </li>
        {/if}
        {#each visible as e (e.id)}
          <li>
            <button
              class="w-full rounded-ml border px-2.5 py-[var(--ml-row-pad)] text-left transition {current?.id === e.id
                ? 'border-accent/60 bg-accent-soft'
                : 'border-border bg-surface-elevated hover:border-accent/40'}"
              aria-current={current?.id === e.id ? 'true' : undefined}
              onclick={() => (selectedId = e.id)}
            >
              <span class="flex items-center gap-2 text-xs text-ink-faint">
                <span class="rounded-full bg-surface-muted px-2 py-0.5">{kindLabel(e.kind)}</span>
                <time datetime={new Date(e.created_at * 1000).toISOString()}>{when(e.created_at)}</time>
              </span>
              <span class="mt-1 line-clamp-2 break-words text-sm leading-snug">
                {e.result_text ?? e.source_text ?? ''}
              </span>
            </button>
          </li>
        {/each}
      </ul>

      {#if current}
        <section
          class="flex min-h-0 min-w-0 flex-col rounded-ml border border-border bg-surface-elevated shadow-ml"
          aria-label={t('履歴詳細')}
          use:replay={{ key: current.id, effect: 'open' }}
        >
          <div class="flex flex-wrap items-center gap-2 border-b border-border px-4 py-3 text-xs text-ink-faint">
            <span class="rounded-full bg-surface-muted px-2 py-0.5">{kindLabel(current.kind)}</span>
            <time datetime={new Date(current.created_at * 1000).toISOString()}>{dateLabel(current.created_at)}</time>
            {#if current.provider}
              <span class="inline-flex items-center gap-1 font-mono">
                {#if current.via_cloud}
                  <Cloud class="h-3 w-3 text-accent" />
                {:else}
                  <Cpu class="h-3 w-3" />
                {/if}
                {current.provider}
              </span>
            {/if}
          </div>
          <div class="scrollbar-minimal flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
            {#if current.source_text}
              <div>
                <div class="mb-1 flex items-center justify-between">
                  <h2 class="text-xs font-medium text-ink-muted">{t('原文')}</h2>
                  <button class="sbtn icon neutral" title={t('原文をコピー')} aria-label={t('原文をコピー')} onclick={() => copy('source')}>
                    <Copy class="h-3.5 w-3.5" />
                  </button>
                </div>
                <p class="font-mono text-xs leading-relaxed break-words whitespace-pre-wrap text-ink-muted">{current.source_text}</p>
              </div>
            {/if}
            {#if current.result_text}
              <div>
                <div class="mb-1 flex items-center justify-between">
                  <h2 class="text-xs font-medium text-ink-muted">{resultLabel(current)}</h2>
                  <button
                    class="sbtn icon neutral"
                    title={t('{label}をコピー', { label: resultLabel(current) })}
                    aria-label={t('{label}をコピー', { label: resultLabel(current) })}
                    onclick={() => copy('result')}
                  >
                    <Copy class="h-3.5 w-3.5" />
                  </button>
                </div>
                <p class="text-sm leading-relaxed break-words whitespace-pre-wrap">{current.result_text}</p>
              </div>
            {/if}
          </div>
          <div class="flex justify-end border-t border-border px-4 py-2">
            <button
              class="sbtn icon"
              title={t('当該履歴を削除')}
              aria-label={t('当該履歴を削除')}
              onclick={() => current && remove(current)}
            >
              <Trash2 class="h-3.5 w-3.5" />
            </button>
          </div>
        </section>
      {/if}
    </div>
  {/if}
</main>

{#if confirmClear}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
    role="presentation"
    onclick={() => (confirmClear = false)}
  >
    <div
      class="w-full max-w-md rounded-ml border border-border bg-surface-elevated p-5 shadow-ml-lg"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === 'Escape' && (confirmClear = false)}
    >
      <h2 class="text-base font-semibold">{t('全履歴を削除')}</h2>
      <p class="mt-1 text-sm text-ink-muted">{t('本操作は復元不可。')}</p>
      <div class="mt-4 flex justify-end gap-2">
        <button
          class="rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm"
          onclick={() => (confirmClear = false)}
        >
          {t('キャンセル')}
        </button>
        <button
          class="rounded-ml-sm bg-red-500 px-3 py-1.5 text-sm font-medium text-white"
          onclick={wipe}
        >
          {t('削除')}
        </button>
      </div>
    </div>
  </div>
{/if}

