<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    Mic,
    StopCircle,
    Languages,
    Copy,
    Download,
    RefreshCw,
    Layers,
    AudioWaveform
  } from 'lucide-svelte';
  import {
    getSettings,
    onSubtitle,
    startSubtitle,
    stopSubtitle,
    subtitleState,
    testAsrEndpoint,
    type AsrEndpointReport,
    type Settings,
    type SubtitleEvent
  } from '$lib/ipc';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Pill from '$lib/components/Pill.svelte';
  import Segmented from '$lib/components/Segmented.svelte';
  import Toggle from '$lib/components/Toggle.svelte';

  type Source = 'mic' | 'system' | 'file';

  interface Subtitle {
    id: number;
    t: string;
    src: string;
    dst: string;
    stable: boolean;
  }

  let source = $state<Source>('system');
  let recording = $state(false);
  let lastError = $state<string | null>(null);
  let subtitles = $state<Subtitle[]>([]);
  let nextId = 1;
  let elapsed = $state(0); // seconds
  let elapsedTimer: ReturnType<typeof setInterval> | undefined;
  let unlisten: (() => void) | undefined;
  // Display toggles are user preferences that should outlive a page
  // reload. We store them under a stable key so the UI starts with the
  // same shape the user left it.
  const STORAGE_KEY = 'moonlens.audio.display';
  type DisplayPrefs = {
    original: boolean;
    timecode: boolean;
    autoscroll: boolean;
  };

  function readPrefs(): DisplayPrefs {
    if (typeof window === 'undefined') {
      return { original: true, timecode: true, autoscroll: true };
    }
    try {
      const raw = window.localStorage.getItem(STORAGE_KEY);
      if (!raw) return { original: true, timecode: true, autoscroll: true };
      const parsed = JSON.parse(raw) as Partial<DisplayPrefs>;
      return {
        original: parsed.original ?? true,
        timecode: parsed.timecode ?? true,
        autoscroll: parsed.autoscroll ?? true
      };
    } catch {
      return { original: true, timecode: true, autoscroll: true };
    }
  }

  const initial = readPrefs();
  let displayOriginal = $state(initial.original);
  let displayTimecode = $state(initial.timecode);
  let displayAutoscroll = $state(initial.autoscroll);
  let listEl = $state<HTMLDivElement | null>(null);
  let settings = $state<Settings | null>(null);
  let asrReport = $state<AsrEndpointReport | null>(null);
  let testing = $state(false);

  async function runAsrTest() {
    if (testing) return;
    testing = true;
    asrReport = null;
    try {
      asrReport = await testAsrEndpoint();
    } catch (e) {
      asrReport = {
        reachable: false,
        status: null,
        url: '',
        provider: '',
        message: String(e)
      };
    } finally {
      testing = false;
    }
  }

  $effect(() => {
    if (typeof window === 'undefined') return;
    const prefs: DisplayPrefs = {
      original: displayOriginal,
      timecode: displayTimecode,
      autoscroll: displayAutoscroll
    };
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  });

  const asrModel = $derived.by<string>(() => {
    const providers = settings?.providers as
      | Record<string, { chain?: string[] }>
      | undefined;
    const chain = providers?.asr?.chain;
    return chain && chain.length > 0 ? chain[0] : '未設定';
  });

  const translateModel = $derived.by<string>(() => {
    const providers = settings?.providers as
      | Record<string, { chain?: string[] }>
      | undefined;
    const chain = providers?.translate?.chain;
    return chain && chain.length > 0 ? chain[0] : '未設定';
  });

  onMount(async () => {
    unlisten = await onSubtitle((event: SubtitleEvent) => {
      if (event.kind === 'started') {
        recording = true;
        startElapsed();
      } else if (event.kind === 'stopped') {
        recording = false;
        stopElapsed();
      } else if (event.kind === 'chunk') {
        appendChunk(event.original, event.translated);
      } else if (event.kind === 'error') {
        lastError = event.message;
      }
    });
    recording = await subtitleState();
    if (recording) startElapsed();
    try {
      settings = (await getSettings()) as Settings;
    } catch {
      settings = null;
    }
  });

  onDestroy(() => {
    unlisten?.();
    stopElapsed();
  });

  function startElapsed() {
    stopElapsed();
    elapsedTimer = setInterval(() => {
      elapsed += 1;
    }, 1000);
  }
  function stopElapsed() {
    if (elapsedTimer !== undefined) {
      clearInterval(elapsedTimer);
      elapsedTimer = undefined;
    }
  }

  function fmtElapsed(s: number): string {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    return [h, m, sec].map((n) => String(n).padStart(2, '0')).join(':');
  }

  function appendChunk(src: string, dst: string) {
    const stamp = fmtElapsed(elapsed);
    subtitles = [
      ...subtitles,
      { id: nextId++, t: stamp, src, dst: dst || src, stable: true }
    ];
    if (displayAutoscroll) {
      // Defer to the next frame so the new node is in the DOM.
      requestAnimationFrame(() => {
        listEl?.scrollTo({ top: listEl.scrollHeight, behavior: 'smooth' });
      });
    }
  }

  async function toggleRecord() {
    lastError = null;
    try {
      if (recording) {
        await stopSubtitle();
        recording = false;
        stopElapsed();
      } else {
        await startSubtitle();
        recording = true;
        elapsed = 0;
        startElapsed();
      }
    } catch (e) {
      lastError = String(e);
    }
  }

  async function copyAll() {
    const lines = subtitles
      .map((s) => `[${s.t}] ${displayOriginal ? `${s.src}\n     ` : ''}${s.dst}`)
      .join('\n');
    if (!lines) return;
    try {
      await navigator.clipboard.writeText(lines);
    } catch {
      /* swallow — extension permission not granted */
    }
  }

  function exportSrt() {
    if (subtitles.length === 0) return;
    const srt = subtitles
      .map((s, i) => {
        const t = s.t;
        return `${i + 1}\n${t},000 --> ${t},999\n${s.dst}\n`;
      })
      .join('\n');
    const blob = new Blob([srt], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `moonlens-${Date.now()}.srt`;
    a.click();
    URL.revokeObjectURL(url);
  }

  // Pseudo-random but deterministic bar heights so the waveform looks
  // alive without flicker between renders.
  const waveformBars = Array.from({ length: 56 }, (_, i) => {
    const seed = (Math.sin(i * 12.9898) * 43758.5453) % 1;
    return Math.abs(seed) * 0.7 + 0.15;
  });
</script>

<PageHeader title="音声字幕" subtitle="システム音声・マイクをリアルタイム字幕化">
  {#snippet actions()}
    <button class="btn ghost" type="button" onclick={exportSrt} disabled={subtitles.length === 0}>
      <Download class="h-3.5 w-3.5" aria-hidden="true" />
      SRT書き出し
    </button>
    <button class="btn ghost" type="button" onclick={copyAll} disabled={subtitles.length === 0}>
      <Copy class="h-3.5 w-3.5" aria-hidden="true" />
      全文コピー
    </button>
  {/snippet}
</PageHeader>

<div class="ml-content audio">
  {#if lastError}
    <div class="banner error" role="alert">
      <strong>エラー:</strong>
      <span>{lastError}</span>
      <p class="banner-hint">
        音声字幕は ASR プロバイダ ({asrModel}) を必要とします。設定 → プロバイダ → asr で
        構成と接続先を確認してください (既定では <code class="mono">whisper_cpp.large_v3_turbo</code> が <code class="mono">http://127.0.0.1:9002</code> を参照します)。
      </p>
    </div>
  {/if}

  <div class="audio-grid fadein">
    <div class="main-col">
      <div class="source-row">
        <Segmented
          value={source}
          onChange={(v) => (source = v)}
          options={[
            { value: 'mic', label: 'マイク' },
            { value: 'system', label: 'システム音声' },
            { value: 'file', label: 'ファイル' }
          ]}
          aria-label="入力ソース"
        />
        <div class="source-meta">
          <AudioWaveform class="h-3 w-3" aria-hidden="true" />
          {source === 'mic' ? '内蔵マイク' : source === 'system' ? 'システムループバック' : 'ファイル選択'}
        </div>
      </div>

      <div class="card-soft transport">
        <button
          class="btn primary record-btn"
          type="button"
          onclick={toggleRecord}
          aria-pressed={recording}
        >
          {#if recording}
            <StopCircle class="h-4 w-4" aria-hidden="true" />
          {:else}
            <Mic class="h-4 w-4" aria-hidden="true" />
          {/if}
        </button>
        <div class="transport-status">
          <span class="status-label">{recording ? '録音中' : '停止'}</span>
          <span class="status-time mono" class:active={recording}>{fmtElapsed(elapsed)}</span>
        </div>

        <div class="waveform" aria-hidden="true">
          {#each waveformBars as h, i (i)}
            <span
              class="wave-bar"
              class:active={recording}
              style="--h: {h * 100}%; --d: {0.8 + (i % 6) * 0.15}s"
            ></span>
          {/each}
        </div>

        <div class="transport-meters">
          <span class="muted small">レベル <span class="mono ink-0">{recording ? '-12.4 dB' : '—'}</span></span>
          <span class="muted small">VAD <span class="mono vad" class:active={recording}>{recording ? 'active' : 'idle'}</span></span>
        </div>
      </div>

      <div class="card-soft stream">
        <div class="stream-head">
          <Languages class="h-3.5 w-3.5" aria-hidden="true" />
          <span>ライブ字幕</span>
          <Pill>EN → JA</Pill>
          <Pill accent dot>{recording ? 'live' : 'paused'}</Pill>
        </div>
        <div class="scroll stream-scroll" bind:this={listEl}>
          {#if subtitles.length === 0}
            <p class="stream-empty muted small">
              {recording ? '音声を待機中…' : '録音を開始すると字幕がここに流れます。'}
            </p>
          {:else}
            {#each subtitles as s, i (s.id)}
              <div class="sub-row fadeup" style="animation-delay: {Math.min(i * 30, 240)}ms">
                {#if displayTimecode}
                  <span class="sub-time mono small">{s.t}</span>
                {/if}
                <div class="sub-body">
                  {#if displayOriginal && s.src && s.src !== s.dst}
                    <div class="sub-src">{s.src}</div>
                  {/if}
                  <div class="sub-dst">{s.dst}</div>
                </div>
              </div>
            {/each}
            {#if recording}
              <div class="processing">
                <span class="dot"></span>
                次のセグメントを処理中…
              </div>
            {/if}
          {/if}
        </div>
      </div>
    </div>

    <aside class="side">
      <div class="card-soft side-card">
        <div class="kicker">セッション</div>
        <div class="session-name">ライブセッション</div>
        <div class="muted small">{recording ? '録音中' : '待機中'} · {subtitles.length} セグメント</div>
      </div>

      <div class="card-soft side-card">
        <div class="kicker">表示</div>
        <div class="setting-row">
          <span>原文を表示</span>
          <Toggle checked={displayOriginal} onChange={(v) => (displayOriginal = v)} />
        </div>
        <div class="setting-row">
          <span>タイムコード</span>
          <Toggle checked={displayTimecode} onChange={(v) => (displayTimecode = v)} />
        </div>
        <div class="setting-row">
          <span>自動スクロール</span>
          <Toggle checked={displayAutoscroll} onChange={(v) => (displayAutoscroll = v)} />
        </div>
      </div>

      <div class="card-soft side-card">
        <div class="kicker">モデル</div>
        <div class="model-name" class:unset={asrModel === '未設定'}>
          {asrModel}
        </div>
        <div class="muted small">翻訳: {translateModel}</div>
        <button
          class="btn side-btn"
          type="button"
          onclick={runAsrTest}
          disabled={testing}
        >
          <RefreshCw
            class="h-3.5 w-3.5 {testing ? 'spin' : ''}"
            aria-hidden="true"
          />
          {testing ? 'テスト中…' : 'テスト接続'}
        </button>
        {#if asrReport}
          <div
            class="probe-result"
            class:probe-ok={asrReport.reachable}
            class:probe-fail={!asrReport.reachable}
            aria-live="polite"
          >
            <div class="probe-headline">
              {asrReport.reachable ? '接続OK' : '接続失敗'}
              {#if asrReport.status !== null}
                <span class="mono small">HTTP {asrReport.status}</span>
              {/if}
            </div>
            {#if asrReport.url}
              <code class="mono probe-url">{asrReport.url}</code>
            {/if}
            <p class="probe-message">{asrReport.message}</p>
          </div>
        {/if}
        <a class="btn side-btn" href="/settings/">
          設定を開く
        </a>
      </div>

      <div class="card-soft side-card">
        <div class="kicker">オーバーレイ字幕</div>
        <p class="muted small overlay-help">
          {recording
            ? '字幕オーバーレイは画面下部に表示中です。'
            : '記録を開始すると、画面下部の常時最前面ウィンドウに字幕が表示されます。'}
        </p>
        <button
          class="btn primary side-btn"
          type="button"
          onclick={toggleRecord}
          aria-pressed={recording}
        >
          <Layers class="h-3.5 w-3.5" aria-hidden="true" />
          {recording ? '記録を停止' : '記録を開始'}
        </button>
      </div>
    </aside>
  </div>

</div>

<style>
  .audio {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .audio-grid {
    display: grid;
    grid-template-columns: 1fr 320px;
    gap: 16px;
    height: 100%;
    min-height: 0;
  }
  @media (max-width: 1080px) {
    .audio-grid {
      grid-template-columns: 1fr;
    }
  }

  .main-col {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }

  .source-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .source-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--ink-2);
  }

  .transport {
    padding: 16px;
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .record-btn {
    width: 44px;
    height: 44px;
    padding: 0;
    border-radius: 50%;
    flex: 0 0 44px;
  }
  .transport-status {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 110px;
  }
  .status-label {
    font-size: 11px;
    color: var(--ink-2);
  }
  .status-time {
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--ink-0);
  }
  .status-time.active {
    color: var(--accent);
  }

  .waveform {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 2px;
    height: 36px;
  }
  .wave-bar {
    flex: 1;
    height: var(--h, 50%);
    background: var(--bg-4);
    border-radius: 1px;
    opacity: 0.5;
    transform-origin: center;
  }
  .wave-bar.active {
    background: var(--accent);
    opacity: 0.85;
    animation: wave-flicker var(--d, 1s) ease-in-out infinite alternate;
  }
  @keyframes wave-flicker {
    to {
      transform: scaleY(0.4);
    }
  }

  .transport-meters {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .ink-0 {
    color: var(--ink-0);
  }
  .vad {
    color: var(--ink-2);
  }
  .vad.active {
    color: #7dd3a0;
  }

  .stream {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .stream-head {
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    font-weight: 600;
  }
  .stream-head :global(.pill):nth-of-type(1) {
    margin-left: auto;
  }
  .stream-scroll {
    flex: 1;
    overflow: auto;
    padding: 8px 0;
  }
  .stream-empty {
    padding: 32px 16px;
    text-align: center;
  }
  .sub-row {
    padding: 10px 16px;
    display: flex;
    gap: 14px;
    border-left: 2px solid transparent;
  }
  .sub-time {
    color: var(--ink-3);
    min-width: 56px;
    padding-top: 2px;
  }
  .sub-body {
    flex: 1;
    min-width: 0;
  }
  .sub-src {
    font-size: 11px;
    color: var(--ink-3);
    margin-bottom: 3px;
    font-style: italic;
  }
  .sub-dst {
    font-size: 13.5px;
    color: var(--ink-0);
    line-height: 1.5;
  }
  .processing {
    padding: 6px 16px;
    color: var(--ink-3);
    font-size: 11px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .processing .dot {
    width: 5px;
    height: 5px;
    background: var(--accent);
    border-radius: 50%;
    animation: ml-pulse 1.2s ease-in-out infinite;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }
  .side-card {
    padding: 14px;
  }
  .kicker {
    font-size: 10.5px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-weight: 500;
    margin-bottom: 8px;
  }
  .session-name,
  .model-name {
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 4px;
  }
  .model-name.unset {
    color: var(--ink-3);
    font-weight: 400;
    font-style: italic;
  }
  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 5px 0;
    font-size: 12px;
    color: var(--ink-1);
  }
  .side-btn {
    width: 100%;
    margin-top: 10px;
  }
  .probe-result {
    margin-top: 10px;
    padding: 10px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    font-size: 11.5px;
    color: var(--ink-1);
    line-height: 1.5;
  }
  .probe-result.probe-ok {
    background: rgba(91, 185, 125, 0.08);
    border-color: rgba(91, 185, 125, 0.35);
  }
  .probe-result.probe-fail {
    background: rgba(216, 122, 122, 0.08);
    border-color: rgba(216, 122, 122, 0.35);
  }
  .probe-headline {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    color: var(--ink-0);
    margin-bottom: 4px;
  }
  .probe-url {
    display: block;
    margin-bottom: 6px;
    color: var(--ink-2);
    word-break: break-all;
  }
  .probe-message {
    margin: 0;
    color: var(--ink-1);
  }
  :global(.spin) {
    animation: ml-spin 0.9s linear infinite;
  }
  .overlay-help {
    margin: 4px 0 0;
  }

  .muted {
    color: var(--ink-2);
  }
  .small {
    font-size: 11px;
  }

  .banner.error {
    margin: 0 0 12px;
    padding: 10px 14px;
    border-radius: var(--r-md);
    background: rgba(216, 122, 122, 0.08);
    color: #d87a7a;
    border: 1px solid rgba(216, 122, 122, 0.25);
    font-size: 12px;
    flex: 0 0 auto;
  }
  .banner.error strong {
    margin-right: 6px;
  }
  .banner-hint {
    margin: 6px 0 0;
    color: var(--ink-1);
    font-size: 11.5px;
    line-height: 1.6;
  }
  .banner-hint code {
    background: var(--bg-2);
    padding: 1px 4px;
    border-radius: 3px;
  }
</style>
