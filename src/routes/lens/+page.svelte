<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    Plus,
    Focus,
    RefreshCw,
    X,
    Clock,
    Crop,
    Cpu
  } from 'lucide-svelte';
  import {
    captureLensNow,
    closeLensWindow,
    listHistory,
    listLensWindows,
    newLensWindow,
    type HistoryEntry,
    type LensDescriptor
  } from '$lib/ipc';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Pill from '$lib/components/Pill.svelte';
  import Kbd from '$lib/components/Kbd.svelte';

  let lenses = $state<LensDescriptor[]>([]);
  let recentLensHistory = $state<HistoryEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state<Set<string>>(new Set());
  let creating = $state(false);
  let pollTimer: ReturnType<typeof setInterval> | undefined;

  onMount(async () => {
    await refresh();
    // Polling here is cheap (the IPC just clones a Vec) and keeps the
    // list in sync if the user creates / closes lens windows from the
    // tray menu while this page is open.
    pollTimer = setInterval(refresh, 2_000);
  });

  onDestroy(() => {
    if (pollTimer !== undefined) clearInterval(pollTimer);
  });

  async function refresh() {
    try {
      const [l, h] = await Promise.all([
        listLensWindows(),
        listHistory({ limit: 30 })
      ]);
      lenses = l;
      recentLensHistory = h.filter((e) => e.kind === 'lens').slice(0, 8);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function createLens() {
    if (creating) return;
    creating = true;
    error = null;
    try {
      await newLensWindow();
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      creating = false;
    }
  }

  async function closeLens(id: string) {
    if (busy.has(id)) return;
    busy = new Set([...busy, id]);
    try {
      await closeLensWindow(id);
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      const next = new Set(busy);
      next.delete(id);
      busy = next;
    }
  }

  async function recapture(id: string) {
    if (busy.has(id)) return;
    busy = new Set([...busy, id]);
    try {
      await captureLensNow(id);
    } catch (e) {
      error = String(e);
    } finally {
      const next = new Set(busy);
      next.delete(id);
      busy = next;
    }
  }

  function shortId(id: string): string {
    return id.replace(/^lens-/, '').split('-').pop() ?? id;
  }

  function autoRefreshLabel(ms: number | null): string {
    if (ms === null || ms === undefined) return '手動のみ';
    if (ms < 1_000) return `${ms} ms`;
    return `${(ms / 1_000).toFixed(1)} 秒`;
  }

  function whenLabel(created: number): string {
    const delta = Date.now() / 1000 - created;
    if (delta < 60) return `${Math.floor(delta)}秒前`;
    if (delta < 3600) return `${Math.floor(delta / 60)}分前`;
    if (delta < 86_400) return `${Math.floor(delta / 3600)}時間前`;
    return `${Math.floor(delta / 86_400)}日前`;
  }
</script>

<PageHeader title="Lensウィンドウ" subtitle="画面の一部に重ねてリアルタイム翻訳">
  {#snippet actions()}
    <button
      class="btn primary"
      type="button"
      onclick={createLens}
      disabled={creating}
    >
      <Plus class="h-3.5 w-3.5" aria-hidden="true" />
      {creating ? '作成中…' : '新規Lens'}
      <span class="kbd-inline">⌘L</span>
    </button>
  {/snippet}
</PageHeader>

<div class="ml-content scroll-y lens">
  {#if error}
    <p class="error" role="alert">エラー: {error}</p>
  {/if}

  <section class="active-section fadein">
    <div class="section-head">
      <span class="section-kicker">アクティブ</span>
      <span class="muted small">{lenses.length} 件</span>
    </div>

    {#if loading}
      <p class="muted">読み込み中…</p>
    {:else if lenses.length === 0}
      <div class="empty card-soft">
        <Focus class="h-8 w-8 muted" aria-hidden="true" />
        <p>表示中のLensウィンドウはありません。</p>
        <p class="muted small">
          「新規Lens」ボタンか <Kbd label="⌘L" /> で起動できます。
        </p>
      </div>
    {:else}
      <div class="lens-grid">
        {#each lenses as lens (lens.id)}
          <article class="card lens-card">
            <header class="lens-card-head">
              <div class="lens-card-title">
                <Focus class="h-4 w-4 accent-text" aria-hidden="true" />
                <span class="lens-name">{lens.title}</span>
              </div>
              <span class="mono small muted">#{shortId(lens.id)}</span>
            </header>

            <dl class="lens-meta">
              <div class="meta-row">
                <dt class="muted small">
                  <Clock class="h-3 w-3" aria-hidden="true" /> 自動更新
                </dt>
                <dd class="mono small">{autoRefreshLabel(lens.auto_refresh_ms)}</dd>
              </div>
              <div class="meta-row">
                <dt class="muted small">
                  <Crop class="h-3 w-3" aria-hidden="true" /> インセット
                </dt>
                <dd class="mono small">
                  T{lens.insets.top} R{lens.insets.right} B{lens.insets.bottom} L{lens.insets.left}
                </dd>
              </div>
            </dl>

            <footer class="lens-actions">
              <button
                class="btn"
                type="button"
                onclick={() => recapture(lens.id)}
                disabled={busy.has(lens.id)}
              >
                <RefreshCw class="h-3.5 w-3.5" aria-hidden="true" />
                再翻訳
              </button>
              <button
                class="btn danger"
                type="button"
                onclick={() => closeLens(lens.id)}
                disabled={busy.has(lens.id)}
              >
                <X class="h-3.5 w-3.5" aria-hidden="true" />
                閉じる
              </button>
            </footer>
          </article>
        {/each}
      </div>
    {/if}
  </section>

  <section class="history-section fadein">
    <div class="section-head">
      <span class="section-kicker">最近のLens翻訳</span>
      <a class="btn ghost section-more" href="/history/">すべて表示</a>
    </div>

    {#if recentLensHistory.length === 0}
      <div class="empty card-soft small-empty">
        <p class="muted small">
          Lensウィンドウからの翻訳結果はまだありません。Lensを起動して翻訳したい領域に重ねると、ここに記録されます。
        </p>
      </div>
    {:else}
      <ul class="history-list">
        {#each recentLensHistory as h (h.id)}
          <li class="card-soft history-row">
            <div class="history-icon">
              <Focus class="h-3.5 w-3.5" aria-hidden="true" />
            </div>
            <div class="history-body">
              <div class="history-head">
                <span class="history-provider mono small">{h.provider ?? '—'}</span>
                <span class="muted small">· {whenLabel(h.created_at)}</span>
                {#if h.via_cloud}
                  <Pill>Cloud</Pill>
                {:else}
                  <Pill>Local</Pill>
                {/if}
              </div>
              {#if h.source_text}
                <p class="history-src">{h.source_text}</p>
              {/if}
              {#if h.result_text}
                <p class="history-dst">{h.result_text}</p>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <aside class="hint-card card-soft">
    <Cpu class="h-4 w-4 accent-text" aria-hidden="true" />
    <div>
      <div class="hint-title">使い方のヒント</div>
      <p class="muted small">
        Lensウィンドウは半透明の常駐ウィンドウです。翻訳したい領域に重ねて配置すると、自動でテキスト検出と翻訳が行われます。ウィンドウは複数同時に起動可能で、それぞれが独立して動作します。
      </p>
    </div>
  </aside>
</div>

<style>
  .lens {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .section-head {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 12px;
  }
  .section-kicker {
    font-size: 10.5px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-weight: 500;
  }
  .section-more {
    margin-left: auto;
    font-size: 12px;
  }

  .empty {
    padding: 32px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
    color: var(--ink-1);
  }
  .empty p {
    margin: 0;
  }
  .small-empty {
    padding: 20px;
  }

  .lens-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 12px;
  }
  .lens-card {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .lens-card-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .lens-card-title {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }
  .lens-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lens-meta {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .meta-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .meta-row dt {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .meta-row dd {
    margin: 0;
    color: var(--ink-1);
  }

  .lens-actions {
    display: flex;
    gap: 6px;
    margin-top: auto;
  }
  .lens-actions .btn {
    flex: 1;
  }

  .history-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .history-row {
    padding: 12px 14px;
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .history-icon {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: var(--bg-3);
    color: var(--ink-1);
    display: grid;
    place-items: center;
    flex: 0 0 28px;
  }
  .history-body {
    flex: 1;
    min-width: 0;
  }
  .history-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }
  .history-src {
    margin: 0 0 4px;
    font-size: 11.5px;
    color: var(--ink-3);
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .history-dst {
    margin: 0;
    font-size: 12.5px;
    color: var(--ink-0);
    line-height: 1.5;
  }

  .hint-card {
    margin-top: 8px;
    padding: 14px;
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .hint-title {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--ink-0);
    margin-bottom: 3px;
  }

  :global(.accent-text) {
    color: var(--accent);
  }
  .muted {
    color: var(--ink-2);
  }
  .small {
    font-size: 11px;
  }
  .kbd-inline {
    margin-left: 8px;
    opacity: 0.55;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .error {
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: rgba(216, 122, 122, 0.1);
    color: #d87a7a;
    border: 1px solid rgba(216, 122, 122, 0.3);
    font-size: 12px;
    margin: 0 0 12px;
  }
</style>
