<script lang="ts">
  import '../app.css';
  import { onDestroy, onMount } from 'svelte';
  import { page } from '$app/state';
  import { getSettings, onSettingsChanged } from '$lib/ipc';
  import { applyAppearance } from '$lib/appearance';
  import MacOnboarding from '$lib/components/MacOnboarding.svelte';
  import AppNav from '$lib/components/AppNav.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';

  let { children } = $props();
  let unlisten: (() => void) | undefined;

  // Overlay-type windows (selector, overlay, subtitle, lens) draw their own
  // chrome; only the main window's pages get the menu and palette.
  const MAIN_ROUTES = ['/', '/settings/', '/history/', '/plugins/'];
  const isMain = $derived(MAIN_ROUTES.includes(page.url.pathname));

  onMount(async () => {
    try {
      applyAppearance(await getSettings());
      unlisten = await onSettingsChanged(applyAppearance);
    } catch {
      /* backend not reachable during pure-frontend dev; keep defaults */
    }
  });
  onDestroy(() => unlisten?.());
</script>

{#if isMain}
  <AppNav />
  {@render children()}
  <CommandPalette />
{:else}
  {@render children()}
{/if}
<MacOnboarding />
