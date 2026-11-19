<script lang="ts">
  import '../app.css';
  import { onDestroy, onMount } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { getSettings, onNavigate, onSettingsChanged } from '$lib/ipc';
  import { applyAppearance } from '$lib/appearance';
  import MacOnboarding from '$lib/components/MacOnboarding.svelte';
  import AppNav from '$lib/components/AppNav.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';

  let { children } = $props();
  const unlisteners: (() => void)[] = [];

  // Overlay-type windows (selector, overlay, subtitle, lens) draw their own
  // chrome; only the main window's pages get the menu and palette.
  const MAIN_ROUTES = ['/', '/translate/', '/image/', '/settings/', '/history/', '/plugins/'];
  const isMain = $derived(MAIN_ROUTES.includes(page.url.pathname));

  onMount(async () => {
    try {
      applyAppearance(await getSettings());
      unlisteners.push(await onSettingsChanged(applyAppearance));
      // Tray menu, hotkeys and other windows open main-window pages.
      // Only the main window receives this event.
      unlisteners.push(await onNavigate((path) => goto(path)));
    } catch {
      /* backend not reachable during pure-frontend dev; keep defaults */
    }
  });
  onDestroy(() => unlisteners.forEach((u) => u()));
</script>

{#if isMain}
  <AppNav />
  {@render children()}
  <CommandPalette />
{:else}
  {@render children()}
{/if}
<MacOnboarding />
