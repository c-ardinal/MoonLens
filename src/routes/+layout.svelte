<script lang="ts">
  import '../app.css';
  import { onDestroy, onMount } from 'svelte';
  import { page } from '$app/state';
  import { afterNavigate, goto } from '$app/navigation';
  import { getOnboardingStatus, getSettings, onNavigate, onSettingsChanged } from '$lib/ipc';
  import { applyAppearance } from '$lib/appearance';
  import { applyPixelSkin } from '$lib/pixel/skin';
  import { motion, watchMotion } from '$lib/pixel/motion.svelte';
  import { pageWipe } from '$lib/pixel/transition';
  import MacOnboarding from '$lib/components/MacOnboarding.svelte';
  import AppNav from '$lib/components/AppNav.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';

  let { children } = $props();
  const unlisteners: (() => void)[] = [];

  // Overlay-type windows (selector, overlay, subtitle, lens) draw their own
  // chrome; only the main window's pages get the menu and palette.
  const MAIN_ROUTES = ['/', '/translate/', '/image/', '/settings/', '/history/', '/setup/'];
  const isMain = $derived(MAIN_ROUTES.includes(page.url.pathname));
  // ドット絵外観はメインウィンドウ限定。訳文表示のウィンドウは原文準拠。
  $effect(() => applyPixelSkin(isMain));
  // 演出：推論中・動き削減設定時は停止（CSS 側は data-motion で同期）
  $effect(() => (isMain ? watchMotion() : undefined));
  $effect(() => {
    document.documentElement.dataset.motion = isMain && motion.on ? 'on' : 'off';
  });

  // 画面遷移の演出：新画面（ナビゲーション直後の要素）を斜めの帯の通過に合わせて表示。
  // 初回表示・同一画面内の移動（ハッシュ変更）は対象外。
  let band = $state<HTMLDivElement | null>(null);
  let shownPath: string | null = null;
  afterNavigate(() => {
    const prev = shownPath;
    shownPath = location.pathname;
    if (prev === null || prev === shownPath || !band) return;
    const pageEl = document.querySelector<HTMLElement>('.app-root > nav + *');
    if (pageEl && pageEl !== band) pageWipe(pageEl, band);
  });

  onMount(async () => {
    try {
      applyAppearance(await getSettings());
      unlisteners.push(await onSettingsChanged(applyAppearance));
      // Tray menu, hotkeys and other windows open main-window pages.
      // Only the main window receives this event.
      unlisteners.push(await onNavigate((path) => goto(path)));
      // 初回起動（翻訳・OCR モデル未取得かつ初期設定未完了）：初期設定へ誘導。
      if (page.url.pathname === '/' && (await getOnboardingStatus()).needs_setup) goto('/setup/');
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
  <div class="px-wipe" aria-hidden="true" bind:this={band}></div>
{:else}
  {@render children()}
{/if}
<MacOnboarding />
