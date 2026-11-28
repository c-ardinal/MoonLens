<script module lang="ts">
  /** 魔法陣の選択位置。機能画面からの復帰時に直前の選択を復元（画面再生成でも保持）。 */
  let lastSelected = 0;
</script>

<script lang="ts">
  import { t } from '$lib/i18n';
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
      if (engines[0].state !== 'ready') out.push(t('home:translation_model'));
      // ocrs alone reads Latin only, so PP-OCR is still offered for setup.
      if (!ocr.ppocr) out.push(t('home:ocr_model'));
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
            lastError = t('home:nothing_to_translate_continuously_start');
        })
      );
      unlisteners.push(
        await onSubtitle((event) => {
          // 複数ウィンドウ：1 枚の停止は全停止と別、稼働状況を照会。
          if (event.kind === 'started' || event.kind === 'stopped')
            subtitleState().then((on) => (subtitleRunning = on));
          else if (event.kind === 'chunk') subtitlePreview = event.translated || event.original;
          else if (event.kind === 'error') lastError = t('common:audio_translation_message', { message: event.message });
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
      label: t('common:window'),
      title: t('common:window_translation'),
      icon: 'region',
      desc: t('home:translates_what_is_behind_the_window_any'),
      hotkey: key('capture_region'),
      run: () => openLens()
    },
    {
      label: t('common:file'),
      title: t('common:file_translation'),
      icon: 'file',
      desc: t('home:translates_image_video_and_audio_files'),
      hotkey: '',
      run: () => goto('/file/')
    },
    {
      label: t('common:text'),
      title: t('common:text_translation'),
      icon: 'text',
      desc: t('home:translates_typed_text_with_glossary_and'),
      hotkey: '',
      run: () => goto('/translate/')
    },
    {
      label: t('common:audio'),
      title: t('common:audio_translation'),
      icon: 'mic',
      desc: subtitleRunning ? t('home:showing_subtitles_enter_to_stop') : t('home:subtitles_for_pc_audio_and_conversation'),
      hotkey: key('toggle_subtitle'),
      // 施錠なし：外部 whisper.cpp サーバー・ASR プラグインも有効なため、可否判定は本体側
      run: toggleSubtitle
    }
  ] satisfies (DialItem & { desc: string; hotkey: string; run: () => unknown })[]);

  let selected = $state(lastSelected);
  $effect(() => {
    lastSelected = selected;
  });
  const current = $derived(functions[selected]);

  /** 最新の履歴 1 件（ホーム下部の「最近」）。 */
  let recent = $state<{ text: string; time: string } | null>(null);
  async function loadRecent() {
    try {
      const [e] = await listHistory({ limit: 1 });
      if (!e) return (recent = null);
      const d = new Date(e.created_at * 1000);
      const text = (e.result_text || e.source_text || '').replace(/\s+/g, ' ').trim();
      recent = { text: text || t('home:image'), time: `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}` };
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
    hint: key('open_command_palette')
      ? t('home:select_enter_1_n_start_key_palette', { n: functions.length, key: key('open_command_palette') })
      : t('home:select_enter_1_n_start', { n: functions.length })
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
  <div class="alerts px-zoom">
      <ConfigIssues />
      {#each notices as n (n.id)}
        <section class="px-win notice" role="alert">
          <p class="min-w-0 flex-1 break-words">
            {n.message}
            {#if n.action}<a href={n.action} class="link">{t('common:settings')}<span class="px-tri" aria-hidden="true"></span></a>{/if}
          </p>
          <button class="close" title={t('common:close')} aria-label={t('common:dismiss_notification')} onclick={() => dismissNotice(n.id).catch(() => {})}>
            <X class="h-4 w-4" />
          </button>
        </section>
      {/each}
      {#if missing.length > 0}
        <section class="px-win notice">
          <p class="min-w-0 flex-1">{t('home:models_not_downloaded_run_setup_to_get', { models: missing.join(t('・')) })}</p>
          <button class="px-chip" data-tone="amber" use:slant onclick={() => goto('/setup/')}>{t('common:setup')}</button>
        </section>
      {/if}
      {#if lastError}
        <section class="px-win notice error" role="alert">
          <p class="min-w-0 flex-1 break-words">{lastError}</p>
          <button class="close" title={t('common:close')} aria-label={t('home:dismiss_error')} onclick={() => (lastError = null)}>
            <X class="h-4 w-4" />
          </button>
        </section>
      {/if}
    </div>
  <div class="stage">
    <HomeCanvas {view} bind:selected onactivate={activate} onregion={onRegion} />
  </div>

  <!-- 支援技術向けの機能一覧（視覚表現は魔法陣） -->
  <ul class="sr-only" aria-label={t('home:features')}>
    {#each functions as f, i (f.icon)}
      <li><button onclick={() => activate(i)}>{t('home:n_label_desc', { n: i + 1, label: f.label, desc: f.desc })}</button></li>
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
  /* 1 行の帯（ウィンドウの上下余白を圧縮し、魔法陣の表示寸法を維持） */
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
    display: inline-flex;
    align-items: center;
    gap: var(--ml-sp-2);
    vertical-align: bottom;
    color: var(--ml-px-amber);
  }
</style>
