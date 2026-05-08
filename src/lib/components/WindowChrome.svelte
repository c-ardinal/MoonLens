<script lang="ts">
  /**
   * Custom window chrome for Tauri windows with `decorations: false`.
   *
   * Lays out three regions:
   *   • left  — three traffic-light buttons (close / minimise / zoom)
   *   • mid   — brand mark + product name (drag region)
   *   • right — quick actions (capture / lens / notifications)
   *
   * Only the close / minimise / zoom hits actually invoke Tauri APIs;
   * the right-side icons emit events the page above can listen for so
   * the chrome stays presentation-only here.
   */
  import MoonLensMark from './MoonLensMark.svelte';
  import { Camera, Focus } from 'lucide-svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { goto } from '$lib/navigation';
  import { newLensWindow } from '$lib/ipc';

  interface Props {
    title?: string;
    actionsVisible?: boolean;
  }

  let { title = 'MoonLens', actionsVisible = true }: Props = $props();

  const hasTauri = () =>
    typeof window !== 'undefined' &&
    typeof (window as unknown as { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__ !== 'undefined';

  async function close() {
    if (!hasTauri()) return;
    try {
      await getCurrentWindow().close();
    } catch {
      /* best-effort */
    }
  }

  async function minimize() {
    if (!hasTauri()) return;
    try {
      await getCurrentWindow().minimize();
    } catch {
      /* best-effort */
    }
  }

  async function toggleMaximize() {
    if (!hasTauri()) return;
    try {
      await getCurrentWindow().toggleMaximize();
    } catch {
      /* best-effort */
    }
  }

  function quickCapture() {
    goto('/capture/');
  }

  async function quickLens() {
    try {
      await newLensWindow();
    } catch {
      /* swallow — surfacing this requires the page-level error toast */
    }
  }
</script>

<div class="ml-titlebar" data-tauri-drag-region>
  <div class="ml-tb-traffic" aria-label="Window controls">
    <button
      type="button"
      class="traffic close"
      title="閉じる"
      aria-label="閉じる"
      onclick={close}
    ></button>
    <button
      type="button"
      class="traffic min"
      title="最小化"
      aria-label="最小化"
      onclick={minimize}
    ></button>
    <button
      type="button"
      class="traffic max"
      title="最大化"
      aria-label="最大化"
      onclick={toggleMaximize}
    ></button>
  </div>

  <div class="ml-tb-title" data-tauri-drag-region>
    <MoonLensMark size={13} />
    <span data-tauri-drag-region>{title}</span>
  </div>

  <div class="ml-tb-spacer" data-tauri-drag-region></div>

  {#if actionsVisible}
    <div class="ml-tb-actions">
      <button
        type="button"
        class="ml-tb-icon"
        title="クイックキャプチャ"
        aria-label="クイックキャプチャ"
        onclick={quickCapture}
      >
        <Camera class="h-3.5 w-3.5" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="ml-tb-icon"
        title="Lens起動"
        aria-label="Lens起動"
        onclick={quickLens}
      >
        <Focus class="h-3.5 w-3.5" aria-hidden="true" />
      </button>
    </div>
  {/if}
</div>

<style>
  .ml-titlebar {
    display: flex;
    align-items: center;
    height: 36px;
    padding: 0 var(--s-3);
    gap: var(--s-3);
    background: var(--bg-1);
    border-bottom: 1px solid var(--line);
    -webkit-app-region: drag;
    user-select: none;
    font-size: 12px;
    color: var(--ink-2);
    letter-spacing: 0.02em;
    flex: 0 0 auto;
  }

  .ml-tb-traffic {
    display: flex;
    gap: 6px;
    -webkit-app-region: no-drag;
  }

  .traffic {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: var(--bg-3);
    border: 0;
    padding: 0;
    cursor: pointer;
    transition: background var(--dur-fast);
  }
  .traffic:hover {
    background: var(--ink-3);
  }
  .traffic.close:hover {
    background: #ed6a5e;
  }
  .traffic.min:hover {
    background: #f5bd4f;
  }
  .traffic.max:hover {
    background: #61c454;
  }

  .ml-tb-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
    color: var(--ink-1);
  }

  .ml-tb-spacer {
    flex: 1;
  }

  .ml-tb-actions {
    display: flex;
    gap: 2px;
    -webkit-app-region: no-drag;
  }

  .ml-tb-icon {
    width: 26px;
    height: 24px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    cursor: pointer;
    color: var(--ink-2);
    background: transparent;
    border: 0;
    transition:
      background var(--dur-fast),
      color var(--dur-fast);
  }
  .ml-tb-icon:hover {
    background: var(--bg-3);
    color: var(--ink-0);
  }
</style>
