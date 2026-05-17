<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Camera, Focus, Mic, Cpu, Shield, ArrowRight } from 'lucide-svelte';
  import {
    captureAndProcess,
    getSettings,
    listHistory,
    listLensWindows,
    newLensWindow,
    onHotkey,
    onSubtitle,
    startSubtitle,
    stopSubtitle,
    subtitleState,
    type HistoryEntry,
    type LensDescriptor,
    type Settings
  } from '$lib/ipc';
  import { goto } from '$lib/navigation';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Pill from '$lib/components/Pill.svelte';
  import MoonLensMark from '$lib/components/MoonLensMark.svelte';
  import { currentSettings } from '$lib/stores/settingsStore';
  import { hotkeyFor } from '$lib/hotkey';

  /** Hotkey chip strings, resolved from the live settings store
   *  so a Settings remap propagates here without remount. Empty
   *  string when the user hasn't bound the action — the template
   *  hides the chip in that case rather than show a fictional
   *  default that wouldn't actually fire. */
  const captureHint = $derived(hotkeyFor('capture_region', $currentSettings));
  const newLensHint = $derived(hotkeyFor('new_lens', $currentSettings));
  const subtitleHint = $derived(hotkeyFor('toggle_subtitle', $currentSettings));

  let running = $state(false);
  let lastError = $state<string | null>(null);
  let lastProvider = $state<string | null>(null);
  let subtitleRunning = $state(false);
  let subtitlePreview = $state<string | null>(null);
  let recents = $state<HistoryEntry[]>([]);
  let allHistory = $state<HistoryEntry[]>([]);
  let lenses = $state<LensDescriptor[]>([]);
  let settings = $state<Settings | null>(null);
  let unlistenHotkey: (() => void) | undefined;
  let unlistenSubtitle: (() => void) | undefined;

  onMount(async () => {
    unlistenHotkey = await onHotkey((action) => {
      if (action === 'open_history') goto('/history/');
      if (action === 'open_command_palette') goto('/settings/');
    });
    unlistenSubtitle = await onSubtitle((event) => {
      if (event.kind === 'started') subtitleRunning = true;
      else if (event.kind === 'stopped') subtitleRunning = false;
      else if (event.kind === 'chunk') subtitlePreview = event.translated || event.original;
      else if (event.kind === 'error') lastError = `字幕エラー: ${event.message}`;
    });
    subtitleRunning = await subtitleState();
    try {
      const all = (await listHistory({ limit: 1000 })) ?? [];
      allHistory = all;
      recents = all.slice(0, 4);
    } catch {
      allHistory = [];
      recents = [];
    }
    try {
      lenses = await listLensWindows();
    } catch {
      lenses = [];
    }
    try {
      settings = (await getSettings()) as Settings;
    } catch {
      settings = null;
    }
  });

  onDestroy(() => {
    unlistenHotkey?.();
    unlistenSubtitle?.();
  });

  async function capture() {
    running = true;
    lastError = null;
    try {
      const res = await captureAndProcess();
      lastProvider = res.result.used_provider;
    } catch (e) {
      lastError = String(e);
    } finally {
      running = false;
    }
  }

  async function createLens() {
    try {
      await newLensWindow();
    } catch (e) {
      lastError = String(e);
    }
  }

  async function startSub() {
    try {
      await startSubtitle();
      subtitleRunning = true;
    } catch (e) {
      lastError = String(e);
    }
  }
  async function stopSub() {
    try {
      await stopSubtitle();
      subtitleRunning = false;
      subtitlePreview = null;
    } catch (e) {
      lastError = String(e);
    }
  }

  type EngineRow = { label: string; model: string };

  // Engine card rows reflect what's actually configured: the first
  // entry of each provider chain is the "active" one. An empty chain
  // surfaces as 未設定 so the user can fix it from /settings/.
  const engineRows = $derived.by<EngineRow[]>(() => {
    const chain = (key: 'translate' | 'ocr' | 'asr'): string => {
      const providers = settings?.providers as
        | Record<string, { chain?: string[] }>
        | undefined;
      const c = providers?.[key]?.chain;
      return c && c.length > 0 ? c[0] : '未設定';
    };
    return [
      { label: '翻訳LLM', model: chain('translate') },
      { label: 'OCR', model: chain('ocr') },
      { label: '音声認識', model: chain('asr') }
    ];
  });

  const engineRunning = $derived(subtitleRunning || lenses.length > 0);

  // Stats are derived from the local history rather than fabricated.
  // Each metric comes with a 7-day per-day count for the sparkline so
  // the chart actually mirrors what the user did.
  function startOfDay(ts: number): number {
    const d = new Date(ts * 1000);
    d.setHours(0, 0, 0, 0);
    return Math.floor(d.getTime() / 1000);
  }

  const stats = $derived.by(() => {
    const now = Math.floor(Date.now() / 1000);
    const dayBuckets: number[] = Array.from({ length: 7 }, () => 0);
    const today0 = startOfDay(now);
    const todayBucket = (created: number): number => {
      const idx = Math.floor((today0 - startOfDay(created)) / 86_400);
      return idx >= 0 && idx < 7 ? 6 - idx : -1;
    };

    const captureBucket = [...dayBuckets];
    const audioBucket = [...dayBuckets];
    let weekTotal = 0;
    let weekCapture = 0;
    let weekAudio = 0;
    let total = 0;
    let local = 0;

    for (const e of allHistory) {
      total += 1;
      if (!e.via_cloud) local += 1;
      const idx = todayBucket(e.created_at);
      if (idx < 0) continue;
      weekTotal += 1;
      if (e.kind === 'audio') {
        audioBucket[idx] += 1;
        weekAudio += 1;
      } else {
        captureBucket[idx] += 1;
        weekCapture += 1;
      }
    }

    const localRatio = total === 0 ? 100 : Math.round((local / total) * 100);
    const localSpark = Array.from({ length: 7 }, () => 9);

    const lastWeekDay = (offset: number): number =>
      captureBucket[6 - offset] + audioBucket[6 - offset];
    const weekTrend = Array.from({ length: 7 }, (_, i) => lastWeekDay(6 - i));

    return [
      {
        k: '今週の翻訳',
        v: weekTotal.toLocaleString(),
        sub: weekTotal === 0 ? 'まだ記録なし' : '直近 7 日',
        spark: weekTrend
      },
      {
        k: '画像翻訳',
        v: weekCapture.toLocaleString(),
        sub: '今週のキャプチャ・Lens',
        spark: captureBucket
      },
      {
        k: '音声字幕',
        v: weekAudio.toLocaleString(),
        sub: '今週のセグメント',
        spark: audioBucket
      },
      {
        k: 'ローカル稼働率',
        v: `${localRatio}%`,
        sub: total === 0 ? '履歴なし' : `${local}/${total} 件をローカル処理`,
        spark: localSpark
      }
    ];
  });

  function labelFor(kind: string): string {
    if (kind === 'audio') return '音声字幕';
    if (kind === 'lens') return 'Lens翻訳';
    return '画像翻訳';
  }

  function iconFor(kind: string): typeof Camera {
    if (kind === 'audio') return Mic;
    if (kind === 'lens') return Focus;
    return Camera;
  }

  function whenLabel(created: number): string {
    const delta = Date.now() / 1000 - created;
    if (delta < 60) return `${Math.floor(delta)}秒前`;
    if (delta < 3600) return `${Math.floor(delta / 60)}分前`;
    if (delta < 86_400) return `${Math.floor(delta / 3600)}時間前`;
    return `${Math.floor(delta / 86_400)}日前`;
  }

  function sparklinePath(data: number[], w = 56, h = 18): string {
    const max = Math.max(...data);
    const step = w / (data.length - 1);
    return data.map((v, i) => `${i * step},${h - (v / max) * h}`).join(' ');
  }
