<script lang="ts">
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import { onDestroy, onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Check, Copy, Eye, EyeOff, FolderOpen, ImageIcon, Loader2 } from 'lucide-svelte';
  import {
    getSettings,
    onImageAnalysis,
    onImageProgress,
    onOverlay,
    readImageFile,
    translateImage,
    type CaptureFlowResult,
    type LensBlock
  } from '$lib/ipc';
  import { LANGUAGES, applyStreaming, blockCssRect, growRoom, joinedText, rgb } from '$lib/lens-overlay';
  import FlowText from '$lib/components/FlowText.svelte';
  import { parseError, type UiError } from '$lib/errors';
  import { extractText } from '$lib/result';
  import {
    IMAGE_ACCEPT,
    MAX_IMAGE_BYTES,
    baseName,
    firstImageFile,
    firstImagePath
  } from '$lib/image-translate';

  const TARGET_KEY = 'moonlens.image.target';

  let fileInput = $state<HTMLInputElement | null>(null);
  let imgEl = $state<HTMLImageElement | null>(null);
  let imageUrl = $state<string | null>(null);
  let imageName = $state('');
  let imageBytes: Uint8Array | null = null;
  /** Image px per CSS px of the shown image. */
  let scale = $state(1);
  let blocks = $state<LensBlock[]>([]);
  let result = $state<CaptureFlowResult | null>(null);
  let busy = $state(false);
  let phase = $state<'ocr' | 'translate'>('ocr');
  let error = $state<UiError | null>(null);
  let showOriginal = $state(false);
  let dragging = $state(false);
  let copied = $state(false);
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

  async function open(bytes: Uint8Array, name: string) {
    if (bytes.byteLength > MAX_IMAGE_BYTES) {
      error = { code: 'error', message: '画像サイズ超過（上限 40 MB）', fix: null };
      return;
    }
    if (imageUrl) URL.revokeObjectURL(imageUrl);
    imageBytes = bytes;
    imageName = name;
    imageUrl = URL.createObjectURL(new Blob([bytes as BlobPart]));
    await start();
  }

  async function openFile(file: File | null) {
    if (!file) {
      error = { code: 'error', message: '非対応形式（PNG・JPEG・WebP のみ対応）', fix: null };
      return;
    }
    await open(new Uint8Array(await file.arrayBuffer()), file.name || '貼り付け画像');
  }

  async function openPath(path: string | null) {
    if (!path) {
      error = { code: 'error', message: '非対応形式（PNG・JPEG・WebP のみ対応）', fix: null };
      return;
    }
    try {
      await open(new Uint8Array(await readImageFile(path)), baseName(path));
    } catch (e) {
      error = parseError(e);
    }
  }

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
    if (imgEl && imgEl.clientWidth > 0) scale = imgEl.naturalWidth / imgEl.clientWidth;
  }

  async function copyAll() {
    const text = blocks.length
      ? joinedText(blocks, showOriginal ? 'source' : 'translation')
      : plainText;
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch (e) {
      error = parseError(e);
    }
  }

  function onPaste(e: ClipboardEvent) {
    const target = e.target as HTMLElement | null;
    if (target?.closest('input, textarea, select, [contenteditable]')) return;
    const file = firstImageFile(e.clipboardData?.files);
    if (file) {
      e.preventDefault();
      openFile(file);
    }
  }

  onMount(async () => {
    loadTarget(undefined);
    getSettings()
      .then((s: any) => loadTarget(s?.subtitle?.target_language))
      .catch(() => {});
    window.addEventListener('paste', onPaste);
    window.addEventListener('resize', measure);
    unlisteners.push(() => window.removeEventListener('paste', onPaste));
    unlisteners.push(() => window.removeEventListener('resize', measure));
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
      const { getCurrentWebview } = await import('@tauri-apps/api/webview');
      unlisteners.push(
        await getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === 'enter' || p.type === 'over') dragging = true;
          else if (p.type === 'leave') dragging = false;
          else if (p.type === 'drop') {
            dragging = false;
            openPath(firstImagePath(p.paths));
          }
        })
      );
    } catch {
      /* pure frontend dev */
    }
  });

  onDestroy(() => {
    run++;
    unlisteners.forEach((u) => u());
    if (imageUrl) URL.revokeObjectURL(imageUrl);
  });

  $effect(() => {
    if (!imgEl) return;
    const ro = new ResizeObserver(measure);
    ro.observe(imgEl);
    return () => ro.disconnect();
  });
</script>

