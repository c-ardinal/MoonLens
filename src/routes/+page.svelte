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
    onNotices,
    ocrModelsStatus,
    onHotkey,
    onSettingsChanged,
    onSubtitle,
    openLens,
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
  import HomeCanvas from '$lib/components/HomeCanvas.svelte';
  import { slant } from '$lib/pixel/slant';
  import type { DialItem } from '$lib/pixel/dial';
  import type { HomeRegion } from '$lib/pixel/home';
  import { listHistory } from '$lib/ipc';

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
    const refresh = () => (checkSetup(), loadRecent());
    loadRecent();
    window.addEventListener('focus', refresh);
    unlisteners.push(() => window.removeEventListener('focus', refresh));
    listNotices()
      .then((n) => (notices = n))
      .catch(() => {});
    try {
      unlisteners.push(await onNotices((n) => (notices = n)));
      unlisteners.push(await onSettingsChanged(applySettings));
      unlisteners.push(
        await onHotkey((action) => {
          if (action === 'toggle_pin_overlay_empty')
            lastError = '常時翻訳の対象なし。ウィンドウ翻訳を先に実行。';
        })
      );
      unlisteners.push(
        await onSubtitle((event) => {
          if (event.kind === 'started') subtitleRunning = true;
          else if (event.kind === 'stopped') subtitleRunning = false;
          else if (event.kind === 'chunk') subtitlePreview = event.translated || event.original;
          else if (event.kind === 'error') lastError = `音声翻訳：${event.message}`;
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

  /** 魔法陣の 4 機能。順序は数字キー（1〜4）と一致。 */
  const functions = $derived([
    {
      label: 'ウィンドウ',
      title: 'ウィンドウ翻訳',
      icon: 'region',
      desc: '窓の背面を常時翻訳。複数配置・任意サイズ、最大化で全画面。',
      hotkey: key('capture_region'),
      run: () => openLens()
    },
    {
      label: '画像',
      title: '画像翻訳',
      icon: 'image',
      desc: '画像ファイルの文字を認識・翻訳。ドロップ・貼付に対応。',
      hotkey: '',
      run: () => goto('/image/')
    },
    {
      label: 'テキスト',
      title: 'テキスト翻訳',
      icon: 'text',
      desc: '入力文の翻訳。用語集・文脈指定に対応。',
      hotkey: '',
      run: () => goto('/translate/')
    },
    {
      label: '音声',
      title: '音声翻訳',
      icon: 'mic',
      desc: subtitleRunning ? '字幕表示中。Enter で停止。' : 'システム音声を認識・翻訳し、字幕表示。',
      hotkey: key('toggle_subtitle'),
      // 施錠なし：外部 whisper.cpp サーバー・ASR プラグインも有効なため、可否判定は本体側
      run: toggleSubtitle
    }
  ] satisfies (DialItem & { desc: string; hotkey: string; run: () => unknown })[]);

  let selected = $state(0);
  const current = $derived(functions[selected]);

  /** 最新の履歴 1 件（ホーム下部の「最近」）。 */
  let recent = $state<{ text: string; time: string } | null>(null);
  async function loadRecent() {
    try {
      const [e] = await listHistory({ limit: 1 });
      if (!e) return (recent = null);
      const d = new Date(e.created_at * 1000);
      const text = (e.result_text || e.source_text || '').replace(/\s+/g, ' ').trim();
      recent = { text: text || '（画像）', time: `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}` };
    } catch {
      recent = null;
    }
  }

  const view = $derived({
    items: functions,
    hotkey: current.hotkey,
    running: current.icon === 'mic' && subtitleRunning,
    desc: current.desc,
    preview: current.icon === 'mic' ? subtitlePreview : null,
    // 起動失敗の全文は上部の通知帯へ表示（魔法陣側は 1 行省略のため重複表示しない）
    error: null,
    engines,
    recent,
    hint: `←→選択 Enter・1-${functions.length}起動${key('open_command_palette') ? ` ${key('open_command_palette')}パレット` : ''}`
  });

  function onRegion(r: HomeRegion) {
    if (r === 'go') activate(selected);
    else if (r === 'engines') goto('/settings/#section-models');
    else goto('/history/');
  }

  function activate(i: number) {
    selected = i;
    attempt(functions[i].run);
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
  <!-- 通知・初回セットアップ：魔法陣の上部へ帯状に配置（該当時のみ、重複なし） -->
  <div class="alerts">
      <ConfigIssues />
      {#each notices as n (n.id)}
        <section class="px-win notice" role="alert">
          <p class="min-w-0 flex-1 break-words">
            {n.message}
            {#if n.action}<a href={n.action} class="link">設定▶</a>{/if}
          </p>
          <button class="close" title="閉じる" aria-label="通知を閉じる" onclick={() => dismissNotice(n.id).catch(() => {})}>
            <X class="h-4 w-4" />
          </button>
        </section>
      {/each}
      {#if missing.length > 0}
        <section class="px-win notice">
          <p class="min-w-0 flex-1">{missing.join('・')}未取得。初期設定で推奨モデルを一括取得</p>
          <button class="px-chip" data-tone="amber" use:slant onclick={() => goto('/setup/')}>初期設定</button>
        </section>
      {/if}
      {#if lastError}
        <section class="px-win notice error" role="alert">
          <p class="min-w-0 flex-1 break-words">{lastError}</p>
          <button class="close" title="閉じる" aria-label="エラーを閉じる" onclick={() => (lastError = null)}>
            <X class="h-4 w-4" />
          </button>
        </section>
      {/if}
    </div>
  <div class="stage">
    <HomeCanvas {view} bind:selected onactivate={activate} onregion={onRegion} />
  </div>

  <!-- 支援技術向けの機能一覧（視覚表現は魔法陣） -->
  <ul class="sr-only" aria-label="機能">
    {#each functions as f, i (f.label)}
      <li><button onclick={() => activate(i)}>{i + 1}. {f.label}：{f.desc}</button></li>
    {/each}
  </ul>

</main>

<style>
  .home {
    display: flex;
    flex-direction: column;
    height: calc(100vh - var(--ml-nav-h, 64px));
    overflow: hidden;
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .alerts {
    display: flex;
    flex-direction: column;
    gap: var(--ml-sp-1);
    padding: var(--ml-sp-2) var(--ml-sp-10) 0;
  }
  .alerts:not(:has(> *)) {
    display: none;
  }
  /* 1 行の帯（窓の上下余白を圧縮し、魔法陣の表示寸法を維持） */
  .notice {
    display: flex;
    align-items: center;
    gap: var(--ml-sp-4);
    padding: var(--ml-sp-1) var(--ml-sp-4);
  }
  .close {
    color: var(--ml-px-faint);
  }
  .error p {
    color: var(--ml-px-red);
  }
  .link {
    color: var(--ml-px-amber);
  }
</style>
