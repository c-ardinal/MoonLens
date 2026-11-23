<script lang="ts">
  import { goto } from '$app/navigation';
  import { onDestroy, onMount } from 'svelte';
  import { X } from 'lucide-svelte';
  import {
    dismissNotice,
    getSettings,
    listAsrModels,
    listLlmModels,
    listNotices,
    newLensWindow,
    onNotices,
    ocrModelsStatus,
    onHotkey,
    onSettingsChanged,
    onSubtitle,
    startRegionSelection,
    startSubtitle,
    stopSubtitle,
    subtitleState
  } from '$lib/ipc';
  import { displayAccelerator } from '$lib/hotkey';
  import { languageLabel } from '$lib/lens-overlay';
  import { errorText } from '$lib/errors';
  import ConfigIssues from '$lib/components/ConfigIssues.svelte';
  import { STATE_LABELS, engineRows, type EngineRow } from '$lib/engine-status';
  import type { Notice } from '$lib/ipc';
  import MagicDial from '$lib/components/MagicDial.svelte';
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import { slant } from '$lib/pixel/slant';
  import type { DialItem } from '$lib/pixel/dial';

  let lastError = $state<string | null>(null);
  let subtitleRunning = $state(false);
  let subtitlePreview = $state<string | null>(null);
  let hotkeys = $state<Record<string, string>>({});
  let targetLanguage = $state('ja');
  /** Built-in engines with their model and state (home "local engine" card). */
  let engines = $state<EngineRow[]>([]);
  /** What still has to be downloaded before translation works. */
  let missing = $state<string[]>([]);
  /** Problems found at start-up or in the background (config, hotkeys). */
  let notices = $state<Notice[]>([]);
  const unlisteners: (() => void)[] = [];

  async function checkSetup() {
    try {
      const [ocr, llm, asr] = await Promise.all([ocrModelsStatus(), listLlmModels(), listAsrModels()]);
      engines = engineRows(llm, ocr, asr);
      const out: string[] = [];
      if (engines[0].state !== 'ready') out.push('翻訳モデル');
      // ocrs alone reads Latin only, so PP-OCR is still offered for setup.
      if (!ocr.ppocr) out.push('OCR モデル');
      missing = out;
    } catch {
      engines = [];
      missing = [];
    }
  }

  function applySettings(s: any) {
    checkSetup();
    hotkeys = s?.hotkeys ?? {};
    targetLanguage = s?.subtitle?.target_language ?? 'ja';
  }

  onMount(async () => {
    getSettings().then(applySettings).catch(() => checkSetup());
    window.addEventListener('focus', checkSetup);
    unlisteners.push(() => window.removeEventListener('focus', checkSetup));
    listNotices()
      .then((n) => (notices = n))
      .catch(() => {});
    try {
      unlisteners.push(await onNotices((n) => (notices = n)));
      unlisteners.push(await onSettingsChanged(applySettings));
      unlisteners.push(
        await onHotkey((action) => {
          if (action === 'toggle_pin_overlay_empty')
            lastError = 'ピン留め対象なし。範囲翻訳を先に実行。';
        })
      );
      unlisteners.push(
        await onSubtitle((event) => {
          if (event.kind === 'started') subtitleRunning = true;
          else if (event.kind === 'stopped') subtitleRunning = false;
          else if (event.kind === 'chunk') subtitlePreview = event.translated || event.original;
          else if (event.kind === 'error') lastError = `ライブ字幕：${event.message}`;
        })
      );
      subtitleRunning = await subtitleState();
    } catch {
      /* pure frontend dev */
    }
  });
  onDestroy(() => unlisteners.forEach((u) => u()));

  async function attempt(fn: () => Promise<unknown>) {
    lastError = null;
    try {
      await fn();
    } catch (e) {
      lastError = errorText(e);
    }
  }

  const key = (name: string) => (hotkeys[name] ? displayAccelerator(hotkeys[name]) : '');

  async function toggleSubtitle() {
    if (subtitleRunning) {
      await stopSubtitle();
      subtitleRunning = false;
      subtitlePreview = null;
    } else {
      await startSubtitle();
      subtitleRunning = true;
    }
  }

  const asrReady = $derived(engines.find((e) => e.id === 'asr')?.state === 'ready');

  /** 魔法陣の 5 機能。順序は数字キー（1〜5）と一致。 */
  const functions = $derived([
    {
      label: '範囲翻訳',
      icon: 'region',
      desc: '範囲を静止し、原文位置へ訳文をオーバーレイ表示。',
      hotkey: key('capture_region'),
      run: () => startRegionSelection(false)
    },
    {
      label: '全画面翻訳',
      icon: 'full',
      desc: '画面全体を静止し、原文位置へ訳文をオーバーレイ表示。',
      hotkey: key('capture_window'),
      run: () => startRegionSelection(true)
    },
    {
      label: 'Lens',
      icon: 'lens',
      desc: '定位置の継続翻訳ウィンドウ。動画字幕・ゲーム画面向け。',
      hotkey: '',
      run: newLensWindow
    },
    {
      label: 'ライブ字幕',
      icon: 'mic',
      desc: subtitleRunning ? '字幕表示中。Enter で停止。' : 'システム音声を認識・翻訳し、字幕表示。',
      hotkey: key('toggle_subtitle'),
      // 一覧取得前（engines 空）は施錠しない
      locked: engines.length > 0 && !asrReady && !subtitleRunning,
      run: toggleSubtitle
    },
    {
      label: 'テキスト翻訳',
      icon: 'text',
      desc: '入力文の翻訳。用語集・文脈指定に対応。',
      hotkey: '',
      run: () => goto('/translate/')
    }
  ] satisfies (DialItem & { desc: string; hotkey: string; run: () => unknown })[]);

  let selected = $state(0);
  let bandEl = $state<HTMLElement | null>(null);
  const current = $derived(functions[selected]);

  function activate(i: number) {
    selected = i;
    const f = functions[i];
    if (f.locked) {
      lastError = '音声認識モデル未導入。モデル準備が必要。';
      return;
    }
    attempt(f.run);
  }

  function onKey(e: KeyboardEvent) {
    if (e.ctrlKey || e.altKey || e.metaKey || e.defaultPrevented) return;
    const t = e.target as HTMLElement | null;
    if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
    const n = functions.length;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') selected = (selected + 1) % n;
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') selected = (selected + n - 1) % n;
    else if (e.key === 'Enter' && (!t || t === document.body)) activate(selected);
    else if (/^[1-9]$/.test(e.key) && Number(e.key) <= n) activate(Number(e.key) - 1);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<main class="home">
  <MagicDial items={functions} bind:selected anchor={bandEl} onactivate={activate} />

  <!-- 支援技術・キーボード向けの機能一覧（視覚表現は魔法陣） -->
  <ul class="sr-only" aria-label="機能">
    {#each functions as f, i (f.label)}
      <li><button onclick={() => activate(i)}>{i + 1}. {f.label}{f.locked ? '（未導入）' : ''}</button></li>
    {/each}
  </ul>

  <div class="side">
    <ConfigIssues />

    {#each notices as n (n.id)}
      <section class="px-win notice" role="alert">
        <p class="min-w-0 flex-1 break-words">
          {n.message}
          {#if n.action}
            <a href={n.action} class="link">設定を開く</a>
          {/if}
        </p>
        <button class="close" title="閉じる" aria-label="通知を閉じる" onclick={() => dismissNotice(n.id).catch(() => {})}>
          <X class="h-4 w-4" />
        </button>
      </section>
    {/each}

    {#if missing.length > 0}
      <section class="px-win notice">
        <p class="min-w-0 flex-1">初回セットアップ：{missing.join('・')}のダウンロードが必要。</p>
        <button class="px-chip" data-tone="amber" use:slant onclick={() => goto('/settings/#section-models')}>モデル準備</button>
      </section>
    {/if}

    <PixelTitle text={current.label} note={`${String(selected + 1).padStart(2, '0')}/${String(functions.length).padStart(2, '0')}`} bind:el={bandEl} />

    <div class="chips">
      {#if current.hotkey}<kbd class="px-chip" data-tone="bone" use:slant>{current.hotkey}</kbd>{/if}
      <span class="px-chip" use:slant>直接 {selected + 1}</span>
      {#if current.label === 'ライブ字幕' && subtitleRunning}<span class="px-chip live" use:slant>● 動作中</span>{/if}
    </div>

    <section class="px-win desc">
      <p>{current.desc}</p>
      {#if current.label === 'ライブ字幕' && subtitlePreview}
        <p class="preview">{subtitlePreview}</p>
      {/if}
      {#if lastError}
        <p class="error" role="alert">{lastError}</p>
      {/if}
      <div class="go">
        {#if current.locked}
          <a href="/settings/#section-models" class="link">モデル準備 ▶</a>
        {:else}
          <button class="go-btn" onclick={() => activate(selected)}>
            ▶ Enter で{current.label === 'ライブ字幕' && subtitleRunning ? '停止' : '開始'}
          </button>
        {/if}
      </div>
    </section>

    {#if engines.length > 0}
      <section class="px-win engines" aria-labelledby="engine-heading">
        <h2 id="engine-heading" class="px-chip px-win-title" data-tone="amber" use:slant>エンジン</h2>
        <ul>
          {#each engines as row (row.id)}
            <li>
              <span class="label">{row.label}</span>
              <span class="model" title={row.model}>{row.model}</span>
              <span class="state" data-state={row.state}>{STATE_LABELS[row.state]}</span>
            </li>
          {/each}
        </ul>
        <a href="/settings/#section-models" class="link manage">モデル管理 ▶</a>
      </section>
    {/if}
  </div>

  <p class="hint">
    ←→選択　Enter決定　1〜5直接起動{#if key('open_command_palette')}　{key('open_command_palette')} コマンド{/if}　翻訳先：{languageLabel(targetLanguage) || targetLanguage}
  </p>
</main>

<style>
  .home {
    position: relative;
    height: calc(100vh - 46px);
    min-height: 420px;
    overflow: hidden;
  }
  .side {
    position: absolute;
    top: 18px;
    right: 16px;
    bottom: 40px;
    width: min(420px, 48%);
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow-y: auto;
    padding: 6px 8px 8px 0;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-left: 12px;
  }
  .live {
    color: var(--ml-px-green);
  }
  .desc {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-left: 8px;
  }
  .desc .preview {
    color: var(--ml-px-faint);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .error {
    color: var(--ml-px-red);
    overflow-wrap: anywhere;
  }
  .go {
    display: flex;
    justify-content: flex-end;
  }
  .go-btn {
    color: var(--ml-px-amber);
  }
  .go-btn:hover {
    color: var(--ml-px-bone);
  }
  .engines {
    margin-left: 16px;
    padding-top: 20px;
  }
  .engines ul {
    display: grid;
    gap: 2px;
  }
  .engines li {
    display: grid;
    grid-template-columns: 6.5em minmax(0, 1fr) auto;
    gap: 8px;
  }
  .engines .label {
    color: var(--ml-px-mid);
  }
  .engines .model {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state[data-state='ready'] {
    color: var(--ml-px-green);
  }
  .state[data-state='missing'] {
    color: var(--ml-px-red);
  }
  .state[data-state='downloading'],
  .state[data-state='unset'] {
    color: var(--ml-px-faint);
  }
  .manage {
    display: block;
    margin-top: 6px;
    text-align: right;
  }
  .notice {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .close {
    color: var(--ml-px-faint);
  }
  .link {
    color: var(--ml-px-amber);
  }
  .link:hover {
    color: var(--ml-px-bone);
  }
  .hint {
    position: absolute;
    right: 16px;
    bottom: 10px;
    color: var(--ml-px-faint);
    font-size: 16px;
    white-space: nowrap;
  }
</style>
