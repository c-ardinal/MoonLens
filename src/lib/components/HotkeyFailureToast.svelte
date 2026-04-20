<script lang="ts">
  /**
   * Global listener for `moonlens://hotkey-failure`.
   *
   * Startup and settings-save re-registrations surface the list of
   * shortcuts that could not be bound (another process holds them,
   * typically). This toast shows the failures so the user knows why
   * their hotkey does nothing, and links them directly to the rebind
   * UI. Dismiss is local-only — the next failure report will show it
   * again.
   */
  import { onMount } from 'svelte';
  import { AlertTriangle, X } from 'lucide-svelte';
  import { onHotkeyFailure, type HotkeyFailure } from '$lib/ipc';
  import { goto } from '$lib/navigation';

  let failures = $state<HotkeyFailure[]>([]);
  let visible = $state(false);

  onMount(() => {
    let unlistenFn: (() => void) | null = null;
    const setup = async () => {
      const fn = await onHotkeyFailure((report) => {
        if (!report.failed || report.failed.length === 0) {
          return;
        }
        failures = report.failed;
        visible = true;
      });
      unlistenFn = fn;
    };
    void setup();
    return () => {
      unlistenFn?.();
    };
  });

  function dismiss() {
    visible = false;
  }

  function rebind() {
    visible = false;
    goto('/settings/');
  }
</script>

{#if visible && failures.length > 0}
  <div
    role="alert"
    aria-live="polite"
    class="fixed right-4 top-4 z-50 max-w-sm rounded-[var(--ml-radius-md)] border border-[color:var(--ml-border)] bg-[color:var(--ml-surface)] p-4 shadow-lg"
  >
    <div class="flex items-start gap-3">
      <AlertTriangle class="mt-0.5 h-4 w-4 flex-none text-amber-500" aria-hidden="true" />
      <div class="flex-1 min-w-0">
        <p class="text-sm font-medium">ホットキーの登録に失敗しました</p>
        <p class="mt-1 text-xs text-[color:var(--ml-ink-muted)]">
          他のアプリや IME が同じキーを使用している可能性があります。
        </p>
        <ul class="mt-2 space-y-1 text-xs">
          {#each failures as failure (failure.id)}
            <li class="truncate">
              <span class="font-medium">{failure.id}</span>
              <span class="text-[color:var(--ml-ink-muted)]">—</span>
              <code class="font-mono">{failure.accelerator}</code>
            </li>
          {/each}
        </ul>
        <div class="mt-3 flex gap-2">
          <button
            type="button"
            class="rounded-[var(--ml-radius-sm)] bg-[color:var(--ml-accent)] px-2.5 py-1 text-xs font-medium text-white hover:opacity-90"
            onclick={rebind}
          >
            設定で変更
          </button>
          <button
            type="button"
            class="rounded-[var(--ml-radius-sm)] px-2.5 py-1 text-xs font-medium text-[color:var(--ml-ink-muted)] hover:bg-[color:var(--ml-surface-muted)]"
            onclick={dismiss}
          >
            閉じる
          </button>
        </div>
      </div>
      <button
        type="button"
        aria-label="閉じる"
        class="flex-none rounded-[var(--ml-radius-sm)] p-0.5 text-[color:var(--ml-ink-muted)] hover:bg-[color:var(--ml-surface-muted)]"
        onclick={dismiss}
      >
        <X class="h-3.5 w-3.5" aria-hidden="true" />
      </button>
    </div>
  </div>
{/if}
