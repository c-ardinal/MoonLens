<script lang="ts">
  import { t } from '$lib/i18n';
  import { holdMotion } from '$lib/pixel/motion.svelte';
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import { onDestroy, onMount, untrack, type Snippet } from 'svelte';
  import { goto } from '$app/navigation';
  import { Copy, Eye, EyeOff, FolderOpen, Loader2 } from 'lucide-svelte';
  import {
    getSettings,
    onImageAnalysis,
    onImageProgress,
    onOverlay,
    translateImage,
    type CaptureFlowResult,
    type LensBlock
  } from '$lib/ipc';
  import { LANGUAGES, applyStreaming, blockCssRect, blockPaint, growRoom, joinedText } from '$lib/lens-overlay';
  import FlowText from '$lib/components/FlowText.svelte';
  import { scanlineHole } from '$lib/pixel/scanline';
  import { parseError, type UiError } from '$lib/errors';
  import { extractText } from '$lib/result';
  import { copyText } from '$lib/toast.svelte';
  import { containSize } from '$lib/image-translate';

  /** 翻訳対象の画像。`id` の変化で翻訳を開始（同一画像の再指定を含む）。 */
  export type ImageInput = { id: number; bytes: Uint8Array; name: string };

  let {
    image,
    notice = null,
    dragging = false,
    onpick,
    extra
  }: {
    image: ImageInput;
    /** ファイル選択側の通知（読込失敗等）。 */
    notice?: UiError | null;
    dragging?: boolean;
    onpick: () => void;
    /** 見出し行の追加操作（字幕生成の状況への復帰等）。 */
    extra?: Snippet;
  } = $props();

  const TARGET_KEY = 'moonlens.image.target';

  let imgEl = $state<HTMLImageElement | null>(null);
  /** 画像枠の内寸（CSS px）と画像の原寸。表示寸法は枠へ収まる最大の等比。 */
  let room = $state({ w: 0, h: 0, zoom: 1 });
  let natural = $state({ w: 0, h: 0 });
  // 拡大の上限：表示上の原寸（本文画面の拡縮を相殺）
  const shown = $derived(containSize(natural, room, 1 / room.zoom));
  let imageUrl = $state<string | null>(null);
  let imageName = $state('');
  let imageBytes: Uint8Array | null = null;
  /** Image px per CSS px of the shown image. */
  let scale = $state(1);
  let blocks = $state<LensBlock[]>([]);
  let result = $state<CaptureFlowResult | null>(null);
  let busy = $state(false);
  // 推論中はメインウィンドウの演出を停止
  $effect(() => (busy ? holdMotion() : undefined));
  let phase = $state<'ocr' | 'translate'>('ocr');
  let error = $state<UiError | null>(null);
  const shownError = $derived(error ?? notice);
  let showOriginal = $state(false);
  let target = $state('ja');

  let run = 0;
  let streamBase: LensBlock[] = [];
  let streamInto: number[] | undefined;
  let streamed = '';
  const unlisteners: (() => void)[] = [];

  /** Text result without blocks (QR code, or OCR-only output). */
  const plainText = $derived(result && blocks.length === 0 ? extractText(result.result) : '');
  const crop = $derived(
    result?.image_size ? { x: 0, y: 0, w: result.image_size[0], h: result.image_size[1] } : null
  );

  function loadTarget(settingsTarget: string | undefined) {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(TARGET_KEY);
    } catch {
      /* storage unavailable */
    }
    target = saved || settingsTarget || 'ja';
  }

  function setTarget(code: string) {
    target = code;
    try {
      localStorage.setItem(TARGET_KEY, code);
    } catch {
      /* storage unavailable */
    }
    if (imageBytes) start();
  }

  /** 訳文言語の確定・進行通知の受信準備の完了（以前の開始は通知・設定値を欠落）。 */
  let ready = $state(false);
  // 画像の指定・変更で翻訳を開始
  $effect(() => {
    const { bytes, name } = image;
    if (!ready) return;
    untrack(() => {
      if (imageUrl) URL.revokeObjectURL(imageUrl);
      imageBytes = bytes;
      imageName = name;
      imageUrl = URL.createObjectURL(new Blob([bytes as BlobPart]));
      start();
    });
  });

  async function start() {
    if (!imageBytes) return;
    const id = ++run;
    busy = true;
    phase = 'ocr';
    error = null;
    result = null;
    blocks = [];
    streamBase = [];
    streamInto = undefined;
    streamed = '';
    try {
      const r = await translateImage(imageBytes, id, target);
      if (id !== run) return;
      result = r;
      blocks = r.blocks ?? [];
    } catch (e) {
      if (id === run) error = parseError(e);
    } finally {
      if (id === run) busy = false;
    }
  }

  function measure() {
    if (!imgEl) return;
    natural = { w: imgEl.naturalWidth, h: imgEl.naturalHeight };
  }
  $effect(() => {
    if (shown.w > 0) scale = natural.w / shown.w;
  });
  /** 画像枠の内寸を追跡（ウィンドウ寸法・文字サイズ・見出し行の折返しで変化）。 */
  function trackRoom(node: HTMLElement) {
    const ro = new ResizeObserver(
      () => (room = { w: node.clientWidth, h: node.clientHeight, zoom: (node as any).currentCSSZoom ?? 1 })
    );
    ro.observe(node);
    return { destroy: () => ro.disconnect() };
  }

  async function copyAll() {
    const text = blocks.length
      ? joinedText(blocks, showOriginal ? 'source' : 'translation')
      : plainText;
    if (text) await copyText(text, showOriginal && blocks.length ? t('common:source') : t('common:translated_text'));
  }

  onMount(async () => {
    loadTarget(undefined);
    await getSettings()
      .then((s: any) => loadTarget(s?.subtitle?.target_language))
      .catch(() => {});
    try {
      unlisteners.push(
        await onImageAnalysis((e) => {
          if (e.run !== run || !busy) return;
          phase = 'translate';
          result = e;
          blocks = e.blocks ?? [];
          streamBase = blocks;
          streamInto = undefined;
          streamed = '';
        })
      );
      unlisteners.push(
        await onImageProgress((e) => {
          if (e.run !== run || !busy) return;
          blocks = e.blocks;
          streamBase = blocks;
          streamInto = e.streaming;
          streamed = '';
        })
      );
      unlisteners.push(
        await onOverlay((ev) => {
          if (ev.kind !== 'chunk' || !busy || phase !== 'translate' || streamBase.length === 0) return;
          streamed += ev.delta;
          blocks = applyStreaming(streamBase, streamed, streamInto);
        })
      );
    } catch {
      /* pure frontend dev */
    }
    ready = true;
  });

  onDestroy(() => {
    run++;
    unlisteners.forEach((u) => u());
    if (imageUrl) URL.revokeObjectURL(imageUrl);
  });
