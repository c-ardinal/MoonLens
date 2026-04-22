<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { X } from 'lucide-svelte';
  import { cancelRegionSelection, hideOverlay } from '$lib/ipc';

  // Drag-to-select implemented entirely in the renderer; backend receives
  // only the resulting rect via `capture_and_process`. The selector window
  // covers the full primary monitor and is otherwise transparent.

  let dragging = $state(false);
  let startX = $state(0);
  let startY = $state(0);
  let curX = $state(0);
  let curY = $state(0);
  let dpr = $state(1);
  let busy = $state(false);

  onMount(() => {
    dpr = window.devicePixelRatio || 1;
  });

  function rect() {
    const x = Math.min(startX, curX);
    const y = Math.min(startY, curY);
    const w = Math.abs(curX - startX);
    const h = Math.abs(curY - startY);
    return { x, y, w, h };
  }

  async function dismiss() {
    // Backend path: always works even if the webview's own
    // `window.hide()` is denied a permission or the show/hide
    // transition swallowed the call. The Rust `cancel_region_selection`
    // command ultimately calls the same `WebviewWindow::hide()` but
    // inside the privileged Tauri runtime.
    busy = true;
    try {
      await hideOverlay().catch(() => {});
      await cancelRegionSelection().catch(() => {});
    } finally {
      busy = false;
    }
  }

  function pointerdown(e: PointerEvent) {
    if (busy) return;
    // Right-click / middle-click = immediate cancel.
    if (e.button !== 0) {
      e.preventDefault();
      dismiss();
      return;
    }
    dragging = true;
    startX = e.clientX;
    startY = e.clientY;
    curX = e.clientX;
    curY = e.clientY;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }
  function pointermove(e: PointerEvent) {
    if (!dragging) return;
    curX = e.clientX;
    curY = e.clientY;
  }
  async function pointerup(_e: PointerEvent) {
    if (!dragging || busy) return;
    dragging = false;
    const r = rect();
    if (r.w < 8 || r.h < 8) {
      // Tiny drags count as cancel — clicks without real drag
      // intent dismiss the selector instead of capturing a 0×0 area.
      await dismiss();
      return;
    }
    busy = true;
    try {
      const region = {
        monitor_index: 0,
        x: Math.round(r.x * dpr),
        y: Math.round(r.y * dpr),
        w: Math.round(r.w * dpr),
        h: Math.round(r.h * dpr)
      };
      // Hide first so the captured frame does not include our overlay.
      await cancelRegionSelection().catch(() => {});
      await invoke('capture_and_process', { args: region });
    } finally {
      busy = false;
    }
  }

  async function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      await dismiss();
    }
  }

  // Context menu would otherwise show the browser right-click menu.
  function onContextMenu(e: Event) {
    e.preventDefault();
  }

  $effect(() => {
    window.addEventListener('keydown', onKey, { capture: true });
    window.addEventListener('contextmenu', onContextMenu);
    return () => {
      window.removeEventListener('keydown', onKey, { capture: true });
      window.removeEventListener('contextmenu', onContextMenu);
    };
  });
</script>

<div
  role="application"
  aria-label="領域を選択"
  class="fixed inset-0 cursor-crosshair select-none"
  style="background: rgba(0, 0, 0, 0.18)"
  onpointerdown={pointerdown}
  onpointermove={pointermove}
  onpointerup={pointerup}
>
  {#if dragging}
    {@const r = rect()}
    <div class="absolute inset-x-0 top-0 bg-black/60" style="height: {r.y}px"></div>
    <div
      class="absolute left-0 bg-black/60"
      style="top: {r.y}px; width: {r.x}px; height: {r.h}px"
    ></div>
    <div
      class="absolute right-0 bg-black/60"
      style="top: {r.y}px; width: calc(100% - {r.x + r.w}px); height: {r.h}px"
    ></div>
    <div
      class="absolute inset-x-0 bottom-0 bg-black/60"
      style="height: calc(100% - {r.y + r.h}px)"
    ></div>

    <div
      class="pointer-events-none absolute border-2 border-accent shadow-[0_0_0_1px_rgba(255,255,255,0.4)]"
      style="left: {r.x}px; top: {r.y}px; width: {r.w}px; height: {r.h}px"
    ></div>
    <div
      class="pointer-events-none absolute rounded-md bg-black/80 px-2 py-0.5 text-xs font-mono text-white"
      style="left: {r.x}px; top: {Math.max(0, r.y - 22)}px"
    >
      {Math.round(r.w)} × {Math.round(r.h)}
    </div>
  {:else}
    <div
      class="pointer-events-none absolute inset-x-0 top-6 mx-auto w-fit rounded-full bg-black/80 px-4 py-1.5 text-sm text-white shadow-ml"
    >
      ドラッグで領域選択 · ESC / 右クリックでキャンセル
    </div>
  {/if}

  <!--
    Explicit Cancel button — always visible in the top-right corner.
    Separate z-layer and pointer-events-auto so it receives the click
    even while a drag is technically in progress (the drag pointer
    capture keeps the first pointerdown on the background; clicks on
    this button are a fresh pointer sequence).
  -->
  <button
    type="button"
    class="absolute right-6 top-6 z-20 inline-flex items-center gap-1.5 rounded-full border border-white/30 bg-black/70 px-3 py-1.5 text-xs font-medium text-white shadow-ml backdrop-blur transition hover:bg-red-500/80 hover:border-red-500"
    onclick={dismiss}
    onpointerdown={(e) => e.stopPropagation()}
    aria-label="キャンセル"
  >
    <X class="h-3.5 w-3.5" aria-hidden="true" />
    キャンセル (Esc)
  </button>
</div>

<style>
  /* `!important` required: `app.css` imported by the shared layout
     sets `body { background: var(--ml-surface) }` (dark-theme black)
     which would otherwise paint the entire transparent selector
     window opaque black. The lens page has the same guard. */
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
    padding: 0;
    overflow: hidden;
  }
</style>
