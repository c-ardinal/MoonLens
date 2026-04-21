<script lang="ts">
  /**
   * Persistent side navigation for the main window. Collapses from
   * 240px down to 48px (icon-only) to stay out of the content's way
   * without hiding the shortcut list. Collapsed state persists to
   * localStorage so it survives reloads.
   */
  import { page } from '$app/stores';
  import { goto } from '$lib/navigation';
  import {
    Home,
    Settings as SettingsIcon,
    Puzzle,
    History,
    Mic,
    PanelLeftClose,
    PanelLeft,
    OctagonX
  } from 'lucide-svelte';
  import { stopAll } from '$lib/ipc';

  interface NavItem {
    href: string;
    label: string;
    icon: typeof Home;
    hint?: string;
  }

  const items: NavItem[] = [
    { href: '/', label: 'ホーム', icon: Home, hint: '⌘1' },
    { href: '/settings/', label: '設定', icon: SettingsIcon, hint: '⌘,' },
    { href: '/history/', label: '履歴', icon: History, hint: '⌘⇧H' },
    { href: '/plugins/', label: 'プラグイン', icon: Puzzle },
    { href: '/subtitle/', label: '字幕', icon: Mic, hint: '⌘⇧S' }
  ];

  const STORAGE_KEY = 'moonlens.sidebar.collapsed';

  let collapsed = $state(false);
  let stopping = $state(false);
  let stopReport = $state<string | null>(null);
  let stopReportTimer: ReturnType<typeof setTimeout> | undefined;

  async function panicStopAll() {
    if (stopping) return;
    stopping = true;
    try {
      const r = await stopAll();
      const parts: string[] = [];
      if (r.lens_count > 0) parts.push(`Lens ${r.lens_count}件`);
      if (r.subtitle_was_running) parts.push('字幕');
      if (r.overlay_was_open) parts.push('Overlay');
      stopReport = parts.length === 0 ? '何も動いていません' : `${parts.join('・')} を停止`;
    } catch (e) {
      stopReport = `停止失敗: ${e}`;
    } finally {
      stopping = false;
      if (stopReportTimer) clearTimeout(stopReportTimer);
      stopReportTimer = setTimeout(() => (stopReport = null), 3000);
    }
  }

  $effect(() => {
    if (typeof window === 'undefined') return;
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved === '1') collapsed = true;
  });

  function toggle() {
    collapsed = !collapsed;
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(STORAGE_KEY, collapsed ? '1' : '0');
    }
  }

  function isActive(href: string): boolean {
    const pathname = $page.url.pathname;
    if (href === '/') return pathname === '/';
    // Treat `/settings/` as a prefix match so `/settings/foo` also lights up.
    return pathname.startsWith(href);
  }

  function navigate(href: string) {
    goto(href);
  }
</script>

<aside
  class="app-sidebar flex shrink-0 flex-col border-r border-border bg-surface/60 backdrop-blur transition-[width] duration-200 ease-out"
  class:collapsed
  aria-label="主ナビゲーション"
>
  <nav class="flex-1 overflow-y-auto px-2 py-3">
    <ul class="flex flex-col gap-1">
      {#each items as item (item.href)}
        {@const active = isActive(item.href)}
        <li>
          <button
            type="button"
            onclick={() => navigate(item.href)}
            class="group relative flex w-full items-center gap-3 rounded-ml-sm px-2.5 py-2 text-sm transition-colors"
            class:text-ink={active}
            class:bg-surface-muted={active}
            class:text-ink-muted={!active}
            aria-current={active ? 'page' : undefined}
            title={collapsed ? item.label : undefined}
          >
            {#if active}
              <span
                class="absolute left-0 top-1/2 h-5 w-[2px] -translate-y-1/2 rounded-full bg-accent"
                aria-hidden="true"
              ></span>
            {/if}
            <item.icon
              class="h-4 w-4 shrink-0 transition-colors {active
                ? 'text-accent'
                : 'group-hover:text-ink'}"
              aria-hidden="true"
            />
            {#if !collapsed}
              <span class="flex-1 truncate text-left">{item.label}</span>
              {#if item.hint}
                <kbd class="font-mono text-[10px] text-ink-faint">{item.hint}</kbd>
              {/if}
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="border-t border-border p-2">
    <button
      type="button"
      onclick={panicStopAll}
      disabled={stopping}
      class="group flex w-full items-center gap-3 rounded-ml-sm px-2.5 py-2 text-xs text-ink-muted transition-colors hover:bg-red-500/10 hover:text-red-500 disabled:opacity-40"
      aria-label="全停止 (panic)"
      title={collapsed ? '全停止' : '全 Lens + 字幕 + Overlay を停止 (⌘⌥Esc)'}
    >
      <OctagonX class="h-4 w-4 shrink-0" aria-hidden="true" />
      {#if !collapsed}
        <span class="flex-1 text-left">{stopping ? '停止中…' : '全停止'}</span>
        <kbd class="font-mono text-[10px] text-ink-faint group-hover:text-red-500/70">⌘⌥Esc</kbd>
      {/if}
    </button>
    {#if stopReport && !collapsed}
      <p
        class="mt-1 rounded-ml-sm bg-surface-muted px-2 py-1 text-[10px] text-ink-faint"
        aria-live="polite"
      >
        {stopReport}
      </p>
    {/if}

    <button
      type="button"
      onclick={toggle}
      class="mt-1 flex w-full items-center gap-3 rounded-ml-sm px-2.5 py-2 text-xs text-ink-faint transition-colors hover:bg-surface-muted hover:text-ink-muted"
      aria-label={collapsed ? 'サイドバーを展開' : 'サイドバーを折り畳む'}
      title={collapsed ? '展開' : '折り畳む'}
    >
      {#if collapsed}
        <PanelLeft class="h-4 w-4" aria-hidden="true" />
      {:else}
        <PanelLeftClose class="h-4 w-4" aria-hidden="true" />
        <span class="flex-1 text-left">折り畳む</span>
      {/if}
    </button>
  </div>
</aside>

<style>
  .app-sidebar {
    width: 240px;
  }
  .app-sidebar.collapsed {
    width: 48px;
  }
</style>
