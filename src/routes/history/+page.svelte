<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    Search,
    List as ListIcon,
    Camera,
    Focus,
    Mic,
    Copy,
    Download,
    RefreshCw,
    Trash2,
    Cloud,
    Cpu
  } from 'lucide-svelte';
  import {
    listHistory,
    deleteHistoryEntry,
    clearHistory,
    type HistoryEntry
  } from '$lib/ipc';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { currentSettings } from '$lib/stores/settingsStore';
  import { hotkeyFor } from '$lib/hotkey';

  const captureHint = $derived(hotkeyFor('capture_region', $currentSettings));
  import Pill from '$lib/components/Pill.svelte';
  import Segmented from '$lib/components/Segmented.svelte';

  type Filter = 'all' | 'capture' | 'lens' | 'audio';

  let q = $state('');
  let entries = $state<HistoryEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let confirmClear = $state(false);
  let filter = $state<Filter>('all');
  let selectedId = $state<number | null>(null);
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;
  let copyHint = $state<string | null>(null);
  let copyHintTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => refresh());
  onDestroy(() => {
    if (refreshTimer) clearTimeout(refreshTimer);
    if (copyHintTimer) clearTimeout(copyHintTimer);
  });

  async function refresh(query = q) {
    loading = true;
    error = null;
    try {
      entries = await listHistory({ limit: 500, query });
      if (entries.length > 0 && (selectedId === null || !entries.find((e) => e.id === selectedId))) {
        selectedId = entries[0].id;
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function onQueryInput(value: string) {
    q = value;
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => refresh(value), 180);
  }

  async function remove(id: number) {
    const prev = entries;
    entries = entries.filter((e) => e.id !== id);
    if (selectedId === id) selectedId = entries[0]?.id ?? null;
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
      selectedId = null;
      confirmClear = false;
    } catch (e) {
      error = String(e);
    }
  }

  /** Export the in-memory entries (already query-filtered by the
   *  backend) as a JSON file. The user gets the same shape they would
   *  see in the API — id, kind, source/result text, provider, timestamp
   *  — so external scripts can ingest it directly. */
  function exportJson() {
    if (entries.length === 0) return;
    const payload = JSON.stringify(
      {
        exported_at: new Date().toISOString(),
        count: entries.length,
        entries
      },
      null,
      2
    );
    triggerDownload(
      payload,
      `moonlens-history-${new Date().toISOString().slice(0, 10)}.json`,
      'application/json'
    );
  }

  function triggerDownload(content: string, filename: string, mime: string) {
    const blob = new Blob([content], { type: `${mime};charset=utf-8` });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }

  async function copyDetail() {
    if (!current) return;
    const text = current.result_text ?? current.source_text ?? '';
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      copyHint = 'コピーしました';
    } catch (e) {
      copyHint = `コピー失敗: ${String(e)}`;
    }
    if (copyHintTimer) clearTimeout(copyHintTimer);
    copyHintTimer = setTimeout(() => (copyHint = null), 1800);
  }

  function exportDetail() {
    if (!current) return;
    const lines = [
      `# ${labelFor(current.kind)} — ${dateLabel(current.created_at)}`,
      `provider: ${current.provider ?? '—'} (${current.via_cloud ? 'cloud' : 'local'})`,
      ''
    ];
    if (current.source_text) {
      lines.push('## 原文', current.source_text, '');
    }
    if (current.result_text) {
      lines.push('## 翻訳', current.result_text, '');
    }
    triggerDownload(
      lines.join('\n'),
      `moonlens-${current.kind}-${current.id}.md`,
      'text/markdown'
    );
  }

  function whenLabel(created: number): string {
    const delta = Date.now() / 1000 - created;
    if (delta < 60) return `${Math.floor(delta)}秒前`;
    if (delta < 3600) return `${Math.floor(delta / 60)}分前`;
    if (delta < 86_400) return `${Math.floor(delta / 3600)}時間前`;
    return `${Math.floor(delta / 86_400)}日前`;
  }

  function dateLabel(created: number): string {
    return new Date(created * 1000).toLocaleString('ja-JP', {
      month: 'numeric',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  function iconFor(kind: string) {
    if (kind === 'audio') return Mic;
    if (kind === 'lens') return Focus;
    return Camera;
  }

  function labelFor(kind: string): string {
    if (kind === 'audio') return '音声字幕';
    if (kind === 'lens') return 'Lens';
    return '画像翻訳';
  }

  const filtered = $derived.by(() => {
    if (filter === 'all') return entries;
    if (filter === 'capture') {
      return entries.filter((e) => e.kind !== 'audio' && e.kind !== 'lens');
    }
    return entries.filter((e) => e.kind === filter);
  });

  const current = $derived(entries.find((e) => e.id === selectedId) ?? null);
</script>

<PageHeader title="翻訳履歴" subtitle={loading ? '読み込み中…' : `${entries.length} 件`}>
  {#snippet actions()}
    <button
      class="btn ghost"
      onclick={() => refresh()}
      type="button"
      title="再読み込み"
      aria-label="再読み込み"
    >
      <RefreshCw class="h-3.5 w-3.5" aria-hidden="true" />
    </button>
    {#if entries.length > 0}
      <button class="btn ghost" type="button" onclick={exportJson} title="JSON書き出し">
        <Download class="h-3.5 w-3.5" aria-hidden="true" />
        エクスポート
      </button>
      <button
        class="btn ghost"
        type="button"
        onclick={() => (confirmClear = true)}
        title="全件削除"
      >
        <Trash2 class="h-3.5 w-3.5" aria-hidden="true" />
        全消去
      </button>
    {/if}
  {/snippet}
</PageHeader>

<div class="ml-content history">
  {#if error}
    <div class="error" role="alert">エラー: {error}</div>
  {/if}

  <div class="history-grid fadein">
    <!-- List pane -->
    <div class="list-pane">
      <div class="list-search">
        <div class="search-wrap">
          <Search class="search-ico h-3.5 w-3.5" aria-hidden="true" />
          <input
            class="input search-input"
            value={q}
            placeholder="履歴を検索…"
            oninput={(e) => onQueryInput((e.target as HTMLInputElement).value)}
          />
          <!-- The ⌘F chip used to live here but it never bound to
               an actual shortcut. Removed rather than misrepresent
               capability; if we add an in-app focus shortcut later
               we can put it back keyed to the actual binding. -->
        </div>
        <button class="btn icon" type="button" title="フィルタ" aria-label="フィルタ">
          <ListIcon class="h-3.5 w-3.5" aria-hidden="true" />
        </button>
      </div>

      <Segmented
        value={filter}
        onChange={(v) => (filter = v)}
        options={[
          { value: 'all', label: 'すべて' },
          { value: 'capture', label: '画像' },
          { value: 'lens', label: 'Lens' },
          { value: 'audio', label: '音声' }
        ]}
        aria-label="種別フィルタ"
      />

      <div class="scroll list-scroll">
        {#if !loading && filtered.length === 0}
          <div class="empty">
            {#if q.trim().length > 0}
              「{q}」に一致する履歴はありません。
            {:else}
              まだ履歴がありません。{#if captureHint}{captureHint}{:else}キャプチャ翻訳{/if}
              でキャプチャすると追加されます。
            {/if}
          </div>
        {:else}
          {#each filtered as e (e.id)}
            {@const Icon = iconFor(e.kind)}
            <button
              type="button"
              class="card-soft list-row"
              class:active={selectedId === e.id}
              onclick={() => (selectedId = e.id)}
            >
              <div class="row-icon">
                <Icon class="h-3.5 w-3.5" aria-hidden="true" />
              </div>
              <div class="row-body">
                <div class="row-head">
                  <span class="row-title">{labelFor(e.kind)}</span>
                  {#if e.via_cloud}
                    <Cloud class="h-3 w-3 cloud-ico" aria-hidden="true" />
                  {:else}
                    <Cpu class="h-3 w-3 cpu-ico" aria-hidden="true" />
                  {/if}
                </div>
                <div class="row-meta">
                  <span>{e.provider ?? '—'}</span>
                  <span>·</span>
                  <span>{whenLabel(e.created_at)}</span>
                </div>
                <div class="row-preview">{e.result_text ?? e.source_text ?? ''}</div>
              </div>
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Detail pane -->
    <div class="card detail-pane">
      {#if current}
        {@const Icon = iconFor(current.kind)}
        <div class="detail-head">
          <div class="detail-pills">
            <Pill>{current.via_cloud ? 'Cloud' : 'Local'}</Pill>
            <Pill>{labelFor(current.kind)}</Pill>
            <span class="detail-date">{dateLabel(current.created_at)}</span>
          </div>
          <div class="detail-title">
            <Icon class="h-4 w-4" aria-hidden="true" />
            <span>{current.provider ?? '不明なプロバイダ'}</span>
          </div>
        </div>
        <div class="scroll detail-body">
          {#if current.source_text}
            <div>
              <div class="kicker">原文</div>
              <p class="detail-src">{current.source_text}</p>
            </div>
            <hr class="hr" />
          {/if}
          <div>
            <div class="kicker">翻訳</div>
            <p class="detail-dst">{current.result_text ?? '—'}</p>
          </div>
        </div>
        <div class="detail-foot">
          <button class="btn" type="button" onclick={copyDetail}>
            <Copy class="h-3.5 w-3.5" aria-hidden="true" /> コピー
          </button>
          <button class="btn" type="button" onclick={exportDetail}>
            <Download class="h-3.5 w-3.5" aria-hidden="true" /> 書き出し
          </button>
          {#if copyHint}
            <span class="copy-hint" aria-live="polite">{copyHint}</span>
          {/if}
          <div style="flex: 1"></div>
          <button
            class="btn danger"
            type="button"
            onclick={() => remove(current!.id)}
            aria-label="削除"
            title="この履歴を削除"
          >
            <Trash2 class="h-3.5 w-3.5" aria-hidden="true" />
          </button>
        </div>
      {:else}
        <div class="detail-empty">
          <p>履歴を選択すると詳細が表示されます。</p>
        </div>
      {/if}
    </div>
  </div>
</div>

{#if confirmClear}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={() => (confirmClear = false)}
  >
    <div
      class="card modal"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <h2>履歴を全て削除しますか？</h2>
      <p class="muted">この操作は取り消せません。</p>
      <div class="modal-actions">
        <button class="btn" type="button" onclick={() => (confirmClear = false)}>キャンセル</button>
        <button class="btn primary" type="button" onclick={wipe}>削除する</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .history {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .history-grid {
    display: grid;
    /* `minmax(0, …)` is required so column tracks shrink past their
     * min-content size. Without it, a long single-line preview row
     * pushes the column wider than 1fr would imply, which made the
     * detail pane spill off the right edge. */
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
    gap: 16px;
    height: 100%;
    min-height: 0;
  }

  .list-pane {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    min-width: 0;
  }

  .list-search {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .search-wrap {
    position: relative;
    flex: 1;
  }
  :global(.search-ico) {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--ink-3);
  }
  .search-input {
    padding-left: 30px;
    padding-right: 36px;
  }
  .search-kbd {
    position: absolute;
    right: 10px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 10.5px;
    color: var(--ink-3);
  }

  .list-scroll {
    flex: 1;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-right: 2px;
  }

  .list-row {
    padding: 12px 14px;
    display: flex;
    gap: 12px;
    align-items: flex-start;
    cursor: pointer;
    text-align: left;
    transition: all var(--dur-fast);
  }
  .list-row.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .list-row .row-icon {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--bg-3);
    color: var(--ink-1);
    display: grid;
    place-items: center;
    flex: 0 0 30px;
  }
  .list-row.active .row-icon {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .row-body {
    flex: 1;
    min-width: 0;
  }
  .row-head {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 2px;
  }
  .row-title {
    font-size: 12.5px;
    font-weight: 550;
    color: var(--ink-0);
  }
  :global(.cloud-ico) {
    color: var(--accent);
  }
  :global(.cpu-ico) {
    color: var(--ink-3);
  }
  .row-meta {
    font-size: 11px;
    color: var(--ink-2);
    margin-bottom: 4px;
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .row-preview {
    font-size: 11.5px;
    color: var(--ink-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-pane {
    padding: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
  }
  .detail-head {
    padding: 16px 20px;
    border-bottom: 1px solid var(--line);
  }
  .detail-pills {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .detail-date {
    font-size: 11px;
    color: var(--ink-3);
    margin-left: auto;
  }
  .detail-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
    font-weight: 600;
  }
  .detail-body {
    flex: 1;
    overflow: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .kicker {
    font-size: 10.5px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-bottom: 6px;
    font-weight: 500;
  }
  .detail-src,
  .detail-dst {
    margin: 0;
    white-space: pre-wrap;
    /* Long URLs / glued katakana lines that have no space should
     * still wrap inside the pane instead of stretching the column. */
    overflow-wrap: anywhere;
    word-break: break-word;
  }
  .detail-src {
    font-size: 13px;
    color: var(--ink-1);
    line-height: 1.7;
    font-style: italic;
  }
  .detail-dst {
    font-size: 14px;
    color: var(--ink-0);
    line-height: 1.8;
  }
  .detail-foot {
    padding: 14px;
    border-top: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .copy-hint {
    font-size: 11px;
    color: var(--accent-strong);
    margin-left: 4px;
  }
  .detail-empty {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--ink-2);
    font-size: 13px;
  }

  .empty {
    padding: 24px;
    border: 1px dashed var(--line);
    border-radius: var(--r-md);
    color: var(--ink-2);
    font-size: 12px;
    text-align: center;
  }

  .error {
    margin-bottom: 12px;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: rgba(216, 122, 122, 0.1);
    color: #d87a7a;
    border: 1px solid rgba(216, 122, 122, 0.3);
    font-size: 12px;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(0, 0, 0, 0.4);
  }
  .modal {
    width: 100%;
    max-width: 360px;
  }
  .modal h2 {
    font-size: 14px;
    font-weight: 600;
    margin: 0 0 4px;
  }
  .modal-actions {
    margin-top: 16px;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
