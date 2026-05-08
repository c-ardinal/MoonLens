<script lang="ts">
  /**
   * Global listener for `moonlens://hotkey-failure`.
   *
   * Backend now deduplicates the event so the toast only fires when
   * the failure set changes between saves. This component still
   * applies a few additional UX guards on top:
   *
   *   - Empty / whitespace-only accelerators (the user cleared the
   *     hotkey on purpose) never produce a toast even if the backend
   *     somehow reports them as failures.
   *   - An explicit "all clear" event from the backend (empty failure
   *     list after a non-empty one) auto-dismisses the visible toast.
   *   - Auto-dismiss fires after 8s so unrelated saves don't keep the
   *     popup hanging.
   */
  import { onDestroy, onMount } from 'svelte';
  import { AlertTriangle, X, KeyRound } from 'lucide-svelte';
  import { onHotkeyFailure, type HotkeyFailure } from '$lib/ipc';
  import { goto } from '$lib/navigation';

  let failures = $state<HotkeyFailure[]>([]);
  let visible = $state(false);
  let dismissTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    let unlistenFn: (() => void) | null = null;
    const setup = async () => {
      unlistenFn = await onHotkeyFailure((report) => {
        const meaningful = (report.failed ?? []).filter(
          (f) => f.accelerator && f.accelerator.trim().length > 0
        );
        if (meaningful.length === 0) {
          // Either the backend signalled "all clear" or only empty-
          // hotkey noise came through. In both cases the user has
          // resolved whatever caused the previous toast.
          visible = false;
          failures = [];
          if (dismissTimer) clearTimeout(dismissTimer);
          return;
        }
        failures = meaningful;
        visible = true;
        if (dismissTimer) clearTimeout(dismissTimer);
        dismissTimer = setTimeout(() => {
          visible = false;
        }, 8000);
      });
    };
    void setup();
    return () => {
      unlistenFn?.();
    };
  });

  onDestroy(() => {
    if (dismissTimer) clearTimeout(dismissTimer);
  });

  function dismiss() {
    visible = false;
    if (dismissTimer) clearTimeout(dismissTimer);
  }

  function rebind() {
    dismiss();
    goto('/settings/');
  }

  const labels: Record<string, string> = {
    capture_region: '範囲キャプチャ',
    capture_window: 'ウィンドウキャプチャ',
    toggle_pin_overlay: 'オーバーレイをピン留め',
    toggle_subtitle: '音声字幕の開始/停止',
    open_command_palette: 'コマンドパレット',
    open_history: '履歴を開く',
    reverse_image_search: '画像で検索',
    stop_all: '全停止'
  };
  function prettyId(id: string): string {
    return labels[id] ?? id;
  }
</script>

{#if visible && failures.length > 0}
  <div
    role="alert"
    aria-live="polite"
    class="hotkey-toast"
  >
    <div class="toast-icon">
      <AlertTriangle class="h-4 w-4" aria-hidden="true" />
    </div>
    <div class="toast-body">
      <p class="toast-title">ホットキーが登録できません</p>
      <p class="toast-sub">
        他のアプリ・IME・OS のショートカットが同じキーを使用している可能性があります。
      </p>
      <ul class="toast-list">
        {#each failures as failure (failure.id)}
          <li>
            <span class="failure-id">{prettyId(failure.id)}</span>
            <code class="mono failure-accel">{failure.accelerator}</code>
          </li>
        {/each}
      </ul>
      <div class="toast-actions">
        <button type="button" class="btn primary toast-btn" onclick={rebind}>
          <KeyRound class="h-3.5 w-3.5" aria-hidden="true" />
          設定で変更
        </button>
        <button type="button" class="btn ghost toast-btn" onclick={dismiss}>
          閉じる
        </button>
      </div>
    </div>
    <button
      type="button"
      class="toast-close"
      aria-label="閉じる"
      onclick={dismiss}
    >
      <X class="h-3.5 w-3.5" aria-hidden="true" />
    </button>
  </div>
{/if}

<style>
  .hotkey-toast {
    position: fixed;
    right: 16px;
    top: 50px;
    z-index: 70;
    width: min(380px, calc(100vw - 32px));
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 12px;
    padding: 14px 14px 12px;
    border-radius: var(--r-md);
    background: var(--bg-1);
    border: 1px solid rgba(216, 122, 122, 0.35);
    box-shadow: var(--shadow-lg);
    animation: ml-fadeup var(--dur-base) var(--ease-out) both;
  }
  .toast-icon {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: rgba(224, 182, 110, 0.18);
    color: #c69655;
    display: grid;
    place-items: center;
    flex: 0 0 28px;
  }
  .toast-body {
    min-width: 0;
  }
  .toast-title {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-0);
  }
  .toast-sub {
    margin: 4px 0 8px;
    font-size: 11.5px;
    color: var(--ink-2);
    line-height: 1.5;
  }
  .toast-list {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .toast-list li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
  }
  .failure-id {
    color: var(--ink-1);
    font-weight: 500;
    flex: 0 0 auto;
  }
  .failure-accel {
    color: var(--ink-2);
    font-size: 10.5px;
    background: var(--bg-2);
    padding: 1px 6px;
    border-radius: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .toast-actions {
    display: flex;
    gap: 6px;
  }
  .toast-btn {
    padding: 5px 10px;
    font-size: 11.5px;
  }
  .toast-close {
    width: 24px;
    height: 24px;
    border-radius: var(--r-sm);
    background: transparent;
    border: 0;
    color: var(--ink-3);
    cursor: pointer;
    display: grid;
    place-items: center;
    flex: 0 0 24px;
    align-self: flex-start;
  }
  .toast-close:hover {
    background: var(--bg-2);
    color: var(--ink-0);
  }
</style>