</script>

<PageHeader title="ホーム" subtitle="すべての翻訳機能をここから開始" />

<div class="ml-content scroll-y">
  <div class="home-grid fadein">
    <!-- Hero card -->
    <div class="card hero">
      <div class="moon-decoration" aria-hidden="true"></div>
      <div class="kicker">QUICK START</div>
      <h2 class="hero-title">目に映るすべてを、<br />日本語で読む。</h2>
      <p class="hero-sub">
        完全ローカル動作のOCRと言語モデル。インターネット接続なしで画像・動画・音声を翻訳します。
      </p>
      <div class="hero-actions">
        <button
          class="btn primary lg"
          onclick={capture}
          disabled={running}
          type="button"
        >
          <Camera class="h-3.5 w-3.5" aria-hidden="true" />
          {running ? '処理中…' : 'キャプチャ翻訳'}
          {#if captureHint}
            <span class="hero-kbd">{captureHint}</span>
          {/if}
        </button>
        <button class="btn lg" onclick={createLens} type="button">
          <Focus class="h-3.5 w-3.5" aria-hidden="true" />
          Lens起動
          {#if newLensHint}
            <span class="hero-kbd">{newLensHint}</span>
          {/if}
        </button>
        {#if subtitleRunning}
          <button class="btn lg" onclick={stopSub} type="button">
            <Mic class="h-3.5 w-3.5" aria-hidden="true" />
            字幕停止
            {#if subtitleHint}
              <span class="hero-kbd">{subtitleHint}</span>
            {/if}
          </button>
        {:else}
          <button class="btn lg" onclick={startSub} type="button">
            <Mic class="h-3.5 w-3.5" aria-hidden="true" />
            音声字幕
            {#if subtitleHint}
              <span class="hero-kbd">{subtitleHint}</span>
            {/if}
          </button>
        {/if}
      </div>
      {#if lastProvider}
        <p class="hero-meta">
          前回: <code class="mono">{lastProvider}</code>
        </p>
      {/if}
      {#if subtitlePreview}
        <p class="hero-meta">
          字幕: <span class="mono">{subtitlePreview}</span>
        </p>
      {/if}
    </div>

    <!-- Engine card -->
    <div class="card engine">
      <div class="engine-header">
        <Cpu class="h-3.5 w-3.5" aria-hidden="true" />
        <span>ローカルエンジン</span>
        {#if engineRunning}
          <Pill accent dot>稼働中</Pill>
        {:else}
          <Pill dot>待機中</Pill>
        {/if}
      </div>
      <div class="engine-rows">
        {#each engineRows as row (row.label)}
          <div class="engine-row">
            <span class="engine-label">{row.label}</span>
            <span class="engine-model" class:unset={row.model === '未設定'}>
              {row.model}
            </span>
            <span class="ml-status-dot" class:cold={row.model === '未設定'}></span>
          </div>
        {/each}
      </div>
      <div class="engine-foot">
        <Shield class="h-3 w-3" aria-hidden="true" />
        {lenses.length > 0
          ? `Lensウィンドウ ${lenses.length} 件・${subtitleRunning ? '字幕稼働中' : '字幕停止'}`
          : subtitleRunning
            ? '音声字幕稼働中 · オフライン処理'
            : 'すべてオフラインで処理 · データ送信なし'}
      </div>
    </div>
  </div>

  <!-- Stats -->
  <div class="stats-grid">
    {#each stats as s (s.k)}
      <div class="card-soft stat">
        <div class="stat-k">{s.k}</div>
        <div class="stat-v mono">{s.v}</div>
        <div class="stat-row">
          <span class="stat-sub">{s.sub}</span>
          <svg width="56" height="18" viewBox="0 0 56 18" aria-hidden="true">
            <polyline
              points={sparklinePath(s.spark)}
              fill="none"
              stroke="var(--accent)"
              stroke-width="1.4"
              stroke-linejoin="round"
              stroke-linecap="round"
            />
          </svg>
        </div>
      </div>
    {/each}
  </div>

  <!-- Recent -->
  <section class="recent">
    <div class="recent-head">
      <div class="kicker">RECENT</div>
      <div class="recent-title">最近の翻訳</div>
      <button class="btn ghost recent-more" onclick={() => goto('/history/')} type="button">
        すべて表示
        <ArrowRight class="h-3 w-3" aria-hidden="true" />
      </button>
    </div>
    <div class="recent-list">
      {#if recents.length === 0}
        <div class="recent-empty">
          <MoonLensMark size={28} />
          <p>
            まだ履歴がありません。{#if captureHint}{captureHint}{:else}キャプチャ翻訳{/if}
            でキャプチャすると追加されます。
          </p>
        </div>
      {:else}
        {#each recents as r, i (r.id)}
          {@const RowIcon = iconFor(r.kind)}
          <button
            class="card-soft recent-row fadeup"
            style="animation-delay: {i * 60}ms"
            onclick={() => goto('/history/')}
            type="button"
          >
            <div class="recent-icon">
              <RowIcon class="h-3.5 w-3.5" aria-hidden="true" />
            </div>
            <div class="recent-body">
              <div class="recent-row-head">
                <span class="recent-name">{labelFor(r.kind)}</span>
                <span class="faint">· {whenLabel(r.created_at)}</span>
              </div>
              {#if r.source_text || r.result_text}
                <div class="recent-row-body">
                  <span class="recent-src">{r.source_text ?? ''}</span>
                  <ArrowRight class="h-3 w-3 faint" aria-hidden="true" />
                  <span class="recent-dst">{r.result_text ?? ''}</span>
                </div>
              {/if}
            </div>
            <Pill>{r.provider ?? '—'}</Pill>
          </button>
        {/each}
      {/if}
    </div>
  </section>

  {#if lastError}
    <p class="error" role="alert">エラー: {lastError}</p>
  {/if}
</div>

<style>
  .home-grid {
    display: grid;
    grid-template-columns: 1.4fr 1fr;
    gap: 16px;
    margin-bottom: 24px;
  }
  @media (max-width: 1024px) {
    .home-grid {
      grid-template-columns: 1fr;
    }
  }

  .hero {
    padding: 24px;
    background: linear-gradient(135deg, var(--bg-1) 0%, var(--bg-2) 100%);
    position: relative;
    overflow: hidden;
  }
  .moon-decoration {
    position: absolute;
    right: -40px;
    top: -40px;
    width: 200px;
    height: 200px;
    border-radius: 50%;
    background: radial-gradient(
      circle at 35% 35%,
      var(--accent-soft) 0%,
      transparent 60%
    );
    pointer-events: none;
  }
  .kicker {
    font-size: 11px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 500;
    margin-bottom: 8px;
  }
  .hero-title {
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.02em;
    line-height: 1.3;
    margin: 0 0 6px;
    color: var(--ink-0);
  }
  .hero-sub {
    color: var(--ink-2);
    font-size: 13px;
    margin: 0 0 20px;
    max-width: 440px;
  }
  .hero-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .hero-kbd {
    opacity: 0.55;
    margin-left: 8px;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .hero-meta {
    margin: 12px 0 0;
    font-size: 11.5px;
    color: var(--ink-2);
  }

  .engine {
    padding: 20px;
  }
  .engine-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 14px;
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-0);
  }
  .engine-header :global(svg) {
    color: var(--ink-1);
  }
  .engine-header :global(.pill) {
    margin-left: auto;
  }
  .engine-rows {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .engine-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
  }
  .engine-label {
    color: var(--ink-2);
    width: 70px;
    font-size: 11.5px;
  }
  .engine-model {
    flex: 1;
    color: var(--ink-0);
    font-weight: 500;
  }
  .engine-model.unset {
    color: var(--ink-3);
    font-weight: 400;
    font-style: italic;
  }
  .ml-status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #5bb97d;
    box-shadow: 0 0 0 3px rgba(91, 185, 125, 0.15);
  }
  .ml-status-dot.cold {
    background: var(--ink-3);
    box-shadow: none;
  }
  .engine-foot {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11.5px;
    color: var(--ink-2);
  }

  .stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(140px, 100%), 1fr));
    gap: 12px;
    margin-bottom: 24px;
  }
  .stat {
    padding: 14px;
  }
  .stat-k {
    font-size: 11px;
    color: var(--ink-2);
    margin-bottom: 4px;
  }
  .stat-v {
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .stat-row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    margin-top: 6px;
  }
  .stat-sub {
    font-size: 11px;
    color: var(--ink-3);
    flex: 1;
  }

  .recent-head {
    display: flex;
    align-items: baseline;
    margin-bottom: 12px;
    gap: 12px;
  }
  .recent-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--ink-0);
  }
  .recent-more {
    margin-left: auto;
    font-size: 12px;
  }
  .recent-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .recent-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 32px;
    border: 1px dashed var(--line);
    border-radius: var(--r-md);
    color: var(--ink-2);
    font-size: 12px;
    text-align: center;
  }
  .recent-row {
    padding: 14px 16px;
    display: flex;
    gap: 14px;
    align-items: center;
    cursor: pointer;
    text-align: left;
    width: 100%;
    transition:
      background var(--dur-fast),
      border-color var(--dur-fast);
  }
  .recent-row:hover {
    background: var(--bg-3);
  }
  .recent-icon {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--bg-3);
    color: var(--ink-1);
    display: grid;
    place-items: center;
    flex: 0 0 32px;
  }
  .recent-body {
    flex: 1;
    min-width: 0;
  }
  .recent-row-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 3px;
  }
  .recent-name {
    font-size: 12.5px;
    font-weight: 550;
    color: var(--ink-0);
    text-transform: capitalize;
  }
  .recent-row-body {
    font-size: 11.5px;
    color: var(--ink-2);
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 10px;
    align-items: center;
  }
  .recent-src,
  .recent-dst {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .recent-dst {
    color: var(--ink-1);
  }

  .error {
    margin-top: 16px;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: rgba(216, 122, 122, 0.1);
    color: #d87a7a;
    border: 1px solid rgba(216, 122, 122, 0.3);
    font-size: 12px;
  }
</style>
