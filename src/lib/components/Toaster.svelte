<script lang="ts">
  import { t } from '$lib/i18n';
  import { X } from 'lucide-svelte';
  import { close, hold, release, toasts } from '$lib/toast.svelte';
</script>

<!--
  共通の通知。画面下端・左寄せへ積層（新規通知は最下段）。
  左寄せの理由：各画面の右下は操作部（履歴の削除・訳文のコピー）の定位置で、通知による遮蔽を回避。
  常設の領域（role=status）：通知の追加を読上げ対象とし、エラーのみ role=alert で即時読上げ。
-->
<div class="toaster px-zoom" role="status" aria-label={t('通知')}>
  {#each toasts.items as toast (toast.id)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="toast"
      class:error={toast.kind === 'error'}
      role={toast.kind === 'error' ? 'alert' : undefined}
      onmouseenter={() => hold(toast.id, 'pointer')}
      onmouseleave={() => release(toast.id, 'pointer')}
      onfocusin={() => hold(toast.id, 'focus')}
      onfocusout={() => release(toast.id, 'focus')}
    >
      <p class="text">{toast.text}</p>
      {#if toast.action}
        <button class="act" onclick={() => close(toast.id, 'action')}>{toast.action}</button>
      {/if}
      <button class="x" title={t('閉じる')} aria-label={t('通知を閉じる')} onclick={() => close(toast.id, 'dismiss')}>
        <X class="h-4 w-4" />
      </button>
    </div>
  {/each}
</div>

<style>
  .toaster {
    position: fixed;
    left: var(--ml-page-x, 24px);
    bottom: var(--ml-sp-4, 16px);
    z-index: 60;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--ml-sp-2, 8px);
    max-width: min(100vw - 2 * var(--ml-page-x, 24px), 720px);
    pointer-events: none;
  }
  /* 設定画面の通知バー（画面下端固定）表示中：バーの上へ退避 */
  :global(:root:has(.savebar)) .toaster {
    bottom: calc(var(--ml-sp-16, 64px) + var(--ml-sp-4, 16px));
  }
  .toast {
    display: flex;
    align-items: center;
    gap: var(--ml-sp-3, 12px);
    min-height: var(--ml-sp-12, 48px);
    padding: var(--ml-sp-1, 4px) var(--ml-sp-2, 8px) var(--ml-sp-1, 4px) var(--ml-sp-4, 16px);
    box-sizing: border-box;
    pointer-events: auto;
    background: var(--ml-px-black, #111);
    color: var(--ml-px-cream, #eee);
    border: 2px solid var(--ml-px-amber, #d9a441);
    box-shadow: 4px 4px 0 var(--ml-px-dim, #444);
    animation: toast-rise 120ms steps(3, start) backwards;
  }
  .toast.error {
    border-color: var(--ml-px-red, #d33);
    color: var(--ml-px-red, #d33);
  }
  .text {
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
  }
  .act {
    flex-shrink: 0;
    height: var(--ml-sp-10, 40px);
    padding: 0 var(--ml-sp-3, 12px);
    background: var(--ml-px-amber, #d9a441);
    color: var(--ml-px-black, #111);
  }
  .act:hover {
    background: var(--ml-px-cream, #eee);
  }
  .x {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    width: var(--ml-sp-10, 40px);
    height: var(--ml-sp-10, 40px);
    color: var(--ml-px-faint, #888);
  }
  .x:hover {
    color: var(--ml-px-cream, #eee);
  }
  @keyframes toast-rise {
    from {
      transform: translateY(100%);
      opacity: 0.2;
    }
  }
  /* 推論中・動き削減時：即時表示 */
  :global(:root:not([data-motion='on'])) .toast {
    animation-duration: 0s;
  }
</style>
