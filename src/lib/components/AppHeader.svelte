<script lang="ts">
  /**
   * Shared top bar for non-root pages.
   *
   * Provides a visible way back to the home screen. Tauri windows
   * have no browser chrome, so without this the user is stuck on
   * whatever page they navigated into until they close the window.
   */
  import type { Snippet } from 'svelte';
  import { ArrowLeft } from 'lucide-svelte';
  import { goto } from '$lib/navigation';

  interface Props {
    title: string;
    subtitle?: string;
    /** Where the back button goes. Defaults to the app home. */
    backTo?: string;
    /** Optional right-aligned actions (save button, search box, etc). */
    actions?: Snippet;
  }

  let { title, subtitle, backTo = '/', actions }: Props = $props();

  function goBack() {
    goto(backTo);
  }

  // ESC from anywhere on the page sends the user home. Tauri windows
  // have no browser chrome, so this is the primary keyboard escape
  // hatch when focus is not on the back button itself.
  function onGlobalKey(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    const target = e.target as HTMLElement | null;
    if (target) {
      const tag = target.tagName;
      // Respect in-flight editing: ESC inside inputs/textareas should
      // fall through to the control (clear value, close menu, etc).
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (target.isContentEditable) return;
    }
    goBack();
  }

  $effect(() => {
    window.addEventListener('keydown', onGlobalKey);
    return () => window.removeEventListener('keydown', onGlobalKey);
  });
</script>

<header class="flex items-center gap-3">
  <button
    type="button"
    onclick={goBack}
    class="inline-flex h-8 w-8 items-center justify-center rounded-ml-sm border border-border bg-surface text-ink-muted transition hover:border-accent/60 hover:text-ink"
    aria-label="ホームへ戻る"
    title="ホームへ戻る"
  >
    <ArrowLeft class="h-4 w-4" aria-hidden="true" />
  </button>
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
