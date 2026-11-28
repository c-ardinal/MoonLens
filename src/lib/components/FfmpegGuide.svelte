<script lang="ts">
  import { t } from '$lib/i18n';
  import { ExternalLink, FolderOpen, X } from 'lucide-svelte';
  import { openExternalUrl, pickFfmpeg, type FfmpegStatus } from '$lib/ipc';
  import { FFMPEG_DOWNLOAD_URL, ffmpegProblem } from '$lib/ffmpeg';

  /** ffmpeg 未検出・利用不可時の案内（入手先・配置場所・ファイル選択）。 */
  interface Props {
    status: FfmpegStatus;
    /** 利用可能な ffmpeg の選択時。 */
    onpicked: (path: string) => void;
    onclose: () => void;
  }
  let { status, onpicked, onclose }: Props = $props();

  let current = $state<FfmpegStatus | null>(null);
  const shown = $derived(current ?? status);
  const problem = $derived(ffmpegProblem(shown));

  async function pick() {
    const picked = await pickFfmpeg().catch(() => null);
    if (!picked) return;
    if (ffmpegProblem(picked)) current = picked;
    else onpicked(picked.path ?? '');
  }
</script>

<section class="guide rounded-ml border border-accent bg-surface-elevated p-4 text-sm" role="alert" aria-label={t('file-burn:guide_title')}>
  <div class="flex items-start gap-3">
    <div class="min-w-0 flex-1">
      <p class="font-medium text-ink">{t(problem ?? 'file-burn:no_ffmpeg')}</p>
      {#if shown.path && problem}
        <p class="truncate text-xs text-ink-faint" title={shown.path}>{shown.path}</p>
      {/if}
      <ol class="steps mt-2 text-ink-muted">
        {#each ['file-burn:guide_get', 'file-burn:guide_place'] as step, i (step)}
          <li><span class="tabular-nums">{i + 1}.</span><span>{t(step)}</span></li>
        {/each}
      </ol>
    </div>
    <button class="rounded-ml-sm p-1 text-ink-faint hover:bg-surface-muted hover:text-ink" title={t('common:close')} aria-label={t('common:close')} onclick={onclose}>
      <X class="h-4 w-4" />
    </button>
  </div>
  <div class="mt-3 flex flex-wrap gap-2">
    <button class="sbtn" onclick={() => openExternalUrl(FFMPEG_DOWNLOAD_URL).catch(() => {})}>
      <ExternalLink class="h-3.5 w-3.5" />{t('file-burn:open_download_page')}
    </button>
    <button class="sbtn" onclick={pick}>
      <FolderOpen class="h-3.5 w-3.5" />{t('file-burn:pick_ffmpeg')}
    </button>
  </div>
</section>

<style>
  .steps {
    margin-bottom: 0;
    padding: 0;
    list-style: none;
  }
  .steps li {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--ml-sp-2);
  }
</style>
