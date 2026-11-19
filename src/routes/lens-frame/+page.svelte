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
    onEmbeddedModelLoading,
    onLensFlowDone,
    onLensFlowStart,
    onLensLineChunk,
    onLensMask,
    onLensOcrLines,
    type EmbeddedModelLoadingEvent,
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
  /** Line indices whose translation has RESOLVED for the current
   *  flow — either committed to a visible card OR determined to be
   *  a passthrough (source == translation) that intentionally
   *  renders no card. The pending-placeholder shimmer keys off this
   *  so a passthrough line stops shimmering instead of pulsing
   *  forever (it has no card to "replace" the placeholder). Reset
   *  per flow in onLensOcrLines. */
  let resolvedIndices = $state(new Set<number>());
  let imageW = $state(1);
  let imageH = $state(1);
  // Rendered height (CSS px) of the card-positioning shell. Bound via
  // `bind:clientHeight` on `<main class="lens-shell">`. Used to convert
  // the backend's capture-space `font_size_px` into an absolute CSS px
  // font size (`font_size_px × shellH / imageH`) — see `cardStyle`.
  let shellH = $state(0);
  let shellW = $state(0);
  // Shell (window inner) size FROZEN at the moment a flow's OCR lines
  // arrive. Cards are laid out in absolute px against these frozen
  // dims (see `cardStyle`) so resizing the window afterwards does NOT
  // rescale or move them — they stay pinned to the translated content.
  // A shrink that reaches a card clears it (see the resize handler).
  let capShellW = $state(0);
  let capShellH = $state(0);
  // Window top-left on the virtual screen, FROZEN at capture time vs.
  // the LIVE value. Resizing from the left/top edge moves the window
  // origin, so cards anchored to that origin would travel with it. We
  // offset every card by (frozen − live) origin so they hold their
  // SCREEN position regardless of which edge the user drags.
  let capScreenX = $state(0);
  let capScreenY = $state(0);
  // Element wrapping all cards. During a resize we translate THIS
  // layer imperatively (in an rAF, in sync with paint) to cancel the
  // window-origin drift, so cards hold their screen position WITHOUT
  // the ~1-frame lag a reactive per-card offset would introduce.
  let cardsLayer = $state<HTMLDivElement | null>(null);
  let resizeRaf: number | null = null;
  let resizeSettleTimer: number | null = null;
  // Last edge-anchor state applied to the cards layer ('L'=left/top,
  // 'R'=right/bottom, '' = not yet anchored). Used to make `anchorLayer`
  // idempotent so a resize burst doesn't rewrite identical CSS (and
  // thrash layout) on every event.
  let anchorH = '';
  let anchorV = '';
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

  // Embedded LLM load state, surfaced ON THE LENS window itself so
  // the user sees concrete progress during the multi-second
  // GGUF cold-start. Pre-fix the only signal was the
  // `EmbeddedModelLoadingBadge` in `/+layout.svelte` — which the
  // Lens frame DOES inherit, but the badge is `pointer-events: none`
  // + fixed top-right + the Lens chrome bars stay opacity:0 until
  // hovered, so the user effectively saw "frozen Lens that does not
  // respond to drag or hover" with no explanation. Surfacing the
  // state via a dedicated overlay (and forcing the chrome bars
  // visible while loading) is what closes the
  // "機能が消えた" perception gap reported on 2026-05-27.
  let modelLoading = $state(false);
  let modelLoadError = $state<string | null>(null);
  let modelLoadProvider = $state<string>('');
  let modelLoadDismissTimer: ReturnType<typeof setTimeout> | null = null;

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
    // Card geometry follows the SOURCE text region (`inpaint_bbox` = the
    // grey bg_patch), NOT the wider `bbox`. The backend widens `bbox` in
    // the alignment direction so centred/right text has slack, but that
    // slack let a long translation spill PAST the grey patch into the
    // transparent card area ("背景枠からはみ出している"). Sizing the card to
    // the inpaint region instead clamps the text to the grey: a longer
    // translation now wraps + shrinks (via the fit pass) to stay inside
    // the source footprint. bg_patch math below degenerates to 100%/0%
    // automatically when w==iw/h==ih, so the patch still fills the card.
    const geomBox =
      Array.isArray(line.inpaint_bbox) && line.inpaint_bbox.length === 4
        ? line.inpaint_bbox
        : line.bbox;
    const [x, y, w, h] = geomBox;
    if (imageW === 0 || imageH === 0) return 'display:none';
    const [br, bgc, bb] = line.bg_color;
    const [fr, fg, fb] = line.fg_color;
    let weight = line.font_weight || 400;
    // CJK density compensation (2026-06): Noto Sans JP renders ~one
    // step heavier than a thin Latin source at the same numeric weight,
    // so a REGULAR Japanese translation reads as bold against a light
    // English original ("本文は太字"). Drop non-bold CJK output to 300
    // (Light) so it visually matches the thin source. Bold (≥600) CJK
    // headings keep their weight, and Latin translations are untouched
    // (Latin 400 is already as light as the source).
    const renderText = line.translation && line.translation.length > 0 ? line.translation : line.text;
    // Hiragana/katakana, CJK ideographs, halfwidth katakana.
    const isCjk = /[぀-ヿ㐀-鿿ｦ-ﾟ]/.test(renderText);
    if (isCjk && weight < 600) {
      weight = 300;
    }
    // Font size, in absolute CSS px (2026-06 rework — replaces the
    // earlier `cqh` approach).
    //
    // The backend's `font_size_px` is the source glyph height in the
    // CAPTURE coordinate space (image_w × image_h px). The shell that
    // hosts the cards renders that whole captured region at
    // `shellH` CSS px tall, so the capture→screen scale is exactly
    // `shellH / imageH`. The on-screen font size is therefore
    // `font_size_px × shellH / imageH` — one multiply, correct for any
    // DPR or OCR upscale (both numerator inputs live in capture space,
    // the ratio cancels the units).
    //
    // The previous `(font_size_px / h) × 100cqh` trick was meant to be
    // the same thing card-relative, but `container-type: size` on a
    // grid card resolved `cqh` against a REDUCED height for MULTI-LINE
    // cards (~0.66×), so paragraph cards rendered ~1/3 smaller than
    // their single-line neighbours. The explicit scale below sizes
    // every card identically regardless of line count.
    // Use the FROZEN shell size (`capShellH`) captured when this flow's
    // OCR lines arrived, not the live `shellH`, so the font does not
    // rescale when the user later resizes the window.
    const refH = capShellH > 0 ? capShellH : shellH;
    const refW = capShellW > 0 ? capShellW : shellW;
    let fontPx: number;
    if (typeof line.font_size_px === 'number' && line.font_size_px > 0) {
      const scale = refH > 0 && imageH > 0 ? refH / imageH : 1;
      fontPx = Math.max(7, Math.min(140, line.font_size_px * scale));
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
    // Absolute px geometry against the FROZEN shell size. Using px
    // (not %) is what pins the card to the captured content: a later
    // window resize changes the live shell but NOT `refW`/`refH`, so
    // the card keeps its exact on-screen rect (overflow:hidden on the
    // shell simply clips a card the shrunk window no longer covers,
    // and the resize handler removes it outright).
    const sx = refW > 0 && imageW > 0 ? refW / imageW : 1;
    const sy = refH > 0 && imageH > 0 ? refH / imageH : 1;
    // Frozen window-relative px. Origin drift from a left/top resize is
    // cancelled by translating the parent `.lens-cards-layer` (see the
    // resize handler), so individual cards never recompute on resize.
    return (
      `left:${(x * sx).toFixed(1)}px;` +
      `top:${(y * sy).toFixed(1)}px;` +
      `width:${(w * sx).toFixed(1)}px;` +
      `height:${(h * sy).toFixed(1)}px;` +
      `--font-px:${fontPx.toFixed(1)};` +
      // Absolute CSS px (`font_size_px × shellH / imageH`). `.line-text`
      // reads `--line-font-size`; the fit routine may shrink it further.
      `--line-font-size:${fontPx.toFixed(1)}px;` +
      `--fw:${weight};` +
      // Line-height as an ABSOLUTE px (source line pitch = ratio ×
      // base font px), NOT a unitless ratio. The fit routine shrinks
      // the font to keep the translation within the source's line
      // COUNT; with a ratio the pitch would shrink too and break the
      // "原文と同じ行間" the user set. Pinning it in px keeps the line
      // spacing at the source pitch no matter how far the font shrinks.
      `--lh:${(lineHeight * fontPx).toFixed(2)}px;` +
      `--ta:${textAlign};` +
      background +
      `color:rgb(${fr},${fg},${fb});` +
      textShadow
    );
  }

  /// Fit translation to its card. Strategy in order:
  ///   1. Measure: does the rendered text overflow horizontally OR
  ///      vertically? The 2026-05-28 lens_eval review surfaced
  ///      cards at 1.80× width overflow (`Quit the application.`
  ///      → `アプリケーションを終了してください。`) where the
  ///      previous height-only check passed but the card visibly
  ///      truncated the translation. Both axes need a measurement
  ///      pass.
  ///   2. If yes, reduce font-size proportionally to the WORST-
  ///      case axis (down to 60%, was 70%).
  ///   3. If still overflowing, fall back to horizontal scaleX
  ///      compression (down to 55%) — stretching is uglier than
  ///      shrinking so it's a last-resort guard, not the default.
  ///   4. If still overflowing after scaleX, clip with
  ///      `text-overflow: ellipsis` so the card never displays
  ///      partial-glyph noise. The browser draws "…" at the
  ///      cut-off and stops.
  ///
  /// `.line-text` carries `white-space: pre-wrap` so multi-line
  /// translations naturally wrap; the width measurement uses the
  /// widest WRAPPED line via `scrollWidth`, not the natural single-
  /// line width.
  ///
  /// A 5% tolerance on each axis shields against Chromium's
  /// integer-pixel rounding so already-fitting single-line cards
  /// aren't pointlessly shrunk.
  function applySourceLineFit(): void {
    if (typeof document === 'undefined') return;
    const els = document.querySelectorAll<HTMLElement>('.line-text');
    for (const el of Array.from(els)) {
      // The card (`.lens-card`) is the box we must fit INTO. The
      // `.line-text` <p> is grid-centered with auto height, so its
      // own clientHeight tracks content, not the card — measuring
      // overflow against it would always read ~1.0 vertically. Use
      // the card's clientHeight/clientWidth as the real bounds.
      const card = el.closest('.lens-card') as HTMLElement | null;
      if (!card) continue;

      // Reset prior fit state so we measure at the natural size first.
      el.style.transform = '';
      el.style.width = '';
      el.style.textOverflow = '';
      el.style.fontSize = '';
      el.style.lineHeight = '';

      const cardH = Math.max(1, card.clientHeight);
      const cardW = Math.max(1, card.clientWidth);

      // Pin the line-height so exactly N source rows span the card
      // height. The card now IS the source inpaint region, which only
      // covers line1-top → lineN-bottom — that is `(N-1)·pitch +
      // glyph_height`, NOT N full CSS line-boxes. Rendering N rows at the
      // source pitch (the absolute `--lh`) therefore overran the box by
      // roughly one row's leading, spilling line 2+ below the grey
      // ("本文の2行目以降がはみだす"). `cardH / N` makes the N rows span the
      // box exactly: the vertical extent matches the source while the
      // per-row pitch lands a hair tighter — visually the same 行間 and,
      // crucially, never overflowing.
      const srcLines = Math.max(1, parseInt(el.dataset.srcLines ?? '1', 10) || 1);
      el.style.lineHeight = `${(cardH / srcLines).toFixed(2)}px`;

      // Shrink the font until the wrapped translation occupies no more
      // than those N rows (and fits the width). With the line-height
      // pinned above, `scrollHeight / cardH` reduces to `wrappedRows / N`,
      // so the loop converges on "≤ N rows" — the source line count —
      // reflowing the wrap onto fewer lines as the glyphs shrink. Iterate
      // (the rows-vs-font relationship is non-linear) down to a 7px floor.
      let fontPx = parseFloat(getComputedStyle(el).fontSize) || 14;
      let guard = 0;
      while (guard < 16) {
        const oH = el.scrollHeight / cardH;
        const oW = el.scrollWidth / cardW;
        if (Math.max(oH, oW) <= 1.05) break;
        if (fontPx <= 7) break;
        fontPx = Math.max(7, fontPx * 0.9);
        el.style.fontSize = `${fontPx.toFixed(2)}px`;
        guard++;
      }

      // Residual horizontal overflow can only be a single token wider
      // than the card (a long URL / identifier that `break-word`
      // couldn't split). Compress it horizontally so it still fits the
      // row instead of clipping.
      const oW = el.scrollWidth / cardW;
      if (oW > 1.05) {
        const ratio = Math.max(0.55, 1 / oW);
        el.style.transformOrigin = 'top left';
        el.style.transform = `scaleX(${ratio.toFixed(3)})`;
        el.style.width = `${(100 / ratio).toFixed(2)}%`;
      }
      if (el.scrollWidth / cardW > 1.05 || el.scrollHeight / cardH > 1.05) {
        el.style.textOverflow = 'ellipsis';
      }
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
    // the dependency tracked by Svelte 5 runes.
    void visibleLines;
    if (typeof window === 'undefined') return;
    // Robust scheduling (2026-06 fix): a single `setTimeout(0)` fired
    // BEFORE the browser had resolved the card's container-query
    // `cqh` font-size, so `getComputedStyle(el).fontSize` and
    // `scrollHeight` were measured against an unsized (or 0-height
    // container) layout. The overflow check then read ≤1.05 and the
    // long translation was NEVER shrunk — it overflowed its
    // single-line source box and bled over the neighbouring card
    // (the "translation is a different size / sits away from the
    // source" report). Measure only after layout has actually
    // resolved the cqh units: a double rAF (guarantees one full
    // layout+paint), and again once the JP/KR webfonts finish
    // loading (glyph metrics change wrapping, so a pre-font measure
    // can be wrong too).
    let cancelled = false;
    const run = () => {
      if (!cancelled) applySourceLineFit();
    };
    requestAnimationFrame(() => requestAnimationFrame(run));
    if (document.fonts?.ready) {
      document.fonts.ready.then(run).catch(() => {});
    }
  });

  function commitLine(lineIndex: number): void {
    const src = pendingLines.find((l) => l.line_index === lineIndex);
    if (!src) return;
    const chunk = pendingChunks.get(lineIndex);
    const raw = (chunk?.text ?? '').trim();

    // 2026-05-28 "翻訳されない card" fix: the backend forwards the
    // SOURCE text verbatim when it deliberately skips translation
    // (is_translatable_segment rejected an OCR fragment), when the
    // translator falls back on error, or when it returns empty.
    // Re-rendering that source on an inpainted patch — in a font /
    // weight / alignment we only approximate — looks broken: the
    // user sees their own untranslated text redrawn slightly wrong.
    // Detect the passthrough (translation == source) and DON'T
    // create a card: with no card, the original source pixels show
    // through untouched, which is strictly better than a wrong
    // re-render. Any existing card for this slot is removed.
    // Mark the line resolved so its pending-placeholder shimmer
    // stops, whether or not we end up rendering a card.
    if (!resolvedIndices.has(lineIndex)) {
      resolvedIndices = new Set(resolvedIndices).add(lineIndex);
    }

    const isPassthrough = raw.length === 0 || raw === src.text.trim();
    if (isPassthrough) {
      const idx = visibleLines.findIndex((l) => l.line_index === lineIndex);
      if (idx >= 0) {
        visibleLines = visibleLines.filter((l) => l.line_index !== lineIndex);
      }
      return;
    }

    const committed: LineCard = {
      ...src,
      translation: raw,
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
    // Resize policy: cards are pinned to the captured content in
    // absolute px, so a resize never moves or rescales them. The only
    // effect a resize has is that SHRINKING the window past a card's
    // own rectangle removes that card (the window edge has "reached"
    // it). Growing the window leaves every card exactly where it was.
    if (typeof window !== 'undefined') {
      // Keep the cards pinned to their original SCREEN position while the
      // window is resized — WITHOUT any per-frame JS. The previous
      // approach translated the layer each rAF by `capScreenX -
      // window.screenX`; that value lagged the live window motion by a
      // frame, so the cards shivered ("小刻みに震える").
      //
      // CSS edge-anchoring instead: give the layer a FROZEN size
      // (capShellW × capShellH — the captured footprint) and anchor it to
      // whichever window edge is NOT being dragged. Dragging the LEFT or
      // TOP edge leaves the RIGHT / BOTTOM edge fixed in screen space, so
      // anchoring there (`right:0` / `bottom:0`) keeps every card screen-
      // stationary with zero JS in the paint path — no laggy value, no
      // jitter. We only need the drag DIRECTION (a sign, robust to the
      // same read lag), not a precise origin. When the window shrinks past
      // a card the frozen-size layer overflows the shell and the card is
      // clipped (then pruned at settle) — the "縮小で接触したらクリア" rule.
      const anchorLayer = () => {
        if (!cardsLayer || !(capShellW > 0 && capShellH > 0)) return;
        const s = cardsLayer.style;
        // Frozen footprint (set once; cheap to re-assert but guarded).
        if (s.width !== `${capShellW}px`) {
          s.transform = '';
          s.width = `${capShellW}px`;
          s.height = `${capShellH}px`;
        }
        // Left edge moved (origin shifted) ⇒ right edge is the fixed one.
        const h = window.screenX !== capScreenX ? 'R' : 'L';
        const v = window.screenY !== capScreenY ? 'R' : 'L';
        if (h !== anchorH) {
          anchorH = h;
          if (h === 'R') {
            s.left = 'auto';
            s.right = '0px';
          } else {
            s.left = '0px';
            s.right = 'auto';
          }
        }
        if (v !== anchorV) {
          anchorV = v;
          if (v === 'R') {
            s.top = 'auto';
            s.bottom = '0px';
          } else {
            s.top = '0px';
            s.bottom = 'auto';
          }
        }
      };
      // ONCE, after the drag settles (origin + size stable): remove any
      // card the resized window no longer fully contains. Doing this
      // only at settle — not every frame — avoids permanently deleting a
      // card that merely strayed out of bounds for a frame because the
      // `window.screenX` read lagged the live window motion mid-drag
      // (the "cards vanish while resizing" bug).
      const pruneOutOfBounds = () => {
        if (!(capShellW > 0 && capShellH > 0 && imageW > 0 && imageH > 0 && cardsLayer)) return;
        // The layer is now CSS edge-anchored at its FROZEN capture size,
        // so card coordinates live in the same capShell space as the live
        // window: a card is reachable by the shrink exactly when it falls
        // outside the current window rect. Compare card px against the
        // live shell size; the anchored edge is the fixed one, the
        // opposite edge is where the shrink bites.
        const sx = capShellW / imageW;
        const sy = capShellH / imageH;
        const liveW = shellW > 0 ? shellW : capShellW;
        const liveH = shellH > 0 ? shellH : capShellH;
        // When anchored right/bottom, the visible window covers the
        // [capShell - live, capShell] band; otherwise [0, live].
        const anchoredRight = window.screenX !== capScreenX;
        const anchoredBottom = window.screenY !== capScreenY;
        const minX = anchoredRight ? capShellW - liveW : 0;
        const maxX = anchoredRight ? capShellW : liveW;
        const minY = anchoredBottom ? capShellH - liveH : 0;
        const maxY = anchoredBottom ? capShellH : liveH;
        const stillInside = (l: LineCard): boolean => {
          const gb =
            Array.isArray(l.inpaint_bbox) && l.inpaint_bbox.length === 4 ? l.inpaint_bbox : l.bbox;
          const left = gb[0] * sx;
          const top = gb[1] * sy;
          const right = left + gb[2] * sx;
          const bottom = top + gb[3] * sy;
          return left >= minX - 2 && top >= minY - 2 && right <= maxX + 2 && bottom <= maxY + 2;
        };
        const pv = visibleLines.filter(stillInside);
        if (pv.length !== visibleLines.length) visibleLines = pv;
        const pp = pendingLines.filter(stillInside);
        if (pp.length !== pendingLines.length) pendingLines = pp;
      };
      const onResize = () => {
        // Re-anchor on every size change (cheap, idempotent CSS writes;
        // no rAF, no per-frame value to lag → no jitter).
        anchorLayer();
        // Prune once, shortly after the drag stops.
        if (resizeSettleTimer) clearTimeout(resizeSettleTimer);
        resizeSettleTimer = window.setTimeout(() => {
          resizeSettleTimer = null;
          anchorLayer(); // settle at the final, stable edge anchor
          pruneOutOfBounds(); // remove cards the shrink actually reached
        }, 160);
      };
      window.addEventListener('resize', onResize);
      unlisteners.push(() => window.removeEventListener('resize', onResize));
    }
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
        // Freeze the shell size AND the window's screen origin for THIS
        // flow's layout. Cards map their capture-space bbox into px
        // against these dims and offset by the origin drift, so they
        // stay pinned to content (and screen position) across resizes.
        capShellW = shellW > 0 ? shellW : capShellW;
        capShellH = shellH > 0 ? shellH : capShellH;
        if (typeof window !== 'undefined') {
          capScreenX = window.screenX;
          capScreenY = window.screenY;
        }
        // Fresh flow: the layout is rebuilt for the current window, so
        // drop any leftover resize anchoring (edge anchor + frozen size)
        // and fall back to the default CSS `inset:0` 100% layer.
        if (cardsLayer) {
          const s = cardsLayer.style;
          s.transform = '';
          s.width = '';
          s.height = '';
          s.left = '';
          s.right = '';
          s.top = '';
          s.bottom = '';
        }
        anchorH = '';
        anchorV = '';
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
        // 2026-05-28 stale-overlay fix: clear the ENTIRE visible set
        // when a new flow's OCR layout arrives. A new flow is only
        // emitted when the captured region's average-hash changed
        // past `HASH_DIFF_THRESHOLD` (lens_watcher.rs) — i.e. the
        // lens is now over DIFFERENT text — so every card from the
        // previous flow is stale by definition. The earlier
        // prune-by-`line_index` approach left flow-A's translation
        // on cards 0..n that flow-B happens to reuse (line indices
        // are per-flow 0,1,2… and collide across flows), which is
        // exactly the "過去にかざした文章の翻訳が次の文章に重なる"
        // bug. The pendingLines placeholders render a shimmer in the
        // new layout's positions until each translation lands via
        // commitLine, so the gap reads as "loading", not "blank".
        visibleLines = [];
        // Fresh flow → no line has resolved yet. (Stale entries would
        // otherwise suppress the new flow's placeholders.)
        resolvedIndices = new Set();
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

    // Embedded LLM load lifecycle. `started` surfaces a centred
    // overlay + freezes the chrome bars visible so the user can
    // see "loading" instead of "frozen". `finished` clears it
    // (success briefly flashes "準備完了", error sticks longer
    // so the user can read the message).
    unlisteners.push(
      await onEmbeddedModelLoading((e: EmbeddedModelLoadingEvent) => {
        if (modelLoadDismissTimer) {
          clearTimeout(modelLoadDismissTimer);
          modelLoadDismissTimer = null;
        }
        if (e.phase === 'started') {
          modelLoading = true;
          modelLoadError = null;
          modelLoadProvider = e.provider_id;
          return;
        }
        modelLoadProvider = e.provider_id;
        if (e.success) {
          modelLoadError = null;
          // Hold the badge briefly so the user can see "準備完了"
          // before it vanishes — masking the moment the load
          // finishes feels jarring after several seconds of
          // visible progress.
          modelLoadDismissTimer = setTimeout(() => {
            modelLoading = false;
          }, 800);
        } else {
          modelLoadError = e.error;
          modelLoadDismissTimer = setTimeout(() => {
            modelLoading = false;
            modelLoadError = null;
          }, 5000);
        }
      })
    );

    window.addEventListener('keydown', handleKey);
  });

  onDestroy(() => {
    for (const u of unlisteners) u();
    if (resizeSettleTimer) {
      clearTimeout(resizeSettleTimer);
      resizeSettleTimer = null;
    }
    if (resizeRaf !== null && typeof cancelAnimationFrame !== 'undefined') {
      cancelAnimationFrame(resizeRaf);
      resizeRaf = null;
    }
    if (typeof window !== 'undefined') {
      window.removeEventListener('keydown', handleKey);
    }
    if (modelLoadDismissTimer) {
      clearTimeout(modelLoadDismissTimer);
      modelLoadDismissTimer = null;
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
  bind:clientHeight={shellH}
  bind:clientWidth={shellW}
  class:opacity-0={masked}
  class:lens-hovered={hovered || modelLoading}
  class:lens-loading={modelLoading}
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

  <div class="lens-cards-layer" bind:this={cardsLayer}>
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
      <p
        class="line-text"
        data-line-index={line.line_index}
        data-fs-px={line.font_size_px ?? 0}
        data-src-h={line.source_bbox[3]}
        data-src-lines={line.source_line_count ?? 1}
      >
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
    {#if !visibleLines.some((v) => v.line_index === line.line_index) && !resolvedIndices.has(line.line_index)}
      <div
        class="lens-card lens-card-pending absolute overflow-hidden"
        style={cardStyle({ ...line, translation: '', done: false })}
        aria-hidden="true"
      >
        <span class="lens-card-shimmer"></span>
      </div>
    {/if}
  {/each}
  </div>

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

  {#if modelLoading}
    <!--
      Centred loading curtain shown while the embedded LLM is
      reading its GGUF off disk + warming up the llama.cpp
      context (several seconds on cold start). The curtain
      covers the whole shell so the user understands that the
      Lens is intentionally non-interactive right now, instead
      of treating the missing drag / hover response as a crash.
      Auto-dismisses on the `finished` lifecycle event.
    -->
    <div
      class="lens-loading-curtain"
      role="status"
      aria-live="polite"
      aria-busy={!modelLoadError}
    >
      <div class="lens-loading-card">
        {#if modelLoadError}
          <span class="lens-loading-icon-error" aria-hidden="true">!</span>
          <div class="lens-loading-text">
            <strong>ローカル LLM の読み込みに失敗</strong>
            <span class="lens-loading-subtle">{modelLoadError}</span>
          </div>
        {:else}
          <span class="lens-loading-spinner" aria-hidden="true"></span>
          <div class="lens-loading-text">
            <strong>ローカル LLM をロード中…</strong>
            {#if modelLoadProvider}
              <span class="lens-loading-subtle">{modelLoadProvider}</span>
            {/if}
          </div>
        {/if}
      </div>
    </div>
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

  /* Container for all cards. During a resize this layer is pinned to its
     captured footprint and CSS edge-anchored to the window edge that is
     NOT being dragged (see `anchorLayer`), so cards hold their screen
     position with zero per-frame JS.
     NOTE: deliberately NO `will-change: transform` / compositor hint.
     Promoting the layer to its own GPU texture made the browser re-place
     that texture from layout one frame behind the live window motion on a
     left/top-edge drag — the cards shivered ("小刻みに震える"). Painted in
     normal flow, the cards move atomically with the window instead.
     `pointer-events: none` keeps the transparent area click-through; the
     cards re-enable it themselves. */
  .lens-cards-layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .lens-cards-layer :global(.lens-card) {
    pointer-events: auto;
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
    /* Wrapping (2026-06):
       - `white-space: pre-wrap` keeps the source's line STRUCTURE so
         the card renders the same line COUNT at the same source-derived
         line-height (`--lh`) — switching to `normal` reflowed the text
         to a different number of lines and broke the "行間" the user had
         dialled in. Each source line still soft-wraps internally when a
         translation runs long, so growth is handled without forcing the
         break onto the source's exact byte positions.
       - `word-break: auto-phrase` breaks Japanese at PHRASE boundaries
         (using the browser's line-break dictionary) so katakana words
         like スナップエンドウ / プラスチック stay whole instead of being
         chopped mid-word the way `normal` splits between any two kana.
         It degrades to `normal` on engines without auto-phrase support,
         and leaves Latin word-breaking unchanged (only at spaces).
       - `line-break: strict` adds kinsoku (never start a line with 。、
         or a closing bracket).
       - `overflow-wrap: break-word` is the last-resort escape for a
         single token WIDER THAN THE CARD (long URL / identifier): it
         breaks only such over-wide tokens, never an ordinary word that
         would simply fit on the next line. */
    white-space: pre-wrap;
    word-break: auto-phrase;
    line-break: strict;
    overflow-wrap: break-word;
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
    /* `--line-font-size` is a card-relative `cqh` length on modern
       payloads (DPR/upscale-proof) and an absolute `px` on legacy
       ones. The `--font-px` calc is the last-resort default if the
       card emitted neither (should not happen in practice). */
    font-size: var(--line-font-size, calc(var(--font-px, 14) * 1px));
    font-weight: var(--fw, 400);
    /* Source-derived ratio; JS rewrites scaleX when the wrapped
       output would exceed `source_line_count` rows so the text
       stays the same size/weight but squeezes horizontally. */
    line-height: var(--lh, normal);
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

  /* ============================================================
     Embedded LLM loading curtain
     ============================================================ */
  .lens-loading-curtain {
    position: absolute;
    inset: 0;
    z-index: 40;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(13, 14, 16, 0.55);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    border-radius: 12px;
    pointer-events: none;
    animation: lens-loading-fade-in 160ms ease;
  }
  .lens-loading-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border-radius: 12px;
    background: rgba(20, 22, 26, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #e8eaee;
    font-size: 12.5px;
    max-width: 80%;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
  }
  .lens-loading-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .lens-loading-text strong {
    font-weight: 600;
  }
  .lens-loading-subtle {
    font-size: 10.5px;
    color: rgba(232, 234, 238, 0.6);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lens-loading-spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(168, 197, 218, 0.22);
    border-top-color: var(--accent, #a8c5da);
    border-radius: 50%;
    animation: lens-loading-spin 0.8s linear infinite;
    flex-shrink: 0;
  }
  .lens-loading-icon-error {
    width: 18px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: rgba(220, 70, 70, 0.25);
    color: #ff8080;
    font-weight: 700;
    font-size: 12px;
    flex-shrink: 0;
  }
  @keyframes lens-loading-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes lens-loading-fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
</style>
