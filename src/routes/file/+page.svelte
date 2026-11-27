<script lang="ts">
  import { t } from '$lib/i18n';
  import { holdMotion } from '$lib/pixel/motion.svelte';
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import { onDestroy, onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Captions, Clapperboard, FileText, FolderOpen, Square } from 'lucide-svelte';
  import {
    cancelFileSubtitle,
    exportFileSubtitle,
    fileSubtitleStatus,
    onFileSubtitle,
    pickMediaFile,
    startFileSubtitle,
    type SubtitleExportContent,
    type SubtitleExportFormat
  } from '$lib/ipc';
  import {
    IDLE,
    PHASE_LABELS,
    positionLabel,
    progressRatio,
    withCue,
    type FileSubtitleCue,
    type FileSubtitleStatus
  } from '$lib/file-subtitle';
  import { timeLabel } from '$lib/subtitle-record';
  import { errorText, parseError, type UiError } from '$lib/errors';
  import { notify, notifyError } from '$lib/toast.svelte';

  let status = $state<FileSubtitleStatus>(IDLE);
  /** 直近の字幕（全件は本体側で保持）。 */
  let recent = $state<FileSubtitleCue[]>([]);
  let error = $state<UiError | null>(null);
  let dragging = $state(false);
  let content = $state<SubtitleExportContent>('translated');
  let list = $state<HTMLOListElement | null>(null);
  const unlisteners: (() => void)[] = [];

  const running = $derived(status.phase === 'running');
  const ratio = $derived(progressRatio(status));
  // 推論中は演出を停止（翻訳性能を優先）
  $effect(() => (running ? holdMotion() : undefined));

  const CONTENT_LABELS: [SubtitleExportContent, string][] = [
    ['translated', 'common:translated_text'],
    ['original', 'common:source'],
    ['both', 'subtitle-record-detail:both']
  ];

  async function start(path: string | null | undefined) {
    if (!path || running) return;
    error = null;
    try {
      status = await startFileSubtitle(path);
      recent = [];
    } catch (e) {
      error = parseError(e);
    }
  }

  async function pick() {
    try {
      await start(await pickMediaFile());
    } catch (e) {
      error = parseError(e);
    }
  }

  async function stop() {
    try {
      status = await cancelFileSubtitle();
    } catch (e) {
      error = parseError(e);
    }
  }

  async function save(format: SubtitleExportFormat) {
    try {
      const path = await exportFileSubtitle(format, content);
      if (path) notify(t('file-subtitle:saved_file', { file: path.split(/[\\/]/).pop() ?? '' }));
    } catch (e) {
      notifyError(t('file-subtitle:save_failed_error', { error: errorText(e) }));
    }
  }

  onMount(async () => {
    try {
      status = (await fileSubtitleStatus()) ?? IDLE;
      unlisteners.push(
        await onFileSubtitle((update) => {
          status = update.status;
          if (update.cue) {
            // 末尾を表示中の場合のみ、追加分へ追従
            const el = list;
            const follow = !el || el.scrollHeight - el.scrollTop - el.clientHeight < 48;
            recent = withCue(recent, update.cue);
            if (el && follow) requestAnimationFrame(() => (el.scrollTop = el.scrollHeight));
          }
        })
      );
      const { getCurrentWebview } = await import('@tauri-apps/api/webview');
      unlisteners.push(
        await getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === 'enter' || p.type === 'over') dragging = !running;
          else if (p.type === 'leave') dragging = false;
          else if (p.type === 'drop') {
            dragging = false;
            start(p.paths[0]);
          }
        })
      );
    } catch {
      /* pure frontend dev */
    }
  });

  onDestroy(() => unlisteners.forEach((u) => u()));
</script>

