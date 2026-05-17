<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { X, Pause, Play, Pin, Settings, Cpu } from 'lucide-svelte';
  import MoonLensMark from '$lib/components/MoonLensMark.svelte';
  import {
    formatLanguagePair,
    getDefaultLanguagePair,
    getSettings,
    showMainRoute,
  } from '$lib/ipc';
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
  /** `pendingLines` is now read by the template to render
   *  placeholder shimmers between OCR-done and translation-done.
   *  It needs to be reactive so Svelte 5 picks up new pendingLines
   *  on the LensOcrLines event without us manually re-triggering. */
  let pendingLines = $state<LineCard[]>([]);
  let pendingChunks = new Map<number, { text: string; done: boolean }>();
  let imageW = $state(1);
  let imageH = $state(1);
  let currentFlowId = $state<number | null>(null);
  let streaming = $state(false);
  let hovered = $state(false);
  let lastError = $state<string | null>(null);
  let paused = $state(false);
  let pinned = $state(false);
  let lastLatencyMs = $state<number | null>(null);
  let flowStartedAt = 0;
  /** Source/target codes from settings (`translation.default_*`). The
   *  Rust-side watcher reads the same settings to drive the actual
   *  translate call, so what we display here always matches what is
   *  being computed — no more "EN→JA" hardcoded lie when the user has
   *  switched their target to ZH or KO. */
  let sourceLang = $state('auto');
  let targetLang = $state('ja');
  const languagePair = $derived(formatLanguagePair(sourceLang, targetLang));
  /** Active provider IDs for OCR and translate, read from the
   *  settings provider chains. The footer used to hardcode
   *  `OCR: PP-OCRv5` / `翻訳: Gemma-3`, which lied to anyone who
   *  had reordered or swapped their chain. We display the
   *  chain's head — what the pipeline actually invokes first. */
  let ocrEngine = $state<string>('—');
  let translateEngine = $state<string>('—');
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

  async function togglePin(): Promise<void> {
    pinned = !pinned;
    try {
      await win.setAlwaysOnTop(pinned);
    } catch (e) {
      pinned = !pinned;
      lastError = `pin failed: ${e}`;
    }
  }

  /**
   * Toggle local pause. The Rust-side watcher keeps polling
   * (cheap; just samples the captured region for diff) but while
   * `paused === true` the event handlers below short-circuit so
   * the visible card set stays frozen. This is the cheapest pause
   * primitive — no IPC roundtrip — and avoids tearing down /
   * restarting the watcher on every Pause click.
   */
  function togglePause(): void {
    paused = !paused;
    if (paused) {
      // Surface the pause visually: keep current cards but stop
      // accepting in-flight chunks until the user un-pauses.
      streaming = false;
      pendingChunks.clear();
    }
  }

  /**
   * Open the main app's Settings page. Lens-specific tweaks live
   * under /settings/lens (the cog icon's natural destination), so
   * we route there directly. The main window is brought forward
   * and focused on the Rust side; the lens stays open so the user
   * can return to it.
   */
  async function openSettings(): Promise<void> {
    try {
      await showMainRoute('/settings/');
    } catch (e) {
      lastError = `settings open failed: ${e}`;
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
    // Contrast guard: when the inpainted patch competes with the
    // text colour, drop a 1px shadow in the inverted-luminance
    // direction so the glyphs read on busy photo backgrounds. On
    // solid surfaces the shadow is omitted to keep type crisp.
    const guard = needsContrastGuard(line);
    const shadowColor =
      fr * 0.299 + fg * 0.587 + fb * 0.114 < 128
        ? 'rgba(255,255,255,0.55)'
        : 'rgba(0,0,0,0.55)';
    const textShadow = guard
      ? `text-shadow:0 1px 2px ${shadowColor},0 0 1px ${shadowColor};`
      : '';
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
      `color:rgb(${fr},${fg},${fb});` +
      textShadow
    );
  }

  /// Fit translation to its card. Strategy in order:
  ///   1. Measure: does the rendered text overflow?
  ///   2. If yes, reduce font-size proportionally (down to 70%).
  ///   3. If still overflowing, fall back to horizontal scaleX
  ///      compression (down to 60%) — the prior single-step
  ///      behaviour. Stretching is uglier than shrinking so it's
  ///      a last-resort guard, not the default path.
  ///
  /// The measurement uses `scrollHeight` vs `clientHeight` directly
  /// (both include the same padding box) so the test is independent
  /// of how the stylesheet expresses line-height (px vs unitless).
  /// A 5% tolerance shields against Chromium's integer-pixel
  /// rounding so already-fitting single-line cards aren't pointlessly
  /// shrunk.
  function applySourceLineFit(): void {
    if (typeof document === 'undefined') return;
    const els = document.querySelectorAll<HTMLElement>('.line-text');
    for (const el of Array.from(els)) {
      // Reset any prior fit state so we measure at the natural
      // size first.
      el.style.transform = '';
      el.style.width = '';
      const baseFontPx = parseFloat(
        getComputedStyle(el).getPropertyValue('--font-px') || '14',
      );
      el.style.fontSize = `${baseFontPx}px`;
      const client = Math.max(1, el.clientHeight);
      let overflowRatio = el.scrollHeight / client;
      if (overflowRatio <= 1.05) continue;

      // Step 1: shrink font-size. Most translations only overflow
      // by 5-30%, which a 5-25% font reduction reliably resolves
      // without distorting glyph proportions.
      const sizeRatio = Math.max(0.7, 1 / overflowRatio);
      const shrunkPx = Math.max(7, baseFontPx * sizeRatio);
      el.style.fontSize = `${shrunkPx.toFixed(2)}px`;
      overflowRatio = el.scrollHeight / Math.max(1, el.clientHeight);
      if (overflowRatio <= 1.05) continue;

      // Step 2: still overflowing — fall back to horizontal scaleX
      // compression on top of the smaller font size. Floor at 0.6
      // so glyphs stay legible; past that, accept clipping rather
      // than render unreadable narrow strokes.
      const ratio = Math.max(0.6, 1 / overflowRatio);
      el.style.transformOrigin = 'top left';
      el.style.transform = `scaleX(${ratio.toFixed(3)})`;
      el.style.width = `${(100 / ratio).toFixed(2)}%`;
    }
  }

  /// Click-to-copy on individual translation cards. Falls back to
  /// the source text when the translation is empty (e.g. the
  /// pipeline returned a passthrough). The copy is silent — the
  /// only feedback is the `lens-card-copied` ripple class we toggle
  /// for ~600ms.
  async function copyCard(line: LineCard, event?: MouseEvent): Promise<void> {
    const text = line.translation || line.text;
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // best-effort — older webviews can refuse cross-window
      // clipboard writes; the user can still use ⌘C for all-card
      // copy from the header path.
      return;
    }
    const target = event?.currentTarget as HTMLElement | null;
    if (target) {
      target.classList.add('lens-card-copied');
      window.setTimeout(() => target.classList.remove('lens-card-copied'), 620);
    }
  }

  /// Decide whether the translation needs a contrast-guard
  /// text-shadow. Returns true when fg ↔ bg WCAG luminance contrast
  /// is below ~3.0 (well below AA), which means the inpainted patch
  /// is fighting the text for visibility. The shadow is otherwise
  /// off to keep crisp text crisp on solid surfaces.
  function needsContrastGuard(line: LineCard): boolean {
    const lum = (rgb: [number, number, number]): number => {
      const norm = rgb.map((c) => {
        const v = c / 255;
        return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * norm[0] + 0.7152 * norm[1] + 0.0722 * norm[2];
    };
    const lFg = lum(line.fg_color);
    const lBg = lum(line.bg_color);
    const ratio = (Math.max(lFg, lBg) + 0.05) / (Math.min(lFg, lBg) + 0.05);
    return ratio < 3.0;
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
    // Pull the user's configured pair before the first flow event
    // lands. The backend already consulted settings to choose the
    // translate target, so reading the same source here keeps the
    // footer pill honest from the very first paint.
    try {
      const pair = await getDefaultLanguagePair();
      sourceLang = pair.source;
      targetLang = pair.target;
    } catch {
      // Defensive: keep the auto/ja seed values if settings fail.
    }
    // Pull the active OCR / translate provider IDs so the footer
    // reflects what's actually running. The Lens window is a
    // separate webview so we can't subscribe to the global
    // `currentSettings` store directly; one-shot fetch on mount is
    // enough because a chain edit while a Lens is open is rare and
    // the next Lens spawn will pick it up.
    try {
      const s = (await getSettings()) as {
        providers?: {
          ocr?: { chain?: string[] };
          translate?: { chain?: string[] };
        };
      } | null;
      const ocrChain = s?.providers?.ocr?.chain;
      const tlChain = s?.providers?.translate?.chain;
      if (Array.isArray(ocrChain) && ocrChain.length > 0) {
        ocrEngine = ocrChain[0];
      }
      if (Array.isArray(tlChain) && tlChain.length > 0) {
        translateEngine = tlChain[0];
      }
    } catch {
      // Footer falls back to "—" placeholders when settings fail.
    }

    unlisteners.push(
      await onLensFlowStart((e: LensFlowStart) => {
        if (e.id !== id) return;
        // While paused we surface neither the spinner nor a flow
        // reset — the user's frozen card set stays put. Resuming
        // re-arms on the very next event from the backend.
        if (paused) return;
        currentFlowId = e.flow_id;
        streaming = true;
        lastError = null;
        pendingLines = [];
        pendingChunks.clear();
        flowStartedAt = performance.now();
      })
    );

    unlisteners.push(
      await onLensOcrLines((e: LensOcrLines) => {
        if (e.id !== id) return;
        if (paused) return;
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
        if (paused) return;
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
        if (paused) return;
        if (currentFlowId !== null && e.flow_id !== currentFlowId) return;
        streaming = false;
        forceCommitRemaining();
        if (flowStartedAt > 0) {
          lastLatencyMs = Math.round(performance.now() - flowStartedAt);
          flowStartedAt = 0;
        }
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
  class="lens-shell"
  class:opacity-0={masked}
  class:lens-hovered={hovered}
  onmouseenter={() => (hovered = true)}
  onmouseleave={() => (hovered = false)}
  onmousedown={beginDrag}
  role="presentation"
>
  <!--
    Persistent header bar — matches the design language used in the
    main app. Logo + Lens label + status pill + EN→JA + control
    buttons. Pointer-events are auto only on interactive children so
    drag-to-move still works on the bar background.
  -->
  <header class="lens-bar lens-bar-top" data-tauri-drag-region>
    <MoonLensMark size={14} />
    <span class="lens-bar-title">Lens</span>
    <span class="lens-bar-divider" aria-hidden="true"></span>
    <span class="lens-bar-status">
      <span class="lens-bar-dot" class:running={streaming && !paused}></span>
      {#if paused}
        一時停止
      {:else if streaming}
        リアルタイム翻訳中
      {:else}
        待機中
      {/if}
    </span>
    <span class="lens-bar-pair">{languagePair}</span>
    <div class="lens-bar-actions">
      <button
        class="lens-icon-btn"
        type="button"
        onclick={togglePause}
        title={paused ? '再開' : '一時停止'}
        aria-label={paused ? '再開' : '一時停止'}
      >
        {#if paused}
          <Play class="h-3 w-3" aria-hidden="true" />
        {:else}
          <Pause class="h-3 w-3" aria-hidden="true" />
        {/if}
      </button>
      <button
        class="lens-icon-btn"
        type="button"
        onclick={togglePin}
        class:active={pinned}
        title="ピン留め"
        aria-label="ピン留め"
      >
        <Pin class="h-3 w-3" aria-hidden="true" />
      </button>
      <button
        class="lens-icon-btn"
        type="button"
        onclick={openSettings}
        title="設定を開く"
        aria-label="設定を開く"
      >
        <Settings class="h-3 w-3" aria-hidden="true" />
      </button>
      <button
        class="lens-icon-btn"
        type="button"
        onclick={close}
        title="閉じる (ESC)"
        aria-label="閉じる"
      >
        <X class="h-3 w-3" aria-hidden="true" />
      </button>
    </div>
  </header>

  <!--
    Scanning effect — a soft accent-coloured beam moving top→bottom
    while a flow is in progress. Pure CSS keyframe animation, fully
    decorative, pointer-events: none.
  -->
  {#if streaming && !paused}
    <div class="lens-scan" aria-hidden="true"></div>
  {/if}

  <!-- Corner brackets — frame the active translation area -->
  <div class="lens-bracket lens-bracket-tl" aria-hidden="true"></div>
  <div class="lens-bracket lens-bracket-tr" aria-hidden="true"></div>
  <div class="lens-bracket lens-bracket-bl" aria-hidden="true"></div>
  <div class="lens-bracket lens-bracket-br" aria-hidden="true"></div>

  {#each visibleLines as line (line.line_index)}
    <button
      type="button"
      class="lens-card absolute overflow-hidden"
      class:lens-card-translating={!line.done}
      style={cardStyle(line)}
      onclick={(e) => void copyCard(line, e)}
      title="クリックでコピー"
      aria-label="翻訳をコピー: {line.translation || line.text}"
    >
      <p class="line-text" data-line-index={line.line_index}>
        {line.translation || line.text}
      </p>
      {#if !line.done}
        <span class="lens-card-shimmer" aria-hidden="true"></span>
      {/if}
    </button>
  {/each}

  <!--
    Pending placeholders: OCR has reported lines but the
    translation chunk hasn't arrived yet. We render a low-opacity
    skeleton over each pending bbox so the user sees that the lens
    is working, instead of a blank inpaint gap. Once the chunk
    arrives (commitLine) the placeholder is replaced by the real
    card above.
  -->
  {#each pendingLines as line (line.line_index)}
    {#if !visibleLines.some((v) => v.line_index === line.line_index)}
      <div
        class="lens-card lens-card-pending absolute overflow-hidden"
        style={cardStyle({ ...line, translation: '', done: false })}
        aria-hidden="true"
      >
        <span class="lens-card-shimmer"></span>
      </div>
    {/if}
  {/each}

  {#if visibleLines.length === 0 && !streaming && hovered}
    <div class="lens-empty">
      <p>対象テキストの上に重ねると自動で翻訳します</p>
    </div>
  {/if}

  <!--
    Persistent footer bar — timing + engine info + block count. Shown
    permanently so the user can confirm the lens is healthy without
    relying on hover. Mirrors `LensWindow` footer in the design.
  -->
  <footer class="lens-bar lens-bar-bottom">
    <Cpu class="h-3 w-3" aria-hidden="true" />
    {#if lastLatencyMs !== null}
      <span class="lens-bar-mono">{lastLatencyMs} ms</span>
    {:else if streaming}
      <span class="lens-bar-mono">…</span>
    {:else}
      <span class="lens-bar-mono">—</span>
    {/if}
    <span class="lens-bar-sep">·</span>
    <span>OCR: <span class="lens-bar-mono">{ocrEngine}</span></span>
    <span class="lens-bar-sep">·</span>
    <span>翻訳: <span class="lens-bar-mono">{translateEngine}</span></span>
    <span class="lens-bar-spacer"></span>
    <span class="lens-bar-mono">{visibleLines.length} blocks</span>
  </footer>

  {#if lastError}
    <p class="lens-error" role="alert">{lastError}</p>
  {/if}
</main>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
  }

  /* ============================================================
     Lens shell — chromeless, transparent overlay window. Header
     and footer bars sit on top of the translation cards. The
     window itself is fully transparent; bars use a translucent
     dark surface with backdrop blur for legibility on any
     background.
     ============================================================ */
  .lens-shell {
    position: relative;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    background: transparent;
    border-radius: 12px;
    /* Soft outer edge so the lens window reads as a discrete
       surface even when nothing is being translated. */
    box-shadow:
      0 24px 60px rgba(0, 0, 0, 0.35),
      0 0 0 1px rgba(255, 255, 255, 0.06) inset;
    transition: opacity 75ms linear, box-shadow 220ms ease;
  }
  .lens-shell.lens-hovered {
    box-shadow:
      0 24px 60px rgba(0, 0, 0, 0.45),
      0 0 0 1px rgba(168, 197, 218, 0.18) inset;
  }
  .lens-shell.opacity-0 {
    opacity: 0;
    pointer-events: none;
  }

  /* ============================================================
     Top / bottom bars
     ============================================================ */
  .lens-bar {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: rgba(13, 14, 16, 0.62);
    backdrop-filter: blur(28px) saturate(140%);
    -webkit-backdrop-filter: blur(28px) saturate(140%);
    color: #e8eaee;
    font-size: 11.5px;
    font-weight: 500;
    letter-spacing: 0.01em;
    user-select: none;
    z-index: 30;
    opacity: 0;
    transition: opacity 160ms ease;
  }
  .lens-shell.lens-hovered .lens-bar {
    opacity: 1;
  }
  .lens-bar-top {
    top: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    border-top-left-radius: 12px;
    border-top-right-radius: 12px;
    cursor: move;
  }
  .lens-bar-bottom {
    bottom: 0;
    padding: 6px 12px;
    font-size: 10.5px;
    color: #9aa1ab;
    background: rgba(0, 0, 0, 0.55);
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    border-bottom-left-radius: 12px;
    border-bottom-right-radius: 12px;
  }
  .lens-bar-title {
    font-weight: 600;
    font-size: 11.5px;
    letter-spacing: 0.02em;
  }
  .lens-bar-divider {
    width: 1px;
    height: 12px;
    background: rgba(255, 255, 255, 0.12);
  }
  .lens-bar-status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 10.5px;
    color: #a0a8b3;
  }
  .lens-bar-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #888;
  }
  .lens-bar-dot.running {
    background: #7dd3a0;
    box-shadow: 0 0 0 3px rgba(125, 211, 160, 0.18);
    animation: lens-pulse 1.6s ease-in-out infinite;
  }
  @keyframes lens-pulse {
    0%, 100% { opacity: 0.7; transform: scale(1); }
    50%      { opacity: 1; transform: scale(1.15); }
  }
  .lens-bar-pair {
    margin-left: auto;
    font-family: 'Geist Mono', ui-monospace, monospace;
    font-size: 10px;
    color: #7a808a;
    letter-spacing: 0.04em;
  }
  .lens-bar-actions {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    -webkit-app-region: no-drag;
  }
  .lens-bar-mono {
    font-family: 'Geist Mono', ui-monospace, monospace;
  }
  .lens-bar-sep {
    color: #5a6068;
  }
  .lens-bar-spacer {
    flex: 1;
  }

  /* ============================================================
     Lens icon buttons in the bar
     ============================================================ */
  .lens-icon-btn {
    width: 22px;
    height: 22px;
    padding: 0;
    display: grid;
    place-items: center;
    border-radius: 5px;
    background: transparent;
    border: 0;
    color: #a0a8b3;
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease;
  }
  .lens-icon-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }
  .lens-icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .lens-icon-btn.active {
    background: rgba(168, 197, 218, 0.2);
    color: var(--accent, #a8c5da);
  }

  /* ============================================================
     Decorative overlays — scan line + corner brackets
     ============================================================ */
  .lens-scan {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 1.5px;
    background: linear-gradient(
      90deg,
      transparent,
      var(--accent, #a8c5da),
      transparent
    );
    box-shadow:
      0 0 16px rgba(168, 197, 218, 0.6),
      0 0 24px rgba(168, 197, 218, 0.4);
    pointer-events: none;
    z-index: 20;
    animation: lens-scan 2.4s linear infinite;
  }
  @keyframes lens-scan {
    0%   { transform: translateY(0); opacity: 0; }
    10%  { opacity: 1; }
    90%  { opacity: 1; }
    100% { transform: translateY(100vh); opacity: 0; }
  }
  .lens-bracket {
    position: absolute;
    width: 14px;
    height: 14px;
    pointer-events: none;
    z-index: 25;
    opacity: 0;
    transition: opacity 160ms ease;
  }
  .lens-shell.lens-hovered .lens-bracket {
    opacity: 0.85;
  }
  .lens-bracket-tl {
    left: 8px;
    top: 38px;
    border-left: 1.5px solid var(--accent, #a8c5da);
    border-top: 1.5px solid var(--accent, #a8c5da);
    border-top-left-radius: 4px;
  }
  .lens-bracket-tr {
    right: 8px;
    top: 38px;
    border-right: 1.5px solid var(--accent, #a8c5da);
    border-top: 1.5px solid var(--accent, #a8c5da);
    border-top-right-radius: 4px;
  }
  .lens-bracket-bl {
    left: 8px;
    bottom: 32px;
    border-left: 1.5px solid var(--accent, #a8c5da);
    border-bottom: 1.5px solid var(--accent, #a8c5da);
    border-bottom-left-radius: 4px;
  }
  .lens-bracket-br {
    right: 8px;
    bottom: 32px;
    border-right: 1.5px solid var(--accent, #a8c5da);
    border-bottom: 1.5px solid var(--accent, #a8c5da);
    border-bottom-right-radius: 4px;
  }

  /* ============================================================
     Empty state hint
     ============================================================ */
  .lens-empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
    text-align: center;
  }
  .lens-empty p {
    padding: 8px 14px;
    border-radius: 999px;
    background: rgba(13, 14, 16, 0.78);
    backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #c8cacd;
    font-size: 11.5px;
    margin: 0;
  }

  .lens-error {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 36px;
    z-index: 40;
    padding: 6px 10px;
    border-radius: 6px;
    background: rgba(216, 122, 122, 0.88);
    color: #fff;
    font-size: 11px;
    margin: 0;
  }

  /* ============================================================
     Translation cards
     ============================================================ */
  .lens-card {
    container-type: size;
    /* The card is a <button> so we strip the platform chrome and
       restore a transparent background — bg_color/bg_patch is
       carried in the inline `style` attribute we generate per-card. */
    border: 0;
    padding: 0;
    cursor: copy;
    color: inherit;
    text-align: inherit;
    font: inherit;
    /* Center the translation text vertically within the card box.
       Without this, short translations in tall cards hugged the
       top edge instead of sitting where the source glyphs would
       have been. */
    display: grid;
    place-content: center stretch;
    /* Fade + slight scale entry so newly committed cards don't
       pop in jarringly. The transition is interruptible so
       successive flow updates don't queue stale animations. */
    animation: lens-card-in 220ms cubic-bezier(0.22, 0.61, 0.36, 1) both;
    transition:
      transform 120ms ease,
      box-shadow 120ms ease;
  }
  .lens-card:focus-visible {
    outline: 2px solid var(--accent, #a8c5da);
    outline-offset: 1px;
  }
  .lens-card:hover {
    box-shadow: 0 0 0 1.5px rgba(168, 197, 218, 0.45);
  }
  /* `.lens-card-copied` is toggled via JS (classList.add) so
     Svelte's scoped CSS rewriter doesn't see it being used in the
     template. The `:global()` wrapper opts the rule out of
     scoping so the class still applies when JS sets it. */
  :global(.lens-card.lens-card-copied) {
    box-shadow: 0 0 0 2px rgba(125, 211, 160, 0.85);
    transform: scale(0.985);
  }
  .lens-card-translating {
    /* While waiting on the translation chunk we mute the card
       slightly so the user can tell it's still in-flight without
       the empty card hiding the original glyphs entirely. */
    opacity: 0.92;
  }
  .lens-card-pending {
    pointer-events: none;
    opacity: 0.6;
  }
  @keyframes lens-card-in {
    from {
      opacity: 0;
      transform: scale(0.985);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  /* Subtle shimmer overlay shown while a card is still streaming
     its translation. Sized to the card itself; the animation is
     CPU-cheap (background-position only). */
  .lens-card-shimmer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: linear-gradient(
      90deg,
      transparent 0%,
      rgba(255, 255, 255, 0.12) 50%,
      transparent 100%
    );
    background-size: 200% 100%;
    animation: lens-card-shimmer 1.4s linear infinite;
  }
  @keyframes lens-card-shimmer {
    from { background-position: -150% 0; }
    to   { background-position: 150% 0; }
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
    margin: 0;
    padding: 0 1px;
    box-sizing: border-box;
    overflow: hidden;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    word-break: break-word;
    /* Multi-script font stack: Latin glyphs come from Geist (matches
       the rest of the app's typography), Japanese from Noto Sans
       JP, Korean from Noto Sans KR. Listing CJK families AFTER
       Geist means Latin codepoints use Geist while CJK codepoints
       fall through to the script-appropriate face — the per-script
       fall-through is what makes mixed-script lines render with
       consistent metric. `system-ui` is the last-resort fallback
       for any glyphs not covered by the Noto families. */
    font-family:
      'Geist',
      'Noto Sans JP',
      'Noto Sans KR',
      'Yu Gothic UI',
      'Hiragino Sans',
      'Malgun Gothic',
      system-ui,
      -apple-system,
      sans-serif;
    font-synthesis: none;
    /* JP `palt` enables proportional alternates so full-width
       punctuation no longer leaves a half-em gap after periods /
       commas. `kern` enables Latin pair kerning so the Geist /
       Noto handoff isn't visibly loose. */
    font-feature-settings: 'palt', 'kern';
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
    /* Better anti-aliasing across Win/macOS — the default font
       smoothing on Windows WebView2 makes Geist look heavy. */
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-rendering: geometricPrecision;
  }
</style>
