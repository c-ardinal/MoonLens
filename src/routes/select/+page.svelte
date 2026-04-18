<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { hideOverlay } from '$lib/ipc';

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

  const win = getCurrentWebviewWindow();

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

  function pointerdown(e: PointerEvent) {
    if (busy) return;
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
      // Treat tiny drags as a cancel.
      await win.hide();
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
      await win.hide();
      await invoke('capture_and_process', { args: region });
    } finally {
      busy = false;
    }
  }
  async function cancel(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      await hideOverlay();
      await win.hide();
    }
  }

  $effect(() => {
    window.addEventListener('keydown', cancel);
    return () => window.removeEventListener('keydown', cancel);
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
      class="pointer-events-none absolute inset-x-0 top-6 mx-auto w-fit rounded-full bg-black/60 px-4 py-1.5 text-sm text-white shadow-ml"
    >
      ドラッグで領域選択 · ESC でキャンセル
    </div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
    margin: 0;
    padding: 0;
    overflow: hidden;
  }
</style>
