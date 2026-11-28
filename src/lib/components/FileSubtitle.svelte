<script lang="ts">
  import { t } from '$lib/i18n';
  import { holdMotion } from '$lib/pixel/motion.svelte';
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import MediaPlayer from '$lib/components/MediaPlayer.svelte';
  import { goto } from '$app/navigation';
  import { Captions, FileText, FolderOpen, Square } from 'lucide-svelte';
  import {
    burnFileSubtitle,
    cancelBurnFileSubtitle,
    exportFileSubtitle,
    onFileBurn,
    type SubtitleExportContent,
    type SubtitleExportFormat
  } from '$lib/ipc';
  import {
    PHASE_LABELS,
    positionLabel,
    progressRatio,
    type FileBurnUpdate,
    type FileSubtitleCue,
    type FileSubtitleJob
  } from '$lib/file-subtitle';
  import { timeLabel } from '$lib/subtitle-record';
  import { cueLines } from '$lib/media-player';
  import { errorText, type UiError } from '$lib/errors';
  import { notify, notifyError } from '$lib/toast.svelte';
  import BurnButton from '$lib/components/BurnButton.svelte';

  let {
    status,
    cues,
    media = null,
    notice = null,
    dragging = false,
    onpick,
    onstop
  }: {
    status: FileSubtitleJob;
    /** 生成済みの字幕の全件（開始時刻の昇順）。 */
    cues: FileSubtitleCue[];
    /** 再生対象の URL。取得不可の場合は null（再生画面を省略）。 */
    media?: string | null;
    /** ファイル選択側の通知（開始失敗等）。 */
    notice?: UiError | null;
    dragging?: boolean;
    onpick: () => void;
    onstop: () => void;
  } = $props();

  let content = $state<SubtitleExportContent>('translated');
  let list = $state<HTMLOListElement | null>(null);
  let player = $state<ReturnType<typeof MediaPlayer> | null>(null);
  /** 再生位置の字幕の番号（該当無しは -1）。 */
  let active = $state(-1);
  /** 再生中：一覧は末尾でなく再生位置の字幕へ追従。 */
  let playing = false;

  const running = $derived(status.phase === 'running');
  const ratio = $derived(progressRatio(status));
  // 推論中は演出を停止（翻訳性能を優先）
  $effect(() => (running ? holdMotion() : undefined));

  // 末尾を表示中の場合のみ、追加分へ追従（再生中は再生位置の字幕へ追従）
  let follow = true;
  $effect.pre(() => {
    void cues.length;
    follow = !list || list.scrollHeight - list.scrollTop - list.clientHeight < 48;
  });
  $effect(() => {
    void cues.length;
    if (list && follow && !playing) list.scrollTop = list.scrollHeight;
  });

  // 表示内容の変更：行数の増減後も再生位置の字幕を表示範囲内へ維持
  $effect(() => {
    void content;
    if (active >= 0) list?.children[active]?.scrollIntoView({ block: 'nearest' });
  });

  function onactive(index: number, isPlaying: boolean) {
    active = index;
    playing = isPlaying;
    if (index < 0 || !list) return;
    list.children[index]?.scrollIntoView({ block: 'nearest' });
  }

  const CONTENT_LABELS: [SubtitleExportContent, string][] = [
    ['translated', 'common:translated_text'],
    ['original', 'common:source'],
    ['both', 'subtitle-record-detail:both']
  ];

  // 字幕合成：保存先の選択後、本体側で逐次処理
  let burn = $state<FileBurnUpdate | null>(null);
  const burning = $derived(burn?.phase === 'running');
  $effect(() => {
    const off = onFileBurn((update) => {
      burn = update;
      if (update.phase === 'done') notify(t('file-subtitle:saved_file', { file: update.file }));
      if (update.phase === 'failed') notifyError(update.error ?? '');
    });
    return () => void off.then((f) => f());
  });
  const burnPercent = $derived(
    burn && status.duration_ms ? Math.min(100, Math.round((burn.done_ms / status.duration_ms) * 100)) : null
  );

  async function startBurn() {
    try {
      await burnFileSubtitle(content);
    } catch (e) {
      notifyError(t('file-burn:failed', { e: errorText(e) }));
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
</script>

<main class="px-body fit mx-auto flex max-w-6xl flex-col gap-4 px-4 py-6 sm:px-6" data-fit-viewport>
  <header class="flex flex-wrap items-end justify-between gap-3">
    <div class="min-w-0">
      <div class="page-title"><PixelTitle text={t('common:file_translation')} tag="h1" height={40} fit /></div>
      <p class="truncate text-sm text-ink-muted">{t('file-subtitle:description')}</p>
    </div>
    <div class="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-2">
      <button
        class="inline-flex items-center gap-1.5 rounded-ml-sm border border-accent bg-accent px-3 py-1.5 text-sm font-medium text-white shadow-ml transition hover:opacity-90 disabled:opacity-50"
        onclick={onpick}
      >
        <FolderOpen class="h-4 w-4" />{t('file:open_file')}
      </button>
    </div>
  </header>

  {#if notice}
    <p class="rounded-ml border border-red-500/50 bg-red-500/10 p-3 text-sm break-words text-red-500" role="alert">
      {notice.message}
      {#if notice.fix}
        <button class="ml-1 text-accent underline-offset-2 hover:underline" onclick={() => goto(notice!.fix!.route)}>
          {notice.fix.label}
        </button>
      {/if}
    </p>
  {/if}

  <section class="rounded-ml border bg-surface-elevated p-4 {dragging ? 'border-accent' : 'border-border'}" aria-label={t('file-subtitle:progress')}>
    <div class="flex flex-wrap items-center gap-x-4 gap-y-2">
      <p class="name truncate text-base font-medium" title={status.name}>{status.name}</p>
      <p class="text-sm tabular-nums text-ink-muted" role="status">
        {t(PHASE_LABELS[status.phase])}・{positionLabel(status)}・{t('file-subtitle:cues_n', { n: status.cues })}
      </p>
      <div class="ml-auto flex flex-wrap items-center gap-2">
        <select
          class="rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm text-ink"
          title={t('file-subtitle:subtitle_content')}
          aria-label={t('file-subtitle:subtitle_content')}
          bind:value={content}
        >
          {#each CONTENT_LABELS as [value, label] (value)}
            <option {value}>{t(label)}</option>
          {/each}
        </select>
        {#if running}
          <button class="sbtn" onclick={onstop}><Square class="h-3.5 w-3.5" />{t('common:cancel')}</button>
        {:else}
          <button class="sbtn" onclick={() => save('srt')} disabled={status.cues === 0}>
            <Captions class="h-3.5 w-3.5" />{t('file-subtitle:save_srt')}
          </button>
          <button class="sbtn" onclick={() => save('txt')} disabled={status.cues === 0}>
            <FileText class="h-3.5 w-3.5" />{t('file-subtitle:save_text')}
          </button>
          {#if burning}
            <button class="sbtn" onclick={() => cancelBurnFileSubtitle()}>
              <Square class="h-3.5 w-3.5" />{t('file-burn:cancel', { n: burnPercent ?? 0 })}
            </button>
          {:else}
            <BurnButton disabled={status.cues === 0} onstart={startBurn} />
          {/if}
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

  <div class="body" class:with-player={media}>
    {#if media}
      {#key media}
        <MediaPlayer bind:this={player} url={media} {cues} {status} {content} {onactive} />
      {/key}
    {/if}
    <section class="cues rounded-ml border border-border bg-surface-elevated" aria-label={t('file-subtitle:subtitles')}>
      <h2 class="border-b border-border px-4 py-2 text-sm text-ink-muted">{t('file-subtitle:subtitles')}</h2>
      {#if cues.length === 0}
        <p class="px-4 py-3 text-sm text-ink-faint">
          {running ? t('file-subtitle:waiting_for_first_subtitle') : t('file-subtitle:no_subtitles')}
        </p>
      {:else}
        <ol bind:this={list}>
          {#each cues as cue, i (cue.start_ms)}
            <li class:active={i === active}>
              <button
                class="cue"
                disabled={!media}
                title={media ? t('file-subtitle:jump_to_cue') : undefined}
                aria-current={i === active ? 'true' : undefined}
                onclick={() => player?.seek(cue.start_ms)}
              >
                <span class="time tabular-nums text-ink-faint">{timeLabel(cue.start_ms)}</span>
                <span class="min-w-0">
                  {#each cueLines(cue, content) as line, n (n)}
                    <span class="block break-words {n > 0 ? 'text-sm text-ink-muted' : ''}">{line}</span>
                  {/each}
                </span>
              </button>
            </li>
          {/each}
        </ol>
      {/if}
    </section>
  </div>
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
  /* 再生画面と字幕一覧：残余の高さを配分 */
  .body {
    display: grid;
    flex: 1 1 0;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--ml-sp-4);
    min-height: 0;
  }
  .body.with-player {
    grid-template-columns: minmax(0, 1fr) minmax(16rem, 34%);
  }
  .cues {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  ol {
    flex: 1 1 0;
    min-height: 0;
    margin: 0;
    padding: var(--ml-sp-2) var(--ml-sp-2);
    overflow-y: auto;
    list-style: none;
  }
  .cue {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--ml-sp-3);
    width: 100%;
    padding: var(--ml-sp-1) var(--ml-sp-2);
    text-align: left;
    border-left: 2px solid transparent;
  }
  .cue:not(:disabled):hover {
    background: var(--ml-surface-muted);
  }
  li.active .cue {
    border-left-color: var(--ml-accent);
    background: var(--ml-accent-soft);
  }
</style>
