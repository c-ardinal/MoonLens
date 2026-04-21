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

  let lines = $state<LineCard[]>([]);
  let imageW = $state(1);
  let imageH = $state(1);
  let currentFlowId = $state<number | null>(null);
  let streaming = $state(false);
  let hovered = $state(false);
  let lastError = $state<string | null>(null);
  // True while the watcher is capturing the screen. The window stays
  // visible but every card + chrome drops to opacity 0 so the screen
  // grab sees through the transparent window to the real source.
  let masked = $state(false);

  const unlisteners: UnlistenFn[] = [];
  const win = getCurrentWebviewWindow();

  /**
   * Force-start a native window drag. `data-tauri-drag-region` alone
   * is unreliable on Tauri 2 transparent windows; calling
   * `startDragging()` explicitly from mousedown works every time.
   * Fire-and-forget so the mouse-down event completes instantly.
   */
  function beginDrag(e: MouseEvent): void {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    // Don't steal clicks from buttons or cards.
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
    if (!lines.length) return;
    const text = lines.map((l) => l.translation || l.text).join('\n');
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

  onMount(async () => {
    unlisteners.push(
      await onLensFlowStart((e: LensFlowStart) => {
        if (e.id !== id) return;
        currentFlowId = e.flow_id;
        streaming = true;
        lastError = null;
      })
    );

    unlisteners.push(
      await onLensOcrLines((e: LensOcrLines) => {
        if (e.id !== id) return;
        if (currentFlowId !== null && e.flow_id !== currentFlowId) return;
        imageW = Math.max(1, e.image_w);
        imageH = Math.max(1, e.image_h);
        lines = e.lines.map((l) => ({
          line_index: l.line_index,
          text: l.text,
          bbox: l.bbox,
          bg_color: l.bg_color,
          fg_color: l.fg_color,
          translation: '',
          done: false
        }));
      })
    );

    unlisteners.push(
      await onLensLineChunk((e: LensLineChunk) => {
        if (e.id !== id) return;
        if (currentFlowId !== null && e.flow_id !== currentFlowId) return;
        const idx = lines.findIndex((l) => l.line_index === e.line_index);
        if (idx < 0) return;
        const next = [...lines];
        if (e.delta) {
          next[idx] = { ...next[idx], translation: next[idx].translation + e.delta };
        }
        if (e.done) {
          next[idx] = { ...next[idx], done: true };
        }
        lines = next;
      })
    );

    unlisteners.push(
      await onLensFlowDone((e: LensFlowDone) => {
        if (e.id !== id) return;
        if (currentFlowId !== null && e.flow_id !== currentFlowId) return;
        streaming = false;
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
  Lens UX per plan.md Q3 D:
  - main is FULLY transparent so the source text underneath shows
    through — no backdrop-blur, no panel background.
  - Each detected line renders as an opaque card whose fill is the
    sampled rim colour of its bbox, and whose text is the contrast
    colour picked from that bg. Google-Lens effect: translation
    "replaces" the source in place.
  - Header is hidden by default; cursor-over reveals drag handle +
    close button so the window remains interactive without a visible
    chrome during normal use.
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
  <!-- Hover-revealed header. Opacity transitions instead of display
       toggles so the drag region is stable when the cursor first
       enters the window. -->
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

  <!-- In-place translation cards: each covers one detected line with
       its sampled background colour, so the original text "becomes"
       the translation visually. -->
  {#each lines as line (line.line_index)}
    <div class="lens-card absolute flex items-center overflow-hidden" style={cardStyle(line)}>
      {#if line.translation}
        <p
          class="line-text w-full overflow-hidden whitespace-nowrap"
          style="--text-len: {Math.max(1, line.translation.length)}"
        >
          {line.translation}{#if !line.done}<span class="animate-pulse">▍</span>{/if}
        </p>
      {:else}
        <span class="line-text opacity-60" style="--text-len: 3">…</span>
      {/if}
    </div>
  {/each}

  <!-- Empty-state hint: only visible on hover so the window stays
       invisible when the source is rich enough to OCR. -->
  {#if lines.length === 0 && !streaming && hovered}
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
  /* app.css's `body { background: var(--ml-surface) }` rule ships the
     opaque surface colour to every route. For the lens window that is
     the enemy of glass — the window is transparent at the OS level
     but the webview fills with surface, so nothing behind it shows.
     Override with !important here so this route's body is truly see
     through while the other routes keep their normal chrome. */
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
    /* Pick the tighter of:
         - 85% of bbox height (fits source cap height)
         - 90% of bbox width divided by character count (fits horizontally)
       This prevents translations that are character-wise longer than
       the source (common for English → Japanese) from running off the
       sides of the card. `--text-len` is set per card from Svelte. */
    font-size: clamp(
      7px,
      min(85cqh, calc(90cqw / var(--text-len, 10))),
      40px
    );
    line-height: 1;
  }
</style>
