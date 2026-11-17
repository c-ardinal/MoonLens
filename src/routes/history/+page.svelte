<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Search, Trash2, RefreshCw, Cloud, Cpu, XCircle } from 'lucide-svelte';
  import {
    getSettings,
    listHistory,
    deleteHistoryEntry,
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

  const KIND_LABELS: Record<string, string> = {
    translate: '翻訳',
    capture: '画面翻訳',
    ocr: '文字認識',
    qr: 'QR コード',
    explain: '説明',
    summarize: '要約',
    transcript: '字幕',
    asr: '字幕'
  };

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

  async function remove(id: number) {
    // Optimistic UI so the click feels instant; roll back if the
    // backend says otherwise.
    const prev = entries;
    entries = entries.filter((e) => e.id !== id);
    try {
      await deleteHistoryEntry(id);
    } catch (e) {
      entries = prev;
      error = String(e);
    }
  }

  async function wipe() {
    try {
      await clearHistory();
      entries = [];
      confirmClear = false;
    } catch (e) {
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
</script>

<main class="mx-auto flex max-w-3xl flex-col gap-4 px-4 py-6 sm:px-6">
  <header class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-xl font-semibold tracking-tight">履歴</h1>
      <p class="text-sm text-ink-muted">
        {#if loading}
          読み込み中…
        {:else}
          {entries.length} 件
        {/if}
      </p>
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
        title="再読み込み"
        aria-label="再読み込み"
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
        <p>履歴なし。画面翻訳の実行後に表示。</p>
      {/if}
    </div>
  {/if}

  <ul class="flex flex-col gap-2">
    {#each entries as e (e.id)}
      <li class="group rounded-ml border border-border bg-surface-elevated p-3 shadow-ml">
        <div class="flex items-center justify-between gap-2 text-xs text-ink-faint">
          <span class="flex min-w-0 flex-wrap items-center gap-2">
            <time datetime={new Date(e.created_at * 1000).toISOString()} title={new Date(e.created_at * 1000).toLocaleString('ja-JP')}>
              {when(e.created_at)}
            </time>
            <span class="rounded-full bg-surface-muted px-2 py-0.5">
              {KIND_LABELS[e.kind] ?? e.kind}
            </span>
            {#if e.provider}
              <span class="inline-flex items-center gap-1 font-mono">
                {#if e.via_cloud}
                  <Cloud class="h-3 w-3 text-accent" />
                {:else}
                  <Cpu class="h-3 w-3" />
                {/if}
                {e.provider}
              </span>
            {/if}
          </span>
          <button
            class="shrink-0 rounded-ml-sm p-1 opacity-60 transition group-hover:opacity-100 focus-visible:opacity-100"
            onclick={() => remove(e.id)}
            title="削除"
            aria-label="この履歴を削除"
          >
            <XCircle class="h-4 w-4 text-ink-faint hover:text-red-500" />
          </button>
        </div>
        {#if e.source_text}
          <p class="mt-1 font-mono text-xs break-words text-ink-faint whitespace-pre-wrap">
            {e.source_text}
          </p>
        {/if}
        {#if e.result_text}
          <p class="mt-1 text-sm leading-relaxed break-words whitespace-pre-wrap">{e.result_text}</p>
        {/if}
      </li>
    {/each}
  </ul>
</main>

{#if confirmClear}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
    role="presentation"
    onclick={() => (confirmClear = false)}
  >
    <div
      class="w-full max-w-sm rounded-ml border border-border bg-surface-elevated p-5 shadow-ml-lg"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === 'Escape' && (confirmClear = false)}
    >
      <h2 class="text-base font-semibold">全履歴を削除</h2>
      <p class="mt-1 text-sm text-ink-muted">この操作は取り消し不可。</p>
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
