<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { ensureCaptureSubscription } from '$lib/stores/captureStore';
  import {
    currentSettings,
    ensureSettingsLoaded
  } from '$lib/stores/settingsStore';
  import EmbeddedModelLoadingBadge from '$lib/components/EmbeddedModelLoadingBadge.svelte';
  import EmbeddedModelOnboarding from '$lib/components/EmbeddedModelOnboarding.svelte';
  import MacOnboarding from '$lib/components/MacOnboarding.svelte';
  import HotkeyFailureToast from '$lib/components/HotkeyFailureToast.svelte';
  import WindowChrome from '$lib/components/WindowChrome.svelte';
  import AppSidebar from '$lib/components/AppSidebar.svelte';

  let { children } = $props();

  // Floating windows (overlay/subtitle/lens-frame/selector) are
  // chromeless by design and render their page directly without the
  // shell. `/lens/` is the management page inside the main app and
  // intentionally renders WITH the shell.
  const FLOATING_PREFIXES = ['/overlay', '/subtitle', '/select', '/lens-frame'];

  const isFloating = $derived(
    FLOATING_PREFIXES.some((p) => $page.url.pathname.startsWith(p))
  );

  // Read theme / accent / density from the global settings store so
  // changes from the settings page reflect immediately without a
  // restart. The store also retains a boot snapshot for the
  // restart-required diff that the settings page consumes.
  const theme = $derived(
    ($currentSettings?.appearance?.theme ?? 'system') as
      | 'light'
      | 'dark'
      | 'system'
  );
  const density = $derived(
    ($currentSettings?.appearance?.density ?? 'normal') as
      | 'compact'
      | 'normal'
      | 'comfortable'
  );
  const accent = $derived<string>(
    ($currentSettings?.appearance?.accent as string | undefined) ?? '#A8C5DA'
  );
  const reducedMotion = $derived<boolean>(
    Boolean($currentSettings?.appearance?.reduced_motion)
  );

  onMount(async () => {
    void ensureCaptureSubscription();
    void ensureSettingsLoaded();
  });

  $effect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.density = density;
    document.documentElement.dataset.reducedMotion = reducedMotion
      ? 'on'
      : 'off';
  });

  /** Push the live accent through to the CSS variables driving every
   *  themed surface. We derive the secondary tones (`accent-soft`,
   *  `accent-strong`, `scan-glow`) so the design stays internally
   *  consistent even when the user picks a custom hex. */
  $effect(() => {
    if (typeof document === 'undefined') return;
    const root = document.documentElement;
    const hex = accent.replace('#', '');
    if (hex.length !== 6) return;
    const r = parseInt(hex.slice(0, 2), 16);
    const g = parseInt(hex.slice(2, 4), 16);
    const b = parseInt(hex.slice(4, 6), 16);
    if (Number.isNaN(r) || Number.isNaN(g) || Number.isNaN(b)) return;
    root.style.setProperty('--accent', accent);
    root.style.setProperty('--accent-soft', `rgba(${r}, ${g}, ${b}, 0.14)`);
    const darken = (c: number) => Math.max(0, Math.round(c * 0.78));
    root.style.setProperty(
      '--accent-strong',
      `rgb(${darken(r)}, ${darken(g)}, ${darken(b)})`
    );
    root.style.setProperty(
      '--scan-glow',
      `rgba(${r}, ${g}, ${b}, 0.6)`
    );
  });
</script>

{#if isFloating}
  {@render children()}
{:else}
  <div class="ml-app">
    <WindowChrome />
    <div class="ml-body">
      <AppSidebar />
      <main class="ml-main">
        {@render children()}
      </main>
    </div>
  </div>
{/if}

<MacOnboarding />
{#if !isFloating}
  <EmbeddedModelOnboarding />
{/if}
<EmbeddedModelLoadingBadge />
<HotkeyFailureToast />

<style>
  .ml-app {
    width: 100%;
    height: 100%;
    display: grid;
    grid-template-rows: 36px 1fr;
    background: var(--bg-0);
    color: var(--ink-0);
    border-radius: 12px;
    overflow: hidden;
    box-shadow: var(--shadow-lg);
    border: 1px solid var(--line);
    position: relative;
  }
  .ml-body {
    display: flex;
    min-height: 0;
    height: 100%;
  }
  .ml-main {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg-0);
    overflow: hidden;
    position: relative;
  }
  /* `.ml-content` is the per-page body slot. By default it holds a
   * grid- or split-pane layout that fills the available height, so we
   * keep it as a vertical flex container with `overflow: hidden` and
   * let internal `.scroll` panes handle scrolling. Pages whose body
   * is naturally stacked (home, lens manager) opt into auto-scroll
   * via the `scroll-y` modifier. */
  .ml-main :global(.ml-content) {
    flex: 1;
    min-height: 0;
    padding: 4px 28px 24px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .ml-main :global(.ml-content.scroll-y) {
    overflow-y: auto;
    overflow-x: hidden;
  }
</style>
