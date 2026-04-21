<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { getSettings } from '$lib/ipc';
  import MacOnboarding from '$lib/components/MacOnboarding.svelte';
  import HotkeyFailureToast from '$lib/components/HotkeyFailureToast.svelte';
  import WindowChrome from '$lib/components/WindowChrome.svelte';
  import AppSidebar from '$lib/components/AppSidebar.svelte';

  let { children } = $props();
  let theme = $state<'light' | 'dark' | 'system'>('system');
  let density = $state<'compact' | 'normal' | 'comfortable'>('normal');

  // Only the "main" window shell uses the sidebar + chrome. Floating
  // windows (overlay/subtitle/lens/selector/lens-frame) are chromeless
  // by design and render their page directly.
  const FLOATING_PREFIXES = ['/overlay', '/subtitle', '/select', '/lens', '/lens-frame'];

  const isFloating = $derived(
    FLOATING_PREFIXES.some((p) => $page.url.pathname.startsWith(p))
  );

  onMount(async () => {
    try {
      const s = (await getSettings()) as any;
      theme = s?.appearance?.theme ?? 'system';
      density = s?.appearance?.density ?? 'normal';
    } catch {
      /* backend not reachable during pure-frontend dev; keep defaults */
    }
  });

  $effect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.density = density;
  });
</script>

{#if isFloating}
  {@render children()}
{:else}
  <div class="app-shell flex h-screen w-screen flex-col overflow-hidden bg-surface text-ink">
    <WindowChrome />
    <div class="flex flex-1 overflow-hidden">
      <AppSidebar />
      <div class="scrollbar-minimal flex-1 overflow-y-auto">
        {@render children()}
      </div>
    </div>
  </div>
{/if}

<MacOnboarding />
<HotkeyFailureToast />

<style>
  .app-shell {
    border: 1px solid var(--ml-border);
    border-radius: var(--ml-radius);
    box-shadow: var(--ml-shadow);
  }
</style>
