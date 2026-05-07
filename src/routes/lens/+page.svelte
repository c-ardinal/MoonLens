<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { X, Radio, GripHorizontal } from 'lucide-svelte';
  import {
    onLensFlowDone,
    onLensFlowStart,
    onLensLineChunk,
    onLensMask,
    onLensOcrLines,
    type LensFlowDone,
    type LensFlowStart,
    type LensLineChunk,
    type LensMask,
    type LensOcrLines
  } from '$lib/ipc';

  const params = new URLSearchParams(
    typeof window !== 'undefined' ? window.location.search : ''
  );
  const id = $derived(params.get('id') ?? '');

  interface LineCard {
    line_index: number;
    text: string;
    bbox: [number, number, number, number];
    source_bbox: [number, number, number, number];
    bg_color: [number, number, number];
    bg_gradient: {
      angle: number;
      from: [number, number, number];
      to: [number, number, number];
      stops?: [number, number, number][];
    } | null;
    fg_color: [number, number, number];
    font_weight: number;
    /** Source-derived font size in image pixels. */
    font_size_px: number | undefined;
    /** Source-derived CSS `line-height` ratio. */
    line_height: number | undefined;
    /** Target line count for the translation; excess lines
     *  trigger horizontal compression instead of resizing. */
    source_line_count: number | undefined;
    /** Source text horizontal alignment, mirrored onto the
     *  rendered translation. */
    text_align: 'left' | 'center' | 'right' | undefined;
    /** Tight inpaint region within the wider card bbox. */
    inpaint_bbox: [number, number, number, number] | undefined;
    /** Base64 PNG of the inpainted source region. Preferred card
     *  background; flat colour / gradient only kick in if the
     *  backend could not decode the captured frame. */
    bg_patch: string | null;
    translation: string;
    done: boolean;
  }

  // `visibleLines` = cards actually rendered on screen. A card only
  // moves into this set when its translation chunk reports
  // `done=true` (or the flow-done terminator sweeps up stragglers),
  // so the user never sees partial tokens or empty placeholders.
  //
  // `pendingLines` buffers the OCR layout for the current flow.
  // Chunks accumulate in `pendingChunks` keyed by `line_index` and
  // are promoted into `visibleLines` via `commitLine`.
  //
  // Previous flow's cards stay visible until their slot is
  // overwritten by the new flow's `done=true` chunk, so the lens
  // never blanks to empty between captures.
  let visibleLines = $state<LineCard[]>([]);
  let pendingLines: LineCard[] = [];
  let pendingChunks = new Map<number, { text: string; done: boolean }>();
  let imageW = $state(1);
  let imageH = $state(1);
  let currentFlowId = $state<number | null>(null);
  let streaming = $state(false);
  let hovered = $state(false);
  let lastError = $state<string | null>(null);
  // `masked` is only driven from the backend on non-Windows builds:
  // Windows now uses WDA_EXCLUDEFROMCAPTURE to hide the lens window
  // from screen capture, so the opacity-0 dance is skipped and the
  // cards stay fully visible through every poll.
  let masked = $state(false);

  const unlisteners: UnlistenFn[] = [];
  const win = getCurrentWebviewWindow();

  function beginDrag(e: MouseEvent): void {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (target && (target.closest('button') || target.closest('.lens-card'))) return;
    win.startDragging().catch((err: unknown) => {
      lastError = `drag failed: ${err}`;
    });
  }

  async function close(): Promise<void> {
    try {
      await invoke('close_lens_window', { id });
    } catch (e) {
      lastError = String(e);
    }
  }

  function copyAll(): void {
    if (!visibleLines.length) return;
    const text = visibleLines.map((l) => l.translation || l.text).join('\n');
    navigator.clipboard.writeText(text).catch(() => {});
  }

  function handleKey(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    } else if (e.key === 'c' && (e.metaKey || e.ctrlKey)) {
      copyAll();
    }
  }

  /** Bbox → CSS percentage position inside the lens body. Each
   *  card is one paragraph, so the translation text wraps inside the
   *  card bbox instead of sitting on one line anchored at the OCR
   *  source position. */
  function cardStyle(line: LineCard): string {
    const [x, y, w, h] = line.bbox;
    if (imageW === 0 || imageH === 0) return 'display:none';
    const [br, bgc, bb] = line.bg_color;
    const [fr, fg, fb] = line.fg_color;
    const weight = line.font_weight || 400;
    // Prefer the backend-computed source-derived font size when
    // available — keeps eval and production visually aligned and
    // guarantees the translation renders at the same scale as the
    // original glyphs. Legacy payloads without `font_size_px` fall
    // back to the old area/chars heuristic so older backends still
    // render something readable.
    let fontPx: number;
    if (typeof line.font_size_px === 'number' && line.font_size_px > 0) {
      // Backend computes font size in captured-image PHYSICAL pixels
      // (source_bbox[3] * 0.82). CSS `px` is a LOGICAL unit — the
      // browser multiplies by devicePixelRatio when rasterising to
      // the screen. On DPR > 1 this double-scales the text, so the
      // translation renders (DPR×) bigger than the source glyphs,
      // spilling out of its card and *looking* like a position drift.
      // The eval renders straight to a PNG in physical pixels, so it
      // never hits this class of bug — this divide reunites the two
      // paths visually.
      const dpr = typeof window !== 'undefined' && window.devicePixelRatio > 0
        ? window.devicePixelRatio
        : 1;
      fontPx = Math.max(7, Math.min(80, line.font_size_px / dpr));
    } else {
      const text = line.translation.length > 0 ? line.translation : line.text;
      const chars = Math.max(1, text.length);
      const area = Math.max(1, w * h);
      const K = 0.55;
      const rawFont = Math.sqrt(area / chars / K);
      fontPx = Math.max(9, Math.min(42, rawFont));
    }
    // Preferred background is the inpainted PNG patch — it makes
    // the card disappear into the source even on photos and complex
    // gradients. Fall back to the rim-sampled gradient (2-5 stops)
    // when the backend could not decode the frame, and then to a
    // flat rim-sample fill as a last resort.
    let background: string;
    if (line.bg_patch) {
      const url = `data:image/png;base64,${line.bg_patch}`;
      // The card bbox is widened in the alignment direction so
      // CSS `text-align` has slack to position the translation;
      // the bg_patch still only covers the source text region
      // (`inpaint_bbox`). Position the patch at its offset within
      // the wider card so it overlays the source glyphs exactly,
      // leaving the rest of the card transparent (source pixels
      // show through, translation can overflow into them
      // naturally).
      if (line.inpaint_bbox && imageW > 0 && imageH > 0) {
        const [ix, iy, iw, ih] = line.inpaint_bbox;
        // CSS background-position percentage uses the slack
        // formula: pos% = (container - image) * pos% / 100. So a
        // pixel offset of `ix - x` inside a card of width `w`
        // with image width `iw` translates to:
        //   bgPosX = (offset_px) / (w - iw) * 100
        // Guard against the degenerate `w == iw` case (no slack)
        // by falling back to 0% — the image fills the card and
        // position is moot.
        const sizeW = (iw / w) * 100;
        const sizeH = (ih / h) * 100;
        const slackW = w - iw;
        const slackH = h - ih;
        const bgPosX = slackW > 0.5 ? ((ix - x) / slackW) * 100 : 0;
        const bgPosY = slackH > 0.5 ? ((iy - y) / slackH) * 100 : 0;
        background =
          `background-image:url("${url}");` +
          `background-position:${bgPosX.toFixed(2)}% ${bgPosY.toFixed(2)}%;` +
          `background-size:${sizeW.toFixed(2)}% ${sizeH.toFixed(2)}%;` +
          'background-repeat:no-repeat;';
      } else {
        background =
          `background-image:url("${url}");` +
          'background-size:100% 100%;background-repeat:no-repeat;';
      }
    } else if (line.bg_gradient) {
      const g = line.bg_gradient;
      const stops = g.stops && g.stops.length >= 2 ? g.stops : [g.from, g.to];
      const n = stops.length;
      const stopsCss = stops
        .map(([r, gg, bv], i) => {
          const pct = ((i * 100) / (n - 1)).toFixed(1);
          return `rgb(${r},${gg},${bv}) ${pct}%`;
        })
        .join(', ');
      background = `background:linear-gradient(${g.angle}deg, ${stopsCss});`;
    } else {
      background = `background-color:rgb(${br},${bgc},${bb});`;
    }
    const lineHeight =
      typeof line.line_height === 'number' && line.line_height > 0 ? line.line_height : 1.25;
    const textAlign =
      line.text_align === 'center' || line.text_align === 'right' ? line.text_align : 'left';
    return (
      `left:${(x / imageW) * 100}%;` +
      `top:${(y / imageH) * 100}%;` +
      `width:${(w / imageW) * 100}%;` +
      `height:${(h / imageH) * 100}%;` +
      `--font-px:${fontPx.toFixed(1)};` +
      `--fw:${weight};` +
      `--lh:${lineHeight.toFixed(2)};` +
      `--ta:${textAlign};` +
      background +
      `color:rgb(${fr},${fg},${fb});`
    );
  }

  /// After render, check whether the wrapped translation actually
  /// overflows the card's vertical content area. If it does, we
  /// horizontally compress via `scaleX` — the text stays at the
  /// source font-size and weight, it just squeezes into narrower
  /// glyphs (縦長) so it still fits in the original row budget.
  ///
  /// The measurement uses `scrollHeight` vs `clientHeight` directly
  /// (both include the same padding box) instead of dividing by a
  /// computed `line-height`. Browsers report `line-height` either
  /// as a px value or as a unitless ratio depending on how the
  /// stylesheet declared it; mixing the two would either over- or
  /// under-count rendered lines and falsely trigger compression on
  /// translations that were already fitting at 1.0.
  ///
  /// A 5% tolerance shields against sub-pixel rounding (Chromium's
  /// `scrollHeight` is rounded up to the next integer pixel even
  /// when the actual overflow is < 1 px) so single-line cards do
  /// not get pointlessly squeezed.
  function applySourceLineFit(): void {
    if (typeof document === 'undefined') return;
    const els = document.querySelectorAll<HTMLElement>('.line-text');
    for (const el of Array.from(els)) {
      // Reset any prior compression so we measure at the natural
      // 1.0 scale first.
      el.style.transform = '';
      el.style.width = '';
      const client = Math.max(1, el.clientHeight);
      const overflowRatio = el.scrollHeight / client;
      if (overflowRatio <= 1.05) continue;
      // Compress horizontally just enough to bring the rendered
      // content height back inside the card. Floor at 0.5 to keep
      // glyphs legible — past that, accept the truncation rather
      // than render unreadable text.
      const ratio = Math.max(0.5, 1 / overflowRatio);
      el.style.transformOrigin = 'top left';
      el.style.transform = `scaleX(${ratio.toFixed(3)})`;
      // Expand the drawing width so scaled text fills the card
      // again (scaleX shrinks the layout box too).
      el.style.width = `${(100 / ratio).toFixed(2)}%`;
    }
  }

  $effect(() => {
    // Re-measure whenever the visible set changes (new flow's cards
    // committed, or a chunk finished streaming). `visibleLines` is
    // the dependency tracked by Svelte 5 runes; the timeout lets the
    // DOM flush before we measure scrollHeight.
    void visibleLines;
    if (typeof window !== 'undefined') {
      window.setTimeout(applySourceLineFit, 0);
    }
  });

  function commitLine(lineIndex: number): void {
    const src = pendingLines.find((l) => l.line_index === lineIndex);
    if (!src) return;
    const chunk = pendingChunks.get(lineIndex);
    const raw = chunk?.text ?? '';
    const translation = raw.trim().length > 0 ? raw.trim() : src.text;
    const committed: LineCard = {
      ...src,
      translation,
      done: chunk?.done ?? true
    };
    const idx = visibleLines.findIndex((l) => l.line_index === lineIndex);
    if (idx >= 0) {
      const next = visibleLines.slice();
      next[idx] = committed;
      visibleLines = next;
    } else {
      visibleLines = [...visibleLines, committed];
    }
  }

  function forceCommitRemaining(): void {
    const validIndices = new Set(pendingLines.map((l) => l.line_index));
    const pruned = visibleLines.filter((l) => validIndices.has(l.line_index));
    if (pruned.length !== visibleLines.length) {
      visibleLines = pruned;
    }
    for (const l of pendingLines) {
      const already = visibleLines.some((v) => v.line_index === l.line_index);
      if (already) continue;
      commitLine(l.line_index);
    }
  }

  onMount(async () => {
    unlisteners.push(
      await onLensFlowStart((e: LensFlowStart) => {
        if (e.id !== id) return;
        currentFlowId = e.flow_id;
        streaming = true;
        lastError = null;
        pendingLines = [];
        pendingChunks.clear();
      })
    );

    unlisteners.push(
      await onLensOcrLines((e: LensOcrLines) => {
        if (e.id !== id) return;
        if (currentFlowId !== null && e.flow_id !== currentFlowId) return;
        imageW = Math.max(1, e.image_w);
        imageH = Math.max(1, e.image_h);
        pendingLines = e.lines.map((l) => ({
          line_index: l.line_index,
          text: l.text,
          bbox: l.bbox,
          source_bbox: l.source_bbox ?? l.bbox,
          bg_color: l.bg_color,
          bg_gradient: l.bg_gradient ?? null,
          fg_color: l.fg_color,
          font_weight: l.font_weight ?? 400,
          font_size_px: l.font_size_px,
          line_height: l.line_height,
          source_line_count: l.source_line_count,
          text_align: l.text_align,
          inpaint_bbox: l.inpaint_bbox,
          bg_patch: l.bg_patch ?? null,
          translation: '',
          done: false
        }));
        if (pendingLines.length === 0) {
          visibleLines = [];
        }
      })
    );

    unlisteners.push(
      await onLensLineChunk((e: LensLineChunk) => {
        if (e.id !== id) return;
        if (currentFlowId !== null && e.flow_id !== currentFlowId) return;
        const existing = pendingChunks.get(e.line_index) ?? { text: '', done: false };
        pendingChunks.set(e.line_index, {
          text: existing.text + (e.delta ?? ''),
          done: existing.done || e.done
        });
        if (e.done) {
          commitLine(e.line_index);
        }
      })
    );

    unlisteners.push(
      await onLensFlowDone((e: LensFlowDone) => {
        if (e.id !== id) return;
        if (currentFlowId !== null && e.flow_id !== currentFlowId) return;
        streaming = false;
        forceCommitRemaining();
      })
    );

    unlisteners.push(
      await onLensMask((e: LensMask) => {
        if (e.id !== id) return;
        masked = e.masked;
      })
    );

    window.addEventListener('keydown', handleKey);
  });

  onDestroy(() => {
    for (const u of unlisteners) u();
    if (typeof window !== 'undefined') {
      window.removeEventListener('keydown', handleKey);
    }
  });
