<script lang="ts">
  import { t } from '$lib/i18n';
  import { onMount } from 'svelte';
  import { Hourglass, TriangleAlert } from 'lucide-svelte';
  import type { FileSubtitleCue, FileSubtitleJob } from '$lib/file-subtitle';
  import { cueIndexAt, cueLines, isAhead, mayResume, mustWait, type CueContent } from '$lib/media-player';

  let {
    url,
    cues,
    status,
    content,
    onactive
  }: {
    /** 再生対象（asset プロトコルの URL）。 */
    url: string;
    /** 生成済みの字幕の全件（開始時刻の昇順）。 */
    cues: FileSubtitleCue[];
    status: FileSubtitleJob;
    content: CueContent;
    /** 再生位置の字幕の番号の変化（該当無しは -1）。 */
    onactive?: (index: number, playing: boolean) => void;
  } = $props();

  let video = $state<HTMLVideoElement | null>(null);
  let track: TextTrack | null = null;
  /** 字幕トラックへ追加済みの件数・内容（差分のみ追加、内容の変更時は全件を再登録）。 */
  let added = 0;
  let addedContent: CueContent | null = null;
  let addedCues: FileSubtitleCue[] | null = null;
  /** 生成位置への到達による自動の一時停止。 */
  let waiting = $state(false);
  /** 待機中の利用者の再生操作：同一の再生区間では待機を省略（シークで解除）。 */
  let manual = false;
  let ahead = $state(false);
  let failed = $state(false);
  let lastActive = -2;
  /** 字幕の寸法：再生画面の高さへ連動（7%、20〜48px）。`::cue` は変数・継承の対象外のため規則を直接生成。 */
  let stageHeight = $state(0);
  const cueSize = $derived(Math.round(Math.min(48, Math.max(20, stageHeight * 0.07))));
  $effect(() => {
    const rule = document.createElement('style');
    rule.textContent = `video[data-ml-player]::cue { font-size: ${cueSize}px !important; }`;
    document.head.append(rule);
    return () => rule.remove();
  });

  const positionMs = () => (video ? video.currentTime * 1000 : 0);

  onMount(() => {
    if (!video) return;
    track = video.addTextTrack('subtitles', 'MoonLens');
    track.mode = 'showing';
  });

  /** 字幕トラックの同期：追加分のみ登録、内容・字幕の差替え時は全件を再登録。 */
  $effect(() => {
    const list = cues;
    const mode = content;
    if (!track) return;
    const replaced = mode !== addedContent || list.length < added || (added > 0 && addedCues?.[0] !== list[0]);
    if (replaced) {
      for (const cue of Array.from(track.cues ?? [])) track.removeCue(cue);
      added = 0;
    }
    for (; added < list.length; added++) {
      const cue = list[added];
      track.addCue(new VTTCue(cue.start_ms / 1000, cue.end_ms / 1000, cueLines(cue, mode).join('\n')));
    }
    addedContent = mode;
    addedCues = list;
    update();
  });

  /** 自動の一時停止の解除：字幕の確定が十分に先行した時点で再開（利用者の一時停止は対象外）。 */
  function resumeIfReady() {
    if (waiting && video && mayResume(status, positionMs())) {
      waiting = false;
      video.play().catch(() => {});
    }
  }

  /** 生成の進行：待機の解除を判定。 */
  $effect(() => {
    void status.ready_ms;
    void status.phase;
    resumeIfReady();
    update();
  });

  function update() {
    if (!video) return;
    const pos = positionMs();
    ahead = isAhead(status, pos);
    const index = cueIndexAt(cues, pos);
    if (index !== lastActive) {
      lastActive = index;
      onactive?.(index, !video.paused);
    }
    if (!video.paused && !manual && mustWait(status, pos)) {
      waiting = true;
      video.pause();
    }
  }

  function onplay() {
    // 待機中の再生操作：利用者の意図を優先し、字幕の生成前でも再生
    if (waiting) {
      waiting = false;
      manual = true;
    }
    update();
  }

  function onseeked() {
    manual = false;
    // 待機中の巻戻し等：移動先の字幕が確定済みなら再開、未確定なら待機を継続
    resumeIfReady();
    update();
  }

  /** 指定の位置へ移動（字幕一覧からの選択）。 */
  export function seek(ms: number) {
    if (!video) return;
    video.currentTime = ms / 1000;
  }
</script>

<div class="stage bg-black" data-overflow-exempt bind:clientHeight={stageHeight}>
  {#if failed}
    <p class="failed text-sm text-white/80" role="alert">
      <TriangleAlert class="h-5 w-5 shrink-0" />{t('file-subtitle:cannot_play')}
    </p>
  {/if}
  <!-- svelte-ignore a11y_media_has_caption -->
  <video
    bind:this={video}
    data-ml-player
    src={url}
    controls
    controlslist="nodownload noremoteplayback"
    disablepictureinpicture
    preload="metadata"
    class:hidden={failed}
    aria-label={t('file-subtitle:player')}
    ontimeupdate={update}
    {onplay}
    onpause={update}
    {onseeked}
    onerror={() => (failed = true)}
  ></video>
  {#if waiting || ahead}
    <p class="badge text-sm" role="status">
      <Hourglass class="h-3.5 w-3.5" />{waiting ? t('file-subtitle:waiting_for_subtitles') : t('file-subtitle:subtitles_not_ready')}
    </p>
  {/if}
</div>

<style>
  .stage {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  video {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  /* 字幕：背景付きで映像の明暗に非依存 */
  video::cue {
    line-height: 1.35;
    color: #fff;
    background: rgb(0 0 0 / 0.72);
  }
  .badge {
    position: absolute;
    top: var(--ml-sp-2);
    left: var(--ml-sp-2);
    display: inline-flex;
    align-items: center;
    gap: var(--ml-sp-1);
    margin: 0;
    padding: var(--ml-sp-1) var(--ml-sp-2);
    color: #fff;
    background: rgb(0 0 0 / 0.72);
    border: 1px solid var(--ml-accent);
  }
  .failed {
    display: flex;
    align-items: center;
    gap: var(--ml-sp-2);
    margin: 0;
    padding: var(--ml-sp-4);
  }
</style>