<main class="px-body mx-auto flex max-w-6xl flex-col gap-4 px-4 py-6 sm:px-6" class:fit={status.phase !== 'idle'} data-fit-viewport={status.phase !== 'idle' ? '' : undefined}>
  <header class="flex flex-wrap items-end justify-between gap-3">
    <div class="min-w-0">
      <div class="page-title"><PixelTitle text={t('common:file_subtitles')} tag="h1" height={40} fit /></div>
      <p class="truncate text-sm text-ink-muted">{t('file-subtitle:description')}</p>
    </div>
    <div class="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-2">
      <button
        class="inline-flex items-center gap-1.5 rounded-ml-sm border border-accent bg-accent px-3 py-1.5 text-sm font-medium text-white shadow-ml transition hover:opacity-90 disabled:opacity-50"
        onclick={pick}
        disabled={running}
      >
        <FolderOpen class="h-4 w-4" />{t('file-subtitle:open_file')}
      </button>
    </div>
  </header>

  {#if error}
    <p class="rounded-ml border border-red-500/50 bg-red-500/10 p-3 text-sm break-words text-red-500" role="alert">
      {error.message}
      {#if error.fix}
        <button class="ml-1 text-accent underline-offset-2 hover:underline" onclick={() => goto(error!.fix!.route)}>
          {error.fix.label}
        </button>
      {/if}
    </p>
  {/if}

  {#if status.phase === 'idle'}
    <button
      class="flex min-h-72 flex-col items-center justify-center gap-3 rounded-ml border-2 border-dashed p-8 text-center transition {dragging
        ? 'border-accent bg-accent-soft'
        : 'border-ink-faint/50 bg-surface-elevated hover:border-accent/60'}"
      onclick={pick}
    >
      <Clapperboard class="h-10 w-10 text-ink-faint" />
      <span class="text-base font-medium">{t('file-subtitle:drop_a_video_or_audio_file')}</span>
      <span class="text-sm text-ink-muted">{t('file-subtitle:formats_hint')}</span>
    </button>
  {:else}
    <section class="rounded-ml border bg-surface-elevated p-4 {dragging ? 'border-accent' : 'border-border'}" aria-label={t('file-subtitle:progress')}>
      <div class="flex flex-wrap items-center gap-x-4 gap-y-2">
        <p class="name truncate text-base font-medium" title={status.name}>{status.name}</p>
        <p class="text-sm tabular-nums text-ink-muted" role="status">
          {t(PHASE_LABELS[status.phase])}・{positionLabel(status)}・{t('file-subtitle:cues_n', { n: status.cues })}
        </p>
        <div class="ml-auto flex flex-wrap items-center gap-2">
          {#if running}
            <button class="sbtn" onclick={stop}><Square class="h-3.5 w-3.5" />{t('common:cancel')}</button>
          {:else}
            <select
              class="rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm text-ink"
              title={t('subtitle-record-detail:export_content')}
              aria-label={t('subtitle-record-detail:export_content')}
              bind:value={content}
            >
              {#each CONTENT_LABELS as [value, label] (value)}
                <option {value}>{t(label)}</option>
              {/each}
            </select>
            <button class="sbtn" onclick={() => save('srt')} disabled={status.cues === 0}>
              <Captions class="h-3.5 w-3.5" />{t('file-subtitle:save_srt')}
            </button>
            <button class="sbtn" onclick={() => save('txt')} disabled={status.cues === 0}>
              <FileText class="h-3.5 w-3.5" />{t('file-subtitle:save_text')}
            </button>
          {/if}
        </div>
      </div>
      <div
        class="bar border border-border bg-surface-muted"
        class:unknown={running && ratio === null}
        role="progressbar"
        aria-label={t('file-subtitle:progress')}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={ratio === null ? undefined : Math.round(ratio * 100)}
      >
        <div class="fill" style="width: {(ratio ?? 0) * 100}%"></div>
      </div>
      {#if status.phase === 'failed' && status.error}
        <p class="mt-2 text-sm break-words text-red-500" role="alert">{status.error}</p>
      {/if}
    </section>

    <section class="cues rounded-ml border border-border bg-surface-elevated" aria-label={t('file-subtitle:latest_subtitles')}>
      <h2 class="border-b border-border px-4 py-2 text-sm text-ink-muted">{t('file-subtitle:latest_subtitles')}</h2>
      {#if recent.length === 0}
        <p class="px-4 py-3 text-sm text-ink-faint">
          {running ? t('file-subtitle:waiting_for_first_subtitle') : t('file-subtitle:no_subtitles_shown')}
        </p>
      {:else}
        <ol bind:this={list}>
          {#each recent as cue (cue.start_ms)}
            <li>
              <span class="time tabular-nums text-ink-faint">{timeLabel(cue.start_ms)}</span>
              <span class="min-w-0">
                <span class="block break-words">{cue.translated || cue.original}</span>
                {#if cue.translated}
                  <span class="block break-words text-sm text-ink-muted">{cue.original}</span>
                {/if}
              </span>
            </li>
          {/each}
        </ol>
      {/if}
    </section>
  {/if}
</main>

<style>
  /* 1 画面収容：見出し行・進行状況の残余を字幕一覧へ配分 */
  main.fit {
    overflow-y: hidden;
  }
  .name {
    flex: 0 1 auto;
    min-width: 0;
    margin: 0;
  }
  .bar {
    height: var(--ml-sp-2);
    margin-top: var(--ml-sp-3);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--ml-accent);
  }
  /* 再生時間不明：進行率の代替として全幅の縞模様（動作なし、推論性能を優先） */
  .bar.unknown .fill {
    width: 100% !important;
    background: repeating-linear-gradient(90deg, var(--ml-accent) 0 8px, transparent 8px 16px);
  }
  .cues {
    display: flex;
    flex: 1 1 0;
    flex-direction: column;
    min-height: 0;
  }
  ol {
    flex: 1 1 0;
    min-height: 0;
    margin: 0;
    padding: var(--ml-sp-2) var(--ml-sp-4);
    overflow-y: auto;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--ml-sp-3);
    padding: var(--ml-sp-1) 0;
  }
</style>
