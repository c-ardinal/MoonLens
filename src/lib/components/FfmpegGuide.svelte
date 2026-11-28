<script lang="ts">
  import { t } from '$lib/i18n';
  import { FolderOpen } from 'lucide-svelte';
  import { pickFfmpeg, type FfmpegStatus } from '$lib/ipc';
  import { ffmpegProblem } from '$lib/ffmpeg';

  /** ffmpeg 未検出・利用不可時の案内（PATH への登録・ファイル選択）。 */
  interface Props {
    status: FfmpegStatus;
    /** 利用可能な ffmpeg の選択時。 */
    onpicked: (path: string) => void;
  }
  let { status, onpicked }: Props = $props();

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

<div class="text-sm" aria-label={t('file-burn:guide_title')}>
  <p class="font-medium text-ink">{t(problem ?? 'file-burn:no_ffmpeg')}</p>
  {#if shown.path && problem}
    <p class="truncate text-xs text-ink-faint" title={shown.path}>{shown.path}</p>
  {/if}
  <p class="mt-2 text-ink-muted">{t('file-burn:guide_place')}</p>
  <button class="sbtn mt-3" onclick={pick}>
    <FolderOpen class="h-3.5 w-3.5" />{t('file-burn:pick_ffmpeg')}
  </button>
</div>