</script>

<!-- 画像表示中は 1 画面へ収容（画像枠・認識結果の高さを残余の領域で確定、画面の縦スクロールなし） -->
<main class="px-body fit mx-auto flex max-w-6xl flex-col gap-4 px-4 py-6 sm:px-6" data-fit-viewport>
  <header class="flex flex-wrap items-end justify-between gap-3">
    <div class="min-w-0">
      <div class="page-title"><PixelTitle text={t('common:file_translation')} tag="h1" height={40} fit /></div>
    </div>
    <!-- 操作部：折返し時も右寄せ（他画面の見出し行と統一）。画像表示中は左端へファイル名（残余の幅、超過分は省略） -->
    <div class="grow ml-auto flex min-w-0 flex-wrap items-center justify-end gap-2">
      <p class="name truncate text-sm text-ink-muted" title={imageName}>{imageName}</p>
      {@render extra?.()}
      <label class="flex items-center gap-1.5 text-sm text-ink-muted">
        {t('common:translated_text')}
        <select
          class="rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm text-ink"
          value={target}
          onchange={(e) => setTarget((e.currentTarget as HTMLSelectElement).value)}
          aria-label={t('image:translation_language')}
        >
          {#each LANGUAGES as l (l.code)}
            <option value={l.code}>{t(l.label)}</option>
          {/each}
        </select>
      </label>
      <button
        class="inline-flex items-center gap-1.5 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm transition hover:border-accent/60 disabled:opacity-50"
        onclick={() => (showOriginal = !showOriginal)}
        disabled={blocks.length === 0}
        aria-pressed={showOriginal}
      >
        {#if showOriginal}<Eye class="h-4 w-4" />{t('common:show_translation')}{:else}<EyeOff class="h-4 w-4" />{t('common:show_source')}{/if}
      </button>
      <button
        class="sbtn icon neutral"
        title={t('image:copy_all_text')}
        aria-label={t('image:copy_all_text')}
        onclick={copyAll}
        disabled={busy || (blocks.length === 0 && !plainText)}
      >
        <Copy class="h-4 w-4" />
      </button>
      <button
        class="inline-flex items-center gap-1.5 rounded-ml-sm border border-accent bg-accent px-3 py-1.5 text-sm font-medium text-white shadow-ml transition hover:opacity-90"
        onclick={onpick}
      >
        <FolderOpen class="h-4 w-4" />{t('file:open_file')}
      </button>
    </div>
  </header>

  {#if shownError}
    <p class="rounded-ml border border-red-500/50 bg-red-500/10 p-3 text-sm break-words text-red-500" role="alert">
      {shownError.message}
      {#if shownError.fix}
        <button class="ml-1 text-accent underline-offset-2 hover:underline" onclick={() => goto(shownError!.fix!.route)}>
          {shownError.fix.label}
        </button>
      {/if}
    </p>
  {/if}

  <div class="work">
    <section
      class="relative overflow-hidden rounded-ml border bg-surface-muted p-2 {dragging ? 'border-accent' : 'border-border'}"
      aria-label={t('common:image')}
    >
      <div class="room" use:trackRoom>
        <!-- 画像と訳文の重畳：原文準拠の表示（ドット絵外観の走査線・書体・拡大補間の対象外） -->
        <div class="shot" style="width: {shown.w}px; height: {shown.h}px" use:scanlineHole data-grid-exempt data-overflow-exempt>
          <img
            bind:this={imgEl}
            src={imageUrl}
            alt={imageName}
            class="block h-full w-full select-none"
            onload={measure}
            draggable="false"
          />
          {#if crop && !showOriginal && shown.w > 0}
            {#each blocks as b, i (i)}
              {#if b.bbox[2] > 0 && !b.same_language}
                {@const r = blockCssRect(b, crop, scale)}
                {@const paint = blockPaint(b, scale)}
                <div
                  class="block-box absolute"
                  class:pending={b.translation == null}
                  class:erased={!!paint.erase}
                  style="left: {r.left}px; top: {r.top}px; width: {r.width}px; height: {r.height}px;
                         background: {paint.background};
                         color: {paint.color};
                         clip-path: {paint.clip};"
                  title={b.source}
                >
                  {#if paint.erase}
                    <img
                      class="erase"
                      src={paint.erase.src}
                      alt=""
                      draggable="false"
                      style="left: {paint.erase.rect.left}px; top: {paint.erase.rect.top}px;
                             width: {paint.erase.rect.width}px; height: {paint.erase.rect.height}px"
                    />
                  {/if}
                  {#if b.translation != null}
                    <FlowText
                      block={b}
                      text={b.translation}
                      {scale}
                      grow={growRoom(blocks, i, crop.w) / scale}
                      background={paint.background}
                      outline={paint.outline}
                    />
                  {/if}
                </div>
              {/if}
            {/each}
          {/if}
        </div>
      </div>
      {#if busy}
        <div
          class="absolute top-3 left-1/2 inline-flex -translate-x-1/2 items-center gap-2 rounded-full bg-black/75 px-4 py-1.5 text-xs text-white"
          aria-live="polite"
        >
          <Loader2 class="h-3.5 w-3.5 animate-spin" />
          {phase === 'ocr' ? t('common:reading_text') : t('common:translating')}
        </div>
      {/if}
    </section>

    <aside class="flex min-h-0 flex-col rounded-ml border border-border bg-surface-elevated shadow-ml">
      <div class="flex items-center justify-between gap-4 border-b border-border px-4 py-2.5 text-sm whitespace-nowrap">
        <span class="font-medium">{t('image:recognized_text')}</span>
        <span class="text-xs text-ink-faint">{t('image:length_blocks', { length: blocks.length })}</span>
      </div>
      <ol class="min-h-0 flex-1 divide-y divide-border overflow-y-auto">
        {#each blocks as b, i (i)}
          <li class="px-4 py-2.5 text-sm">
            <p class="text-xs break-words whitespace-pre-wrap text-ink-faint">{b.source}</p>
            {#if b.same_language}
              <p class="mt-1 text-xs text-ink-faint">{t('image:not_translated')}</p>
            {:else if b.translation != null}
              <p class="mt-1 break-words whitespace-pre-wrap select-text">{b.translation}</p>
            {:else}
              <p class="mt-1 text-xs text-ink-faint">{t('image:waiting_to_translate')}</p>
            {/if}
          </li>
        {:else}
          <li class="px-4 py-6 text-center text-sm text-ink-faint">
            {#if busy}{t('common:reading_text')}{:else if plainText}<span class="break-words whitespace-pre-wrap text-ink"
                >{plainText}</span
              >{:else if result}{t('common:no_text')}{/if}
          </li>
        {/each}
      </ol>
      {#if result && !busy}
        <p class="border-t border-border px-4 py-2 text-xs text-ink-faint">
          {result.result.used_provider}・{result.result.latency_ms} ms
        </p>
      {/if}
    </aside>
  </div>
</main>

<style>
  /* 1 画面収容：見出し行の残余を作業領域（画像枠・認識結果）へ配分 */
  main.fit {
    overflow-y: hidden;
  }
  .work {
    display: grid;
    flex: 1 1 0;
    grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
    grid-template-rows: minmax(0, 1fr);
    gap: var(--ml-sp-4);
    min-height: 0;
  }
  header .grow {
    flex: 1 1 0;
  }
  /* 下限 6 字分：未満の幅では操作部を折返し（判読不可の断片表示を回避） */
  .name {
    flex: 1 0 192px;
    min-width: 0;
    margin: 0;
  }
  .room {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }
  .shot {
    position: relative;
    flex-shrink: 0;
    font-family: var(--ml-font-plain);
    font-synthesis: weight style;
    -webkit-font-smoothing: auto;
    image-rendering: auto;
  }
  .block-box {
    border-radius: 3px;
    user-select: text;
    cursor: text;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.04);
  }
  /* 写真・グラデーション上：文字画素のみの補完画像（枠の影なし） */
  .block-box.erased {
    box-shadow: none;
  }
  .block-box .erase {
    position: absolute;
    max-width: none;
    pointer-events: none;
    user-select: none;
  }
  .block-box.pending {
    background-image: linear-gradient(100deg, transparent 30%, color-mix(in srgb, var(--ml-accent) 18%, transparent) 50%, transparent 70%) !important;
    background-size: 250% 100%;
    animation: scan 1.1s linear infinite;
  }
  @keyframes scan {
    from {
      background-position: 120% 0;
    }
    to {
      background-position: -120% 0;
    }
  }
</style>