<main class="mx-auto flex max-w-6xl flex-col gap-4 px-4 py-6 sm:px-6">
  <header class="flex flex-wrap items-end justify-between gap-3">
    <div class="min-w-0">
      <div class="page-title"><PixelTitle text="画像翻訳" tag="h1" height={40} fit /></div>
      <p class="truncate text-sm text-ink-muted">
        {imageName || '画像ファイルの文字を認識・翻訳。全処理ローカル実行。'}
      </p>
    </div>
    <div class="flex flex-wrap items-center gap-2">
      <label class="flex items-center gap-1.5 text-sm text-ink-muted">
        訳文
        <select
          class="rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm text-ink"
          value={target}
          onchange={(e) => setTarget((e.currentTarget as HTMLSelectElement).value)}
          aria-label="訳文の言語"
        >
          {#each LANGUAGES as l (l.code)}
            <option value={l.code}>{l.label}</option>
          {/each}
        </select>
      </label>
      {#if imageUrl}
        <button
          class="inline-flex items-center gap-1.5 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm transition hover:border-accent/60 disabled:opacity-50"
          onclick={() => (showOriginal = !showOriginal)}
          disabled={blocks.length === 0}
          aria-pressed={showOriginal}
        >
          {#if showOriginal}<Eye class="h-4 w-4" />訳文表示{:else}<EyeOff class="h-4 w-4" />原文表示{/if}
        </button>
        <button
          class="inline-flex items-center gap-1.5 rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm transition hover:border-accent/60 disabled:opacity-50"
          onclick={copyAll}
          disabled={busy || (blocks.length === 0 && !plainText)}
        >
          {#if copied}<Check class="h-4 w-4" />コピー済{:else}<Copy class="h-4 w-4" />全文コピー{/if}
        </button>
      {/if}
      <button
        class="inline-flex items-center gap-1.5 rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white shadow-ml transition hover:opacity-90"
        onclick={() => fileInput?.click()}
      >
        <FolderOpen class="h-4 w-4" />ファイルを開く
      </button>
      <input
        bind:this={fileInput}
        type="file"
        accept={IMAGE_ACCEPT}
        class="hidden"
        onchange={(e) => {
          const input = e.currentTarget as HTMLInputElement;
          openFile(firstImageFile(input.files));
          input.value = '';
        }}
      />
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

  {#if !imageUrl}
    <button
      class="flex min-h-72 flex-col items-center justify-center gap-3 rounded-ml border-2 border-dashed p-8 text-center transition {dragging
        ? 'border-accent bg-accent-soft'
        : 'border-ink-faint/50 bg-surface-elevated hover:border-accent/60'}"
      onclick={() => fileInput?.click()}
    >
      <ImageIcon class="h-10 w-10 text-ink-faint" />
      <span class="text-base font-medium">画像をドロップ、または Ctrl+V で貼り付け</span>
      <span class="text-sm text-ink-muted">PNG・JPEG・WebP（40 MB まで）。クリックでファイル選択。</span>
    </button>
  {:else}
    <div class="grid items-start gap-4 md:grid-cols-[minmax(0,1fr)_18rem]">
      <section
        class="relative flex justify-center overflow-hidden rounded-ml border bg-surface-muted p-2 {dragging
          ? 'border-accent'
          : 'border-border'}"
        aria-label="画像"
      >
        <div class="relative">
          <img
            bind:this={imgEl}
            src={imageUrl}
            alt={imageName}
            class="block max-h-[calc(100vh-12rem)] max-w-full select-none"
            onload={measure}
            draggable="false"
          />
          {#if crop && !showOriginal}
            {#each blocks as b, i (i)}
              {#if b.bbox[2] > 0 && !b.same_language}
                {@const r = blockCssRect(b, crop, scale)}
                <div
                  class="block-box absolute"
                  class:pending={b.translation == null}
                  style="left: {r.left}px; top: {r.top}px; width: {r.width}px; height: {r.height}px;
                         background: {rgb(b.colors?.background, 'rgba(255,255,255,0.96)')};
                         color: {rgb(b.colors?.foreground, '#111')};"
                  title={b.source}
                >
                  {#if b.translation != null}
                    <FlowText
                      block={b}
                      text={b.translation}
                      {scale}
                      grow={growRoom(blocks, i, crop.w) / scale}
                      background={rgb(b.colors?.background, 'rgba(255,255,255,0.96)')}
                    />
                  {/if}
                </div>
              {/if}
            {/each}
          {/if}
        </div>
        {#if busy}
          <div
            class="absolute top-3 left-1/2 inline-flex -translate-x-1/2 items-center gap-2 rounded-full bg-black/75 px-4 py-1.5 text-xs text-white"
            aria-live="polite"
          >
            <Loader2 class="h-3.5 w-3.5 animate-spin" />
            {phase === 'ocr' ? '文字認識中…' : '翻訳中…'}
          </div>
        {/if}
      </section>

      <aside class="flex max-h-[calc(100vh-12rem)] flex-col rounded-ml border border-border bg-surface-elevated shadow-ml">
        <div class="flex items-center justify-between border-b border-border px-4 py-2.5 text-sm">
          <span class="font-medium">認識結果</span>
          <span class="text-xs text-ink-faint">{blocks.length} ブロック</span>
        </div>
        <ol class="min-h-0 flex-1 divide-y divide-border overflow-y-auto">
          {#each blocks as b, i (i)}
            <li class="px-4 py-2.5 text-sm">
              <p class="text-xs break-words whitespace-pre-wrap text-ink-faint">{b.source}</p>
              {#if b.same_language}
                <p class="mt-1 text-xs text-ink-faint">（翻訳対象外）</p>
              {:else if b.translation != null}
                <p class="mt-1 break-words whitespace-pre-wrap select-text">{b.translation}</p>
              {:else}
                <p class="mt-1 text-xs text-ink-faint">翻訳待ち…</p>
              {/if}
            </li>
          {:else}
            <li class="px-4 py-6 text-center text-sm text-ink-faint">
              {#if busy}文字認識中…{:else if plainText}<span class="break-words whitespace-pre-wrap text-ink"
                  >{plainText}</span
                >{:else if result}文字なし{/if}
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
  {/if}
</main>

<style>
  .block-box {
    border-radius: 3px;
    user-select: text;
    cursor: text;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.04);
  }
  .block-box.pending {
    background-image: linear-gradient(100deg, transparent 30%, rgba(139, 92, 246, 0.18) 50%, transparent 70%) !important;
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
