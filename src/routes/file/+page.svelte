<script lang="ts">
  import { t } from '$lib/i18n';
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import ImageTranslate, { type ImageInput } from '$lib/components/ImageTranslate.svelte';
  import FileSubtitle from '$lib/components/FileSubtitle.svelte';
  import { onDestroy, onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Captions, Clapperboard, FolderOpen, ImageIcon } from 'lucide-svelte';
  import {
    cancelFileSubtitle,
    fileSubtitleStatus,
    onFileSubtitle,
    pickInputFile,
    readImageFile,
    startFileSubtitle
  } from '$lib/ipc';
  import { IDLE, PHASE_LABELS, isJob, withCue, type FileSubtitleCue, type FileSubtitleStatus } from '$lib/file-subtitle';
  import { MAX_IMAGE_BYTES, baseName, firstImageFile } from '$lib/image-translate';
  import { fileKind, initialView, type FileView } from '$lib/file-open';
  import { parseError, type UiError } from '$lib/errors';

  /** 表示中の画面。ファイルの種別で確定（画像：画像翻訳、動画・音声：字幕生成）。 */
  let view = $state<FileView>('idle');
  let image = $state<ImageInput | null>(null);
  /** 字幕生成のジョブ（画像の表示中も本体側で継続）。 */
  let status = $state<FileSubtitleStatus>(IDLE);
  const job = $derived(isJob(status) ? status : null);
  let recent = $state<FileSubtitleCue[]>([]);
  let error = $state<UiError | null>(null);
  let dragging = $state(false);
  let opened = 0;
  const unlisteners: (() => void)[] = [];

  function showImage(bytes: Uint8Array, name: string) {
    if (bytes.byteLength > MAX_IMAGE_BYTES) {
      error = { code: 'error', message: t('common:image_too_large_limit_40_mb'), fix: null };
      return;
    }
    image = { id: ++opened, bytes, name };
    view = 'image';
  }

  /** ファイルを開始：種別で処理を振分け。失敗時は表示中の画面を維持し、理由を通知。 */
  async function open(path: string | null | undefined) {
    if (!path) return;
    error = null;
    try {
      if (fileKind(path) === 'image') {
        showImage(new Uint8Array(await readImageFile(path)), baseName(path));
      } else {
        status = await startFileSubtitle(path);
        recent = [];
        view = 'media';
      }
    } catch (e) {
      error = parseError(e);
    }
  }

  async function pick() {
    try {
      await open(await pickInputFile());
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

  async function onPaste(e: ClipboardEvent) {
    if (e.target instanceof Element && e.target.closest('input, textarea, select, [contenteditable]')) return;
    const file = firstImageFile(e.clipboardData?.files);
    if (!file) return;
    e.preventDefault();
    error = null;
    showImage(new Uint8Array(await file.arrayBuffer()), file.name || t('image:pasted_image'));
  }

  onMount(async () => {
    window.addEventListener('paste', onPaste);
    unlisteners.push(() => window.removeEventListener('paste', onPaste));
    try {
      status = (await fileSubtitleStatus()) ?? IDLE;
      if (view === 'idle') view = initialView(status);
      unlisteners.push(
        await onFileSubtitle((update) => {
          status = update.status;
          // ファイル未選択の表示中にジョブを検知：進行状況へ移行
          if (view === 'idle') view = initialView(status);
          if (update.cue) recent = withCue(recent, update.cue);
        })
      );
      const { getCurrentWebview } = await import('@tauri-apps/api/webview');
      unlisteners.push(
        await getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === 'enter' || p.type === 'over') dragging = true;
          else if (p.type === 'leave') dragging = false;
          else if (p.type === 'drop') {
            dragging = false;
            open(p.paths[0]);
          }
        })
      );
    } catch {
      /* pure frontend dev */
    }
  });

  onDestroy(() => unlisteners.forEach((u) => u()));
</script>

{#if view === 'image' && image}
  <ImageTranslate {image} notice={error} {dragging} onpick={pick}>
    {#snippet extra()}
      {#if job}
        <!-- 字幕生成のジョブへの復帰（画像の表示中も本体側で継続） -->
        <button class="sbtn" data-action="show-subtitle-job" onclick={() => ((error = null), (view = 'media'))}>
          <Captions class="h-3.5 w-3.5" />{t('file:subtitle_job_state', { state: t(PHASE_LABELS[job.phase]) })}
        </button>
      {/if}
    {/snippet}
  </ImageTranslate>
{:else if view === 'media' && job}
  <FileSubtitle status={job} {recent} notice={error} {dragging} onpick={pick} onstop={stop} />
{:else}
  <main class="px-body mx-auto flex max-w-6xl flex-col gap-4 px-4 py-6 sm:px-6">
    <header class="flex flex-wrap items-end justify-between gap-3">
      <div class="min-w-0">
        <div class="page-title"><PixelTitle text={t('common:file_translation')} tag="h1" height={40} fit /></div>
        <p class="truncate text-sm text-ink-muted">{t('file:description')}</p>
      </div>
      <div class="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-2">
        <button
          class="inline-flex items-center gap-1.5 rounded-ml-sm border border-accent bg-accent px-3 py-1.5 text-sm font-medium text-white shadow-ml transition hover:opacity-90"
          onclick={pick}
        >
          <FolderOpen class="h-4 w-4" />{t('file:open_file')}
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

    <button
      class="flex min-h-72 flex-col items-center justify-center gap-4 rounded-ml border-2 border-dashed p-8 text-center transition {dragging
        ? 'border-accent bg-accent-soft'
        : 'border-ink-faint/50 bg-surface-elevated hover:border-accent/60'}"
      onclick={pick}
    >
      <span class="text-base font-medium">{t('file:drop_a_file')}</span>
      <!-- 種別と処理の対応（振分けは自動） -->
      <span class="kinds text-left text-sm">
        <ImageIcon class="h-6 w-6 text-ink-faint" />
        <span>
          <span class="block font-medium">{t('file:kind_image')}</span>
          <span class="block text-ink-muted">{t('file:kind_image_hint')}</span>
        </span>
        <Clapperboard class="h-6 w-6 text-ink-faint" />
        <span>
          <span class="block font-medium">{t('file:kind_media')}</span>
          <span class="block text-ink-muted">{t('file:kind_media_hint')}</span>
        </span>
      </span>
      <span class="text-sm text-ink-muted">{t('file:click_to_choose')}</span>
    </button>
  </main>
{/if}

<style>
  .kinds {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    align-items: center;
    gap: var(--ml-sp-3) var(--ml-sp-3);
    max-width: 100%;
  }
</style>
