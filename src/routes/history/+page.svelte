<script lang="ts">
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import { onDestroy, onMount } from 'svelte';
  import { replay, type Effect } from '$lib/pixel/transition';
  import { SvelteSet } from 'svelte/reactivity';
  import { Search, Trash2, RefreshCw, Cloud, Cpu, Copy } from 'lucide-svelte';
  import { copyText, notify, undoable } from '$lib/toast.svelte';
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
    clearHistory,
    type HistoryEntry
  } from '$lib/ipc';

  let q = $state('');
  /** 読込済みの履歴（取消猶予中の削除対象を含む）。 */
  let loaded = $state<HistoryEntry[]>([]);
  /** 削除操作済みの id：取消猶予中は非表示のみ、猶予の満了後に削除を確定。 */
  const removed = new SvelteSet<number>();
  const entries = $derived(loaded.filter((e) => !removed.has(e.id)));
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
    if (text) void copyText(text, which === 'source' ? '原文' : resultLabel(current!));
  }

  /** 結果欄の名称（QR は翻訳なし）。 */
  const resultLabel = (e: HistoryEntry) => (e.kind === 'qr' ? '内容' : '訳文');

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
      loaded = await listHistory({ limit: 500, query });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    refresh();
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
   * 個別削除：一覧から即時に除外し、通知の「取消」で復元。
   * 削除の確定は通知の終了時（画面遷移後も継続）。
   */
  function remove(id: number) {
    // 選択は隣接項目へ移動（先頭への移動を回避）
    const i = visible.findIndex((e) => e.id === id);
    const neighbour = visible[i + 1] ?? visible[i - 1] ?? null;
    undoable({
      text: '履歴 1 件：削除完了',
      failure: '削除失敗',
      apply: () => {
        removed.add(id);
        if (selectedId === id || current?.id === id) selectedId = neighbour?.id ?? null;
      },
      revert: () => {
        removed.delete(id);
        selectedId = id;
      },
      commit: () => deleteHistoryEntry(id)
    });
  }

  async function wipe() {
    try {
      const count = entries.length;
      await clearHistory();
      loaded = [];
      confirmClear = false;
      notify(`履歴 ${count} 件：全消去完了`);
    } catch (e) {
      confirmClear = false;
      error = String(e);
    }
  }

  function when(created: number): string {
    const delta = Date.now() / 1000 - created;
    if (delta < 60) return `${Math.floor(delta)}秒前`;
    if (delta < 3600) return `${Math.floor(delta / 60)}分前`;
    if (delta < 86_400) return `${Math.floor(delta / 3600)}時間前`;
    return `${Math.floor(delta / 86_400)}日前`;
  }

  /** 種別の切替演出：タブの並び順で左右。 */
  const filterIndex = (v: string) => HISTORY_FILTERS.findIndex((f) => f.value === v);
  const filterMotion = (a: string, b: string): Effect => (filterIndex(b) > filterIndex(a) ? 'slide-left' : 'slide-right');
</script>

<!-- 画面高へ固定：一覧・詳細は各自でスクロール（画面全体のはみ出し防止） -->
<main class="px-body history mx-auto flex max-w-5xl flex-col gap-4 px-4 py-6 sm:px-6">
  <header class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <div class="page-title"><PixelTitle text="履歴" tag="h1" height={40} fit /></div>
      <!-- 件数は種別タブ「全件」に表示済み -->
      {#if loading}<p class="text-sm text-ink-muted">読込中…</p>{/if}
    </div>
    <div class="flex min-w-0 items-center gap-2">
      <div class="relative min-w-0 flex-1 sm:flex-none">
        <Search
          class="pointer-events-none absolute left-2 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-faint"
        />
        <input
          value={q}
          oninput={(e) => onQueryInput((e.target as HTMLInputElement).value)}
          placeholder="原文・訳文で検索"
          aria-label="履歴を検索"
          class="w-full rounded-ml-sm sm:w-72 border border-border bg-surface-elevated py-1.5 pl-7 pr-3 text-sm"
        />
      </div>
      <button
        class="inline-flex items-center gap-1 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted transition hover:border-accent/60 hover:text-ink"
        onclick={() => refresh()}
        title="再読込"
        aria-label="再読込"
      >
        <RefreshCw class="h-4 w-4" />
      </button>
      {#if entries.length > 0}
        <button
          class="inline-flex shrink-0 items-center gap-1 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-red-500 transition hover:border-red-500/60"
          onclick={() => (confirmClear = true)}
        >
          <Trash2 class="h-4 w-4" />
          全消去
        </button>
      {/if}
    </div>
  </header>

  {#if error}
    <div
      class="rounded-ml border border-red-500/50 bg-red-500/10 p-3 text-sm text-red-500"
    >
      エラー: {error}
    </div>
  {/if}

  {#if !loading && entries.length === 0}
    <div
      class="rounded-ml border border-dashed border-border bg-surface-elevated p-8 text-center text-ink-muted"
    >
      {#if q.trim().length > 0}
        <p class="break-words">「{q}」：該当なし</p>
      {:else if keepHistory === false}
        <p>履歴保存：オフ（変更は<a href="/settings/#section-history" class="text-accent underline-offset-2 hover:underline">設定</a>）</p>
      {:else}
        <p>履歴なし。翻訳の実行後に表示。</p>
      {/if}
    </div>
  {:else if entries.length > 0}
    <div
      class="scrollbar-minimal flex gap-1 overflow-x-auto"
      role="tablist"
      aria-label="種別"
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
          {f.label}
          <span class="text-xs tabular-nums text-ink-faint">{counts[f.value]}</span>
        </button>
      {/each}
    </div>

    <div class="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)] gap-3 sm:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
      <ul
        class="scrollbar-minimal flex min-h-0 flex-col gap-1.5 overflow-y-auto"
        aria-label="履歴一覧"
        use:replay={{ key: filter, effect: filterMotion }}
      >
        {#if visible.length === 0}
          <li class="rounded-ml border border-dashed border-border p-6 text-center text-sm text-ink-muted">
            該当なし
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
          aria-label="履歴詳細"
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
                  <h2 class="text-xs font-medium text-ink-muted">原文</h2>
                  <button class="sbtn icon neutral" title="原文をコピー" aria-label="原文をコピー" onclick={() => copy('source')}>
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
                    title="{resultLabel(current)}をコピー"
                    aria-label="{resultLabel(current)}をコピー"
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
              title="この履歴を削除"
              aria-label="この履歴を削除"
              onclick={() => current && remove(current.id)}
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
      <h2 class="text-base font-semibold">全履歴を削除</h2>
      <p class="mt-1 text-sm text-ink-muted">この操作は復元不可。</p>
      <div class="mt-4 flex justify-end gap-2">
        <button
          class="rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm"
          onclick={() => (confirmClear = false)}
        >
          キャンセル
        </button>
        <button
          class="rounded-ml-sm bg-red-500 px-3 py-1.5 text-sm font-medium text-white"
          onclick={wipe}
        >
          削除
        </button>
      </div>
    </div>
  </div>
{/if}

