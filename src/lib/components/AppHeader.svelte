<script lang="ts">
  /**
   * Shared in-page header for sub-route content. The back button is
   * now opt-in: with the persistent sidebar shell, most pages don't
   * need it. Kept here for floating windows that still route
   * independently.
   */
  import type { Snippet } from 'svelte';
  import { ArrowLeft } from 'lucide-svelte';
  import { goto } from '$lib/navigation';

  interface Props {
    title: string;
    subtitle?: string;
    /** Show a back button and ESC-to-home; defaults to false because
     *  navigation lives in the persistent sidebar. */
    showBack?: boolean;
    /** Where the back button goes. Defaults to the app home. */
    backTo?: string;
    /** Optional right-aligned actions (save button, search box, etc). */
    actions?: Snippet;
  }

  let {
    title,
    subtitle,
    showBack = false,
    backTo = '/',
    actions
  }: Props = $props();

  function goBack() {
    goto(backTo);
  }

  function onGlobalKey(e: KeyboardEvent) {
    if (!showBack || e.key !== 'Escape') return;
    const target = e.target as HTMLElement | null;
    if (target) {
      const tag = target.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (target.isContentEditable) return;
    }
    goBack();
  }

  $effect(() => {
    if (!showBack) return;
    window.addEventListener('keydown', onGlobalKey);
    return () => window.removeEventListener('keydown', onGlobalKey);
  });
</script>

<header class="flex items-center gap-3">
  {#if showBack}
    <button
      type="button"
      onclick={goBack}
      class="inline-flex h-8 w-8 items-center justify-center rounded-ml-sm border border-border bg-surface text-ink-muted transition hover:border-accent/60 hover:text-ink"
      aria-label="ホームへ戻る"
      title="ホームへ戻る"
    >
      <ArrowLeft class="h-4 w-4" aria-hidden="true" />
    </button>
  {/if}
  <div class="flex-1 min-w-0">
    <h1 class="text-xl font-semibold tracking-tight">{title}</h1>
    {#if subtitle}
      <p class="text-sm text-ink-muted">{subtitle}</p>
    {/if}
  </div>
  {#if actions}
    <div class="flex items-center gap-2">
      {@render actions()}
    </div>
  {/if}
</header>
