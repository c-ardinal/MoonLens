<script lang="ts">
  import { t } from '$lib/i18n';
  import { FolderOpen } from 'lucide-svelte';
  import { ffmpegStatus, pickFfmpeg, type FfmpegStatus } from '$lib/ipc';
  import { notifyError } from '$lib/toast.svelte';
  import { ffmpegProblem } from '$lib/ffmpeg';

  /** 字幕合成用 ffmpeg の場所（空文字：自動検出）。選択時に実行可否・libass 対応を判定。 */
  interface Props {
    id: string;
    value: string;
    onChange: (v: string) => void;
  }
  let { id, value, onChange }: Props = $props();

  let status = $state<FfmpegStatus | null>(null);
  $effect(() => {
    const configured = value;
    ffmpegStatus(configured)
      .then((s) => (status = s))
      .catch(() => (status = null));
  });

  const label = $derived(
    value || (status?.path ? t('file-burn:auto_found') : t('file-burn:auto_missing'))
  );

  async function pick() {
    const picked = await pickFfmpeg().catch(() => null);
    if (!picked) return;
    const problem = ffmpegProblem(picked);
    if (problem) notifyError(t(problem));
    else onChange(picked.path ?? '');
  }
</script>

<span class="path min-w-0 flex-1 truncate text-right text-sm text-ink-muted" title={status?.path ?? ''}>{label}</span>
<button {id} type="button" class="sbtn shrink-0" style="flex: 0 0 auto" onclick={pick}>
  <FolderOpen class="h-3.5 w-3.5" />{t('file-burn:choose')}
</button>