</script>

<!--
  Lens UX:
  - main is fully transparent so source text shows through.
  - Cards render only after each line's translation is `done`, so
    the user sees whole blocks, never mid-stream tokens.
  - On Windows, WDA_EXCLUDEFROMCAPTURE makes the window invisible
    to screen capture; `masked` stays false and the cards never
    disappear. On Linux the backend still emits the mask signal as
    a fallback, and the CSS transition softens the visual pulse.
-->
<main
  class="relative h-screen w-screen overflow-hidden transition-opacity duration-75"
  class:opacity-0={masked}
  style="background: transparent;"
  onmouseenter={() => (hovered = true)}
  onmouseleave={() => (hovered = false)}
  onmousedown={beginDrag}
  role="presentation"
>
  <header
    class="pointer-events-none absolute inset-x-0 top-0 z-30 flex h-6 select-none items-center justify-between gap-2 border-b border-violet-300/40 bg-black/70 px-2 text-white transition-opacity duration-150"
    class:opacity-0={!hovered}
    class:opacity-100={hovered}
  >
    <span class="flex items-center gap-1 font-mono text-[10px] text-white/80">
      <GripHorizontal class="h-3 w-3" />
      lens {id.replace('lens-', '')}
      {#if streaming}
        <Radio class="ml-2 h-3 w-3 animate-pulse text-violet-300" />
        <span class="text-violet-300">受信中</span>
      {/if}
    </span>
    <button
      class="pointer-events-auto rounded-sm p-0.5 text-white/80 hover:bg-white/20 hover:text-white"
      onclick={close}
      title="閉じる (ESC)"
      aria-label="閉じる"
    >
      <X class="h-3.5 w-3.5" />
    </button>
  </header>

  {#each visibleLines as line (line.line_index)}
    <div class="lens-card absolute overflow-hidden" style={cardStyle(line)}>
      <p class="line-text" data-line-index={line.line_index}>{line.translation}</p>
    </div>
  {/each}

  {#if visibleLines.length === 0 && !streaming && hovered}
    <div class="pointer-events-none absolute inset-0 flex items-center justify-center text-center">
      <p class="rounded bg-black/70 px-3 py-2 text-[11px] leading-relaxed text-white">
        対象テキストの上に重ねると自動で翻訳します
      </p>
    </div>
  {/if}

  {#if lastError}
    <p class="absolute inset-x-2 bottom-2 z-30 rounded bg-red-500/90 px-2 py-1 text-[11px] text-white">
      {lastError}
    </p>
  {/if}
</main>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
  }
  .lens-card {
    container-type: size;
  }
  .line-text {
    /* The card IS the paragraph now — fill it completely and let
       the translation text wrap inside. The inpainted bg patch
       carries the proper region so the text needs no padding to
       avoid hitting card edges visually.
       NOTE: percentage padding (2% 3%) was a regression — CSS
       resolves BOTH horizontal and vertical % padding against the
       containing block's WIDTH. On a wide-but-short card (e.g.
       800×30 px) that produced padding-top of 16 px, eating most
       of the vertical content area and pushing scrollHeight past
       clientHeight, which falsely triggered scaleX compression
       on translations that already fit. */
    position: absolute;
    inset: 0;
    margin: 0;
    padding: 0;
    box-sizing: border-box;
    overflow: hidden;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    word-break: break-word;
    font-size: calc(var(--font-px, 14) * 1px);
    font-weight: var(--fw, 400);
    /* Source-derived ratio; JS rewrites scaleX when the wrapped
       output would exceed `source_line_count` rows so the text
       stays the same size/weight but squeezes horizontally. */
    line-height: var(--lh, 1.25);
    /* Mirror the source's horizontal alignment so a centred
       heading stays centred and a right-aligned status strip
       stays right-aligned even when the translation is narrower
       than the source. */
    text-align: var(--ta, left);
  }
</style>
