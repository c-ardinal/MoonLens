<script lang="ts">
  /**
   * Custom window chrome for Tauri windows with `decorations: false`.
   *
   * Provides the only way to move/minimize/close the main window. The
   * top 32px strip is the drag region (data-tauri-drag-region); the
   * trailing icons are explicit buttons with their own hit targets.
   */
  import { Minus, X } from 'lucide-svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  interface Props {
    /** Optional title text shown at left. Defaults to "MoonLens". */
    title?: string;
    /** Hide min/close when shown on lens/overlay frames. */
    actionsVisible?: boolean;
  }

  let { title = 'MoonLens', actionsVisible = true }: Props = $props();

  const hasTauri = () =>
    typeof window !== 'undefined' &&
    typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !==
      'undefined';

  async function minimize() {
    if (!hasTauri()) return;
    try {
      await getCurrentWindow().minimize();
    } catch {
      /* best-effort: fall through silently when IPC is unavailable */
    }
  }

  async function close() {
    if (!hasTauri()) return;
    try {
      await getCurrentWindow().close();
    } catch {
      /* best-effort */
    }
  }
</script>

<div
  class="window-chrome flex h-8 select-none items-center justify-between gap-3 border-b border-border bg-surface/80 px-3 backdrop-blur"
  data-tauri-drag-region
>
  <div
    class="pointer-events-none flex items-center gap-2 text-[11px] font-medium tracking-wide text-ink-muted"
    data-tauri-drag-region
  >
    <span
      class="inline-block h-2 w-2 rounded-full bg-accent"
      aria-hidden="true"
      data-tauri-drag-region
    ></span>
    <span data-tauri-drag-region>{title}</span>
  </div>

  {#if actionsVisible}
    <div class="flex items-center gap-1">
      <button
        type="button"
        onclick={minimize}
        aria-label="最小化"
        title="最小化"
        class="inline-flex h-6 w-6 items-center justify-center rounded-ml-sm text-ink-faint transition-colors hover:bg-surface-muted hover:text-ink"
      >
        <Minus class="h-3.5 w-3.5" aria-hidden="true" />
      </button>
      <button
        type="button"
        onclick={close}
        aria-label="閉じる"
        title="閉じる"
        class="close-btn inline-flex h-6 w-6 items-center justify-center rounded-ml-sm text-ink-faint transition-colors hover:bg-red-500 hover:text-white"
      >
        <X class="h-3.5 w-3.5" aria-hidden="true" />
      </button>
    </div>
  {/if}
</div>

<style>
  /* Double-click the drag strip to maximize on Windows feels native;
     Tauri already handles this via data-tauri-drag-region. */
  .window-chrome {
    flex: 0 0 auto;
  }
</style>
