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
          bg_color: l.bg_color,
          fg_color: l.fg_color,
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
    <div class="lens-card absolute flex items-center overflow-hidden" style={cardStyle(line)}>
      <p
        class="line-text w-full overflow-hidden whitespace-nowrap"
        style="--text-len: {Math.max(1, line.translation.length)}"
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
  .lens-card {
    container-type: size;
    line-height: 1;
  }
  .line-text {
    /* MVP font-fit: tighter of bbox-height-based sizing (85cqh) and
       character-width-based sizing (90cqw / text-len). Max 40px to
       accommodate large source text (subtitles, big UI labels).
       `--text-len` is set per card from Svelte. */
    font-size: clamp(
      7px,
      min(85cqh, calc(90cqw / var(--text-len, 10))),
      40px
    );
    line-height: 1;
  }
</style>
