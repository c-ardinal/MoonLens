<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { getSettings } from '$lib/ipc';
  import MacOnboarding from '$lib/components/MacOnboarding.svelte';
  import HotkeyFailureToast from '$lib/components/HotkeyFailureToast.svelte';

  let { children } = $props();
  let theme = $state<'light' | 'dark' | 'system'>('system');
  let density = $state<'compact' | 'normal' | 'comfortable'>('normal');

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

{@render children()}
<MacOnboarding />
<HotkeyFailureToast />
