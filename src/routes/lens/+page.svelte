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
    bg_color: [number, number, number];
    fg_color: [number, number, number];
    translation: string;
    done: boolean;
  }

  // Per-line commit: each line is published to `visibleLines` only
  // when its translation chunk carries `done=true`. The user never
  // sees intermediate tokens (no character-by-character typing) or
  // empty placeholders, and the previous flow's cards stay visible
  // until they are individually replaced by a newer line at the same
  // `line_index`. On `lens-flow-done`, any line that never produced
  // a `done=true` chunk falls back to its original OCR text so the
  // UI never stalls on a half-translated source.
  let visibleLines = $state<LineCard[]>([]);
  let pendingLines: LineCard[] = [];
  let pendingChunks = new Map<number, { text: string; done: boolean }>();
  let imageW = $state(1);
  let imageH = $state(1);
  let currentFlowId = $state<number | null>(null);
  let streaming = $state(false);
  let hovered = $state(false);
  let lastError = $state<string | null>(null);
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

  /** Bbox → CSS percentage position inside the lens body. */
  function cardStyle(line: LineCard): string {
    const [x, y, w, h] = line.bbox;
    if (imageW === 0 || imageH === 0) return 'display:none';
    const [br, bg, bb] = line.bg_color;
    const [fr, fg, fb] = line.fg_color;
    return (
      `left:${(x / imageW) * 100}%;` +
      `top:${(y / imageH) * 100}%;` +
      `width:${(w / imageW) * 100}%;` +
      `height:${(h / imageH) * 100}%;` +
      `background-color:rgb(${br},${bg},${bb});` +
      `color:rgb(${fr},${fg},${fb});`
    );
  }

  /** Commit a single line to `visibleLines`. Called when its
   *  translation chunk carries `done=true`. Replaces an existing
   *  card with the same `line_index` if any, otherwise appends. The
   *  previous flow's cards stay put until their slot is taken —
   *  avoiding the empty-state flash on every new capture. */
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

  /** Force remaining pending lines into `visibleLines` using whatever
   *  text has accumulated (or the original OCR text as fallback).
   *  Called on `lens-flow-done` to ensure the UI never stalls on a
   *  translation that silently failed or got truncated. Also prunes
   *  stale cards whose `line_index` is no longer in the new OCR
   *  layout. */
  function forceCommitRemaining(): void {
    // 1. Prune cards that don't exist in the new layout anymore.
    const validIndices = new Set(pendingLines.map((l) => l.line_index));
    const pruned = visibleLines.filter((l) => validIndices.has(l.line_index));
    if (pruned.length !== visibleLines.length) {
      visibleLines = pruned;
    }
    // 2. Commit any pending line that never got a done chunk.
    for (const l of pendingLines) {
      const already = visibleLines.some((v) => v.line_index === l.line_index);
      if (already) continue;
      commitLine(l.line_index);
    }
  }

  /** Split the translation on newlines (LLM often preserves the
   *  source's line structure) so we can size the font against the
   *  longest sub-line AND the total line count, not just the raw
   *  character length. Keeps multi-line paragraphs readable in a
   *  single card without overflowing. */
  function longestSubLine(text: string): number {
    return text
      .split(/\r?\n/)
      .reduce((acc, seg) => Math.max(acc, seg.length), 0) || text.length;
  }

  function subLineCount(text: string): number {
    return Math.max(1, text.split(/\r?\n/).length);
  }

  onMount(async () => {
    unlisteners.push(
      await onLensFlowStart((e: LensFlowStart) => {
        if (e.id !== id) return;
        currentFlowId = e.flow_id;
        streaming = true;
        lastError = null;
        // Reset the per-line buffer. `visibleLines` stays put: the
        // previous flow's cards remain on screen until their slots
        // are overwritten by the new flow's `done=true` chunks or
        // pruned by `forceCommitRemaining` on flow-done.
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
          bg_color: l.bg_color,
          fg_color: l.fg_color,
          translation: '',
          done: false
        }));
        // If OCR returned no lines, clear the old translations
        // immediately so a now-empty source frame doesn't keep stale
        // cards on screen.
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
        // Per-line commit: once this specific line is complete,
        // push it to visibleLines. Other lines can still be
        // streaming and the user sees this one pop in as a whole
        // block, no typing animation, no partial text.
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
        // Safety net: commit any line that never got a done chunk
        // and prune stale cards whose line_index is no longer in
        // the new layout.
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
  Lens UX per plan.md Q3 D (refined 2026-04-21):
  - main is FULLY transparent so source text shows through.
  - Cards render only after a flow is fully done (no per-chunk
    repaint, no blank placeholder) so the user sees one stable
    translation block at a time.
  - Header is hidden by default; hover reveals the drag handle and
    close button so the window stays interactive without chrome.
-->
<main
  class="relative h-screen w-screen overflow-hidden"
  class:masked
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

  <!-- Committed translation cards. Only updated atomically on
       flow-done so there is no character-by-character typing or
       empty-placeholder flash. -->
  {#each visibleLines as line (line.line_index)}
    <div class="lens-card absolute flex items-center overflow-hidden" style={cardStyle(line)}>
      <p
        class="line-text w-full overflow-hidden whitespace-pre-wrap break-words text-left"
        style="--text-len: {Math.max(1, longestSubLine(line.translation))}; --line-count: {subLineCount(line.translation)}"
      >
        {line.translation}
      </p>
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
  /* Mask is applied INSTANTLY — no transition-opacity here. The
     backend sleeps 60ms after emitting the mask signal; that budget
     has to cover Svelte's reactive update + browser repaint +
     compositor, not an animation. A visible fade would both (a) flash
     the user on every auto-poll tick and (b) leak half-opaque cards
     into the hash-compare capture, which the backend then mistakes
     for "source changed" and re-runs translation forever. */
  :global(main.masked) {
    opacity: 0;
  }
  .lens-card {
    container-type: size;
    line-height: 1.2;
  }
  .line-text {
    /* Size the font so the translation looks at home on top of the
       source, not like a billboard. Max is capped at 22px — real
       on-screen text is rarely bigger than that, so letting a short
       translation balloon to 40px made the card feel disconnected
       from the source layout. The vertical budget uses 75cqh (not
       90) because `line-height: 1.2` already eats 20% of the
       allotted height per line. */
    font-size: clamp(
      8px,
      min(
        calc(75cqh / var(--line-count, 1)),
        calc(95cqw / var(--text-len, 10))
      ),
      22px
    );
    line-height: 1.2;
    padding: 0 2px;
  }
</style>
