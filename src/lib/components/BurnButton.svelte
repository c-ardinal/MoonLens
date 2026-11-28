<script lang="ts">
  import { t } from '$lib/i18n';
  import { Film } from 'lucide-svelte';
  import { ffmpegStatus, setFfmpegPath, type FfmpegStatus } from '$lib/ipc';
  import { ffmpegProblem } from '$lib/ffmpeg';
  import { errorText } from '$lib/errors';
  import { notifyError } from '$lib/toast.svelte';
  import FfmpegGuide from './FfmpegGuide.svelte';

  /**
   * 字幕合成ボタン。ffmpeg 未検出・利用不可時はホバー（フォーカス）で案内を表示し、
   * 押下時はファイル選択へ誘導。
   */
  interface Props {
    disabled?: boolean;
    /** 合成の開始（ffmpeg 利用可能時）。 */
    onstart: () => void;
  }
  let { disabled = false, onstart }: Props = $props();

  let status = $state<FfmpegStatus | null>(null);
  let open = $state(false);
  const problem = $derived(status ? ffmpegProblem(status) : null);
  let anchor = $state<HTMLElement>();
  let pop = $state<HTMLElement>();
  /** 案内の表示位置（ウィンドウ基準）。ボタン下側の余白不足時は上側へ配置。 */
  let place = $state('');

  $effect(() => {
    if (!open) place = '';
    if (!open || !anchor || !pop) return;
    const a = anchor.getBoundingClientRect();
    const { width, height } = pop.getBoundingClientRect();
    const gap = 8;
    const left = Math.max(gap, Math.min(a.right - width, innerWidth - width - gap));
    const below = a.bottom + height + gap <= innerHeight;
    place = `left:${left}px;` + (below ? `top:${a.bottom}px;` : `bottom:${innerHeight - a.top}px;`);
  });

  async function refresh() {
    status = await ffmpegStatus().catch(() => null);
  }
  $effect(() => void refresh());

  async function click() {
    await refresh();
    if (problem) open = true;
    else onstart();
  }

  async function usePicked(path: string) {
    try {
      await setFfmpegPath(path);
    } catch (e) {
      notifyError(t('file-burn:failed', { e: errorText(e) }));
      return;
    }
    open = false;
    await refresh();
    onstart();
  }
</script>

<div
  class="burn"
  role="group"
  bind:this={anchor}
  onpointerenter={() => (open = !!problem)}
  onpointerleave={() => (open = false)}
  onfocusin={() => (open = !!problem)}
  onfocusout={(e) => {
    if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) open = false;
  }}
>
  <button class="sbtn" {disabled} title={problem ? undefined : t('file-burn:hint')} onclick={click}>
    <Film class="h-3.5 w-3.5" />{t('file-burn:start')}
  </button>
  {#if open && status && problem}
    <div class="pop" style={place} bind:this={pop}>
      <div class="rounded-ml border border-accent bg-surface-elevated p-3 shadow-ml">
        <FfmpegGuide {status} onpicked={usePicked} />
      </div>
    </div>
  {/if}
</div>

<style>
  /* 案内は字幕一覧等を覆う浮動表示（配置・スクロール量へ影響なし）。
     ボタンとの間隙は透明な余白で補完し、ホバー状態を維持 */
  .pop {
    position: fixed;
    z-index: 20;
    width: min(28em, calc(100vw - 16px));
    padding: var(--ml-sp-1) 0;
  }
  .pop:not([style*='left']) {
    visibility: hidden;
  }
</style>
