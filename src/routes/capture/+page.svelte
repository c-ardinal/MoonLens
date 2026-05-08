<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    Camera,
    Eye,
    Copy,
    Download,
    ArrowRight,
    Folder,
    Loader2,
    AlertCircle
  } from 'lucide-svelte';
  import {
    captureAndProcess,
    captureWindowById,
    listHistory,
    listWindows,
    pickAndCaptureFile,
    startRegionSelection,
    type HistoryEntry,
    type WindowDescriptor
  } from '$lib/ipc';
  import {
    captureState,
    clearCaptureResult,
    setCaptureStage,
    type CaptureStage
  } from '$lib/stores/captureStore';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Pill from '$lib/components/Pill.svelte';
  import Segmented from '$lib/components/Segmented.svelte';

  type Mode = 'region' | 'window' | 'fullscreen' | 'file';

  let mode = $state<Mode>('region');
  let showOriginal = $state(false);
  let recentCaptures = $state<HistoryEntry[]>([]);
  let copyHint = $state<string | null>(null);
  let copyHintTimer: ReturnType<typeof setTimeout> | undefined;
  let showWindowPicker = $state(false);
  let availableWindows = $state<WindowDescriptor[]>([]);
  let loadingWindows = $state(false);
  let windowQuery = $state('');

  // Reactive bindings to the global store. These survive navigation.
  const stage = $derived($captureState.stage);
  const blocks = $derived($captureState.blocks);
  const lastError = $derived($captureState.lastError);
  const lastProvider = $derived($captureState.lastProvider);
  const lastLatencyMs = $derived($captureState.lastLatencyMs);
  const statusMessage = $derived($captureState.statusMessage);
  const streamingText = $derived($captureState.streamingText);

  onMount(async () => {
    try {
      const all = await listHistory({ limit: 6 });
      recentCaptures = all.filter((e) => e.kind !== 'audio');
    } catch {
      recentCaptures = [];
    }
  });

  onDestroy(() => {
    if (copyHintTimer) clearTimeout(copyHintTimer);
  });

  async function startCapture() {
    if (stage === 'capturing' || stage === 'processing') return;
    clearCaptureResult();
    setCaptureStage('capturing', { statusMessage: '準備中…' });
    try {
      switch (mode) {
        case 'region':
          await startRegionSelection();
          // Region selector now drives the rest of the flow via overlay
          // events. If the user cancels (Esc / right-click), the
          // selector closes silently and we never see another event —
          // age out the spinner after a short delay so the page goes
          // back to idle.
          setCaptureStage('idle');
          break;
        case 'fullscreen':
          await captureAndProcess();
          break;
        case 'window':
          setCaptureStage('idle');
          await openWindowPicker();
          break;
        case 'file': {
          const result = await pickAndCaptureFile();
          if (result === null) {
            setCaptureStage('idle');
          }
          break;
        }
      }
    } catch (e) {
      setCaptureStage('error', { lastError: String(e) });
    }
  }

  async function openWindowPicker() {
    showWindowPicker = true;
    loadingWindows = true;
    windowQuery = '';
    try {
      const list = await listWindows();
      availableWindows = list.filter(
        (w) =>
          !(w.app_name === 'MoonLens' || w.title.startsWith('MoonLens'))
      );
    } catch (e) {
      setCaptureStage('error', { lastError: `ウィンドウ列挙失敗: ${e}` });
      availableWindows = [];
    } finally {
      loadingWindows = false;
    }
  }

  async function captureSelectedWindow(id: number) {
    showWindowPicker = false;
    clearCaptureResult();
    setCaptureStage('processing', { statusMessage: 'ウィンドウをキャプチャ中…' });
    try {
      await captureWindowById(id);
    } catch (e) {
      setCaptureStage('error', { lastError: String(e) });
    }
  }

  const filteredWindows = $derived.by<WindowDescriptor[]>(() => {
    const q = windowQuery.trim().toLowerCase();
    if (!q) return availableWindows;
    return availableWindows.filter((w) => {
      const haystack = `${w.title} ${w.app_name}`.toLowerCase();
      return q.split(/\s+/).every((token) => haystack.includes(token));
    });
  });

  async function copyAll() {
    const joined = blocks.map((b) => b.dst).join('\n');
    if (!joined) return;
    try {
      await navigator.clipboard.writeText(joined);
      copyHint = 'コピーしました';
    } catch (e) {
      copyHint = `コピー失敗: ${String(e)}`;
    }
    if (copyHintTimer) clearTimeout(copyHintTimer);
    copyHintTimer = setTimeout(() => (copyHint = null), 1800);
  }

  function saveExtraction() {
    if (blocks.length === 0) return;
    const lines = [
      `# MoonLens Capture — ${new Date().toLocaleString('ja-JP')}`,
      '',
      lastProvider ? `provider: ${lastProvider}` : '',
      lastLatencyMs !== null ? `processing: ${lastLatencyMs} ms` : '',
      '',
      ...blocks.flatMap((b, i) => [
        `## #${String(i + 1).padStart(2, '0')}${b.confidence !== null ? ` (信頼度 ${Math.round(b.confidence * 100)}%)` : ''}`,
        '',
        '原文:',
        '',
        b.src,
        '',
        '翻訳:',
        '',
        b.dst,
        ''
      ])
    ].filter((l) => l !== '');
    const blob = new Blob([lines.join('\n')], {
      type: 'text/markdown;charset=utf-8'
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `moonlens-capture-${Date.now()}.md`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }

  function whenLabel(created: number): string {
    const delta = Date.now() / 1000 - created;
    if (delta < 60) return `${Math.floor(delta)}秒前`;
    if (delta < 3600) return `${Math.floor(delta / 60)}分前`;
    if (delta < 86_400) return `${Math.floor(delta / 3600)}時間前`;
    return `${Math.floor(delta / 86_400)}日前`;
  }

  const isBusy = $derived(stage === 'capturing' || stage === 'processing');
  const buttonLabel = $derived.by<string>(() => {
    if (stage === 'capturing') return '領域選択中…';
    if (stage === 'processing') return '解析中…';
    return '新しいキャプチャ';
  });

  function dismissError() {
    setCaptureStage('idle', { lastError: null });
  }
</script>

<PageHeader title="画像翻訳" subtitle="ワンショットで画像内のテキストを翻訳">
  {#snippet actions()}
    <button class="btn ghost" type="button" onclick={() => (mode = 'file')}>
      <Folder class="h-3.5 w-3.5" aria-hidden="true" />
      ファイルを開く
    </button>
  {/snippet}
</PageHeader>

<div class="ml-content capture">
  <div class="capture-grid fadein">
    <!-- Canvas pane -->
    <div class="canvas-pane">
      <div class="toolbar">
        <Segmented
          value={mode}
          onChange={(v) => (mode = v)}
          options={[
            { value: 'region', label: '範囲' },
            { value: 'window', label: 'ウィンドウ' },
            { value: 'fullscreen', label: '全画面' },
            { value: 'file', label: 'ファイル' }
          ]}
          aria-label="キャプチャモード"
        />
        <div style="flex: 1"></div>
        <button
          class="btn ghost"
          type="button"
          onclick={() => (showOriginal = !showOriginal)}
          disabled={blocks.length === 0}
        >
          <Eye class="h-3.5 w-3.5" aria-hidden="true" />
          {showOriginal ? '翻訳を表示' : '原文を表示'}
        </button>
        <button
          class="btn ghost"
          type="button"
          onclick={copyAll}
          disabled={blocks.length === 0}
        >
          <Copy class="h-3.5 w-3.5" aria-hidden="true" />
          コピー
        </button>
        <button
          class="btn ghost"
          type="button"
          onclick={saveExtraction}
          disabled={blocks.length === 0}
        >
          <Download class="h-3.5 w-3.5" aria-hidden="true" />
          保存
        </button>
        <button class="btn primary" type="button" onclick={startCapture} disabled={isBusy}>
          {#if isBusy}
            <Loader2 class="h-3.5 w-3.5 spin" aria-hidden="true" />
          {:else}
            <Camera class="h-3.5 w-3.5" aria-hidden="true" />
          {/if}
          {buttonLabel}
          {#if !isBusy}
            <span class="kbd-inline">⌘⇧S</span>
          {/if}
        </button>
      </div>

      <div class="canvas">
        <div class="checkerboard" aria-hidden="true"></div>

        {#if blocks.length > 0}
          <div class="result-board">
            {#each blocks as b, i (b.id)}
              <div class="result-block fadeup" style="animation-delay: {i * 50}ms">
                {#if showOriginal}
                  <span class="block-src">{b.src}</span>
                {:else}
                  <span class="block-dst">{b.dst}</span>
                {/if}
                {#if b.confidence !== null}
                  <span class="block-conf mono">{Math.round(b.confidence * 100)}%</span>
                {/if}
              </div>
            {/each}
          </div>
        {:else if stage === 'idle'}
          <div class="canvas-empty">
            <div class="empty-icon">
              <Camera class="h-7 w-7" aria-hidden="true" />
            </div>
            <p class="empty-title">キャプチャを開始すると結果がここに表示されます</p>
            <p class="empty-sub">
              範囲・ウィンドウ・全画面・ファイルから選択して「新しいキャプチャ」をクリック
            </p>
          </div>
        {:else if stage === 'error'}
          <div class="canvas-empty error">
            <div class="empty-icon error-icon">
              <AlertCircle class="h-7 w-7" aria-hidden="true" />
            </div>
            <p class="empty-title">処理に失敗しました</p>
            <p class="empty-sub">{lastError}</p>
            <div class="empty-actions">
              <button class="btn" type="button" onclick={dismissError}>閉じる</button>
              <button class="btn primary" type="button" onclick={startCapture}>再試行</button>
            </div>
          </div>
        {:else}
          <div class="canvas-empty">
            <div class="empty-icon spinning">
              <Loader2 class="h-7 w-7 spin" aria-hidden="true" />
            </div>
            <p class="empty-title">
              {stage === 'capturing'
                ? mode === 'region'
                  ? '領域を選択してください'
                  : '準備中…'
                : 'OCR と翻訳を実行中…'}
            </p>
            {#if streamingText}
              <p class="streaming-preview mono">{streamingText}</p>
            {:else}
              <p class="empty-sub">{statusMessage || 'バックグラウンドで処理しています'}</p>
            {/if}
          </div>
        {/if}

        {#if isBusy}
          <div class="status-bar">
            <Loader2 class="h-3.5 w-3.5 spin" aria-hidden="true" />
            <span>
              {stage === 'capturing' ? '画面取得中' : statusMessage || '解析中'}
            </span>
            <div class="indeterminate-bar" aria-hidden="true">
              <div class="indeterminate-fill"></div>
            </div>
          </div>
        {/if}
      </div>
    </div>

    <!-- Side panel -->
    <aside class="side">
      <div class="card-soft text-card">
        <div class="text-card-head">
          <span>抽出されたテキスト</span>
          <Pill>{blocks.length} ブロック</Pill>
        </div>
        <div class="scroll text-list">
          {#if blocks.length === 0}
            <div class="text-empty muted small">
              {stage === 'idle' || stage === 'error'
                ? 'まだキャプチャがありません。'
                : 'テキストを認識中…'}
            </div>
          {:else}
            {#each blocks as b, i (b.id)}
              <div class="block-row fadeup" style="animation-delay: {Math.min(i * 40, 200)}ms">
                <div class="block-row-head">
                  <span class="mono small muted">#{String(i + 1).padStart(2, '0')}</span>
                  {#if b.confidence !== null}
                    <span class="mono small muted">
                      · {Math.round(b.confidence * 100)}%
                    </span>
                  {/if}
                </div>
                <div class="block-row-src">{b.src}</div>
                {#if b.dst && b.dst !== b.src}
                  <div class="block-row-dst">{b.dst}</div>
                {/if}
              </div>
            {/each}
          {/if}
        </div>
      </div>

      <div class="side-actions">
        <button class="btn" type="button" onclick={copyAll} disabled={blocks.length === 0}>
          <Copy class="h-3.5 w-3.5" aria-hidden="true" /> 全文コピー
        </button>
        <button
          class="btn"
          type="button"
          onclick={saveExtraction}
          disabled={blocks.length === 0}
        >
          <Download class="h-3.5 w-3.5" aria-hidden="true" /> 保存
        </button>
      </div>
      {#if copyHint}
        <p class="copy-hint" aria-live="polite">{copyHint}</p>
      {/if}

      {#if lastProvider || lastLatencyMs !== null}
        <div class="meta-card card-soft">
          <div class="meta-row">
            <span class="muted">プロバイダ</span>
            <span class="mono">{lastProvider ?? '—'}</span>
          </div>
          {#if lastLatencyMs !== null}
            <div class="meta-row">
              <span class="muted">処理時間</span>
              <span class="mono">{lastLatencyMs} ms</span>
            </div>
          {/if}
        </div>
      {/if}

      {#if recentCaptures.length > 0}
        <div class="card-soft recent-card">
          <div class="kicker recent-kicker">最近のキャプチャ</div>
          <ul class="recent-list">
            {#each recentCaptures as r (r.id)}
              <li>
                <span class="recent-text">{r.result_text ?? r.source_text ?? '—'}</span>
                <span class="muted small">{whenLabel(r.created_at)}</span>
              </li>
            {/each}
          </ul>
        </div>
      {/if}
    </aside>
  </div>
</div>

{#if showWindowPicker}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={() => (showWindowPicker = false)}
  >
    <div
      class="card window-modal"
      role="dialog"
      aria-modal="true"
      aria-label="ウィンドウを選択"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => {
        if (e.key === 'Escape') showWindowPicker = false;
      }}
    >
      <header class="window-modal-head">
        <div>
          <div class="kicker">ウィンドウキャプチャ</div>
          <h2>翻訳するウィンドウを選択</h2>
        </div>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="input window-search"
          placeholder="アプリ名・タイトルで絞り込み"
          bind:value={windowQuery}
          autofocus
        />
      </header>
      <div class="scroll window-list">
        {#if loadingWindows}
          <p class="muted small loading-line">ウィンドウを取得中…</p>
        {:else if filteredWindows.length === 0}
          <p class="muted small empty-line">
            {windowQuery.trim()
              ? '一致するウィンドウがありません。'
              : '取得可能なウィンドウがありません。'}
          </p>
        {:else}
          {#each filteredWindows as w (w.id)}
            <button
              class="window-row"
              type="button"
              onclick={() => captureSelectedWindow(w.id)}
              disabled={w.is_minimized}
              title={w.is_minimized
                ? '最小化されたウィンドウはキャプチャできません'
                : '選択してキャプチャ'}
            >
              <div class="window-icon">
                <Camera class="h-4 w-4" aria-hidden="true" />
              </div>
              <div class="window-body">
                <div class="window-title">{w.title || '(無題)'}</div>
                <div class="window-meta">
                  <span class="muted">{w.app_name || '不明なアプリ'}</span>
                  <span class="mono small">
                    {w.w} × {w.h}
                  </span>
                  {#if w.is_minimized}
                    <Pill>最小化</Pill>
                  {/if}
                </div>
              </div>
            </button>
          {/each}
        {/if}
      </div>
      <footer class="window-modal-foot">
        <button class="btn ghost" type="button" onclick={openWindowPicker}>
          再読込
        </button>
        <div style="flex: 1"></div>
        <button
          class="btn"
          type="button"
          onclick={() => (showWindowPicker = false)}
        >
          キャンセル
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .capture {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .capture-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 16px;
    height: 100%;
    min-height: 0;
  }

  .canvas-pane {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    min-width: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .kbd-inline {
    margin-left: 8px;
    opacity: 0.55;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .canvas {
    flex: 1;
    position: relative;
    min-height: 0;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    overflow: hidden;
    display: grid;
    place-items: center;
  }
  .checkerboard {
    position: absolute;
    inset: 0;
    background-image:
      linear-gradient(45deg, var(--bg-2) 25%, transparent 25%),
      linear-gradient(-45deg, var(--bg-2) 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, var(--bg-2) 75%),
      linear-gradient(-45deg, transparent 75%, var(--bg-2) 75%);
    background-size: 20px 20px;
    background-position: 0 0, 0 10px, 10px -10px, -10px 0;
    opacity: 0.3;
    pointer-events: none;
  }

  .result-board {
    position: relative;
    z-index: 1;
    width: 100%;
    height: 100%;
    padding: 28px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: stretch;
  }
  .result-block {
    background: rgba(255, 255, 255, 0.97);
    color: #0c0c0c;
    padding: 8px 12px;
    border-radius: 6px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
    font-family: 'Noto Sans JP', sans-serif;
    line-height: 1.4;
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  .block-src {
    font-style: italic;
    color: #555;
  }
  .block-dst {
    color: #0c0c0c;
    font-weight: 500;
  }
  .block-conf {
    margin-left: auto;
    font-size: 10px;
    color: #888;
  }

  .canvas-empty {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 48px;
    text-align: center;
    max-width: 480px;
  }
  .empty-icon {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: var(--bg-2);
    border: 1px solid var(--line);
    display: grid;
    place-items: center;
    color: var(--ink-2);
  }
  .empty-icon.spinning {
    color: var(--accent);
    border-color: var(--accent-soft);
  }
  .empty-icon.error-icon {
    color: #d87a7a;
    background: rgba(216, 122, 122, 0.08);
    border-color: rgba(216, 122, 122, 0.3);
  }
  .empty-title {
    margin: 0;
    font-size: 14px;
    font-weight: 550;
    color: var(--ink-0);
  }
  .empty-sub {
    margin: 0;
    font-size: 12px;
    color: var(--ink-2);
    line-height: 1.6;
  }
  .empty-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
  .canvas-empty.error .empty-title {
    color: #d87a7a;
  }
  .streaming-preview {
    margin: 0;
    max-width: 420px;
    padding: 12px 14px;
    background: var(--bg-2);
    border-radius: var(--r-sm);
    font-size: 12px;
    color: var(--ink-1);
    line-height: 1.6;
    text-align: left;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .status-bar {
    position: absolute;
    bottom: 16px;
    left: 16px;
    right: 16px;
    padding: 10px 14px;
    background: var(--bg-overlay);
    backdrop-filter: blur(20px);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    z-index: 2;
  }
  /* Indeterminate progress: a sliding strip that loops continuously
   * so the user always sees motion while we wait. The earlier 92%
   * bar dishonestly capped — this version stays honest about not
   * knowing the duration. */
  .indeterminate-bar {
    flex: 1;
    height: 3px;
    background: var(--gauge-track);
    border-radius: 2px;
    overflow: hidden;
    position: relative;
  }
  .indeterminate-fill {
    position: absolute;
    inset: 0;
    width: 35%;
    background: linear-gradient(
      90deg,
      transparent,
      var(--accent) 30%,
      var(--accent) 70%,
      transparent
    );
    animation: indeterminate-slide 1.4s ease-in-out infinite;
  }
  @keyframes indeterminate-slide {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(285%);
    }
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }

  .text-card {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .text-card-head {
    padding: 12px 14px;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    font-weight: 600;
  }
  .text-card-head :global(.pill) {
    margin-left: auto;
  }
  .text-list {
    flex: 1;
    overflow: auto;
    padding: 6px;
  }
  .text-empty {
    padding: 20px;
    text-align: center;
  }
  .block-row {
    padding: 10px 12px;
    border-radius: 8px;
    margin: 2px;
    transition: background var(--dur-fast);
    cursor: default;
  }
  .block-row:hover {
    background: var(--bg-3);
  }
  .block-row-head {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }
  .block-row-src {
    font-size: 11.5px;
    color: var(--ink-3);
    font-style: italic;
    margin-bottom: 4px;
    overflow-wrap: anywhere;
    word-break: break-word;
  }
  .block-row-dst {
    font-size: 12.5px;
    color: var(--ink-0);
    line-height: 1.5;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .side-actions {
    display: flex;
    gap: 8px;
  }
  .side-actions .btn {
    flex: 1;
  }
  .copy-hint {
    margin: 0;
    text-align: center;
    font-size: 11px;
    color: var(--accent-strong);
  }

  .meta-card {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .meta-row {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
  }

  .recent-card {
    padding: 14px;
  }
  .kicker {
    font-size: 10.5px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-weight: 500;
  }
  .recent-kicker {
    margin-bottom: 8px;
  }
  .recent-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .recent-list li {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 11.5px;
  }
  .recent-text {
    flex: 1;
    color: var(--ink-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .muted {
    color: var(--ink-2);
  }
  .small {
    font-size: 11px;
  }
  :global(.spin) {
    animation: ml-spin 0.9s linear infinite;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(0, 0, 0, 0.4);
  }
  .window-modal {
    width: 100%;
    max-width: 560px;
    max-height: 70vh;
    padding: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .window-modal-head {
    display: flex;
    align-items: flex-end;
    gap: 14px;
    padding: 18px 20px 14px;
    border-bottom: 1px solid var(--line);
  }
  .window-modal-head h2 {
    margin: 4px 0 0;
    font-size: 15px;
    font-weight: 600;
  }
  .window-search {
    width: 240px;
  }
  .window-list {
    flex: 1;
    overflow: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .loading-line,
  .empty-line {
    margin: 0;
    padding: 24px;
    text-align: center;
  }
  .window-row {
    display: flex;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: transparent;
    border: 1px solid transparent;
    text-align: left;
    cursor: pointer;
    transition:
      background var(--dur-fast),
      border-color var(--dur-fast);
  }
  .window-row:hover:not(:disabled) {
    background: var(--bg-2);
    border-color: var(--line);
  }
  .window-row:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .window-icon {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--bg-3);
    color: var(--ink-1);
    display: grid;
    place-items: center;
    flex: 0 0 30px;
  }
  .window-body {
    flex: 1;
    min-width: 0;
  }
  .window-title {
    font-size: 12.5px;
    font-weight: 550;
    color: var(--ink-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .window-meta {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    color: var(--ink-2);
    margin-top: 3px;
  }
  .window-modal-foot {
    display: flex;
    gap: 8px;
    padding: 14px 20px;
    border-top: 1px solid var(--line);
  }
</style>
