<script lang="ts">
  import { page } from '$app/state';
  import { Minus, X } from 'lucide-svelte';
  import { slant } from '$lib/pixel/slant';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onDestroy, onMount } from 'svelte';
  import { getSettings, hideMain, onSettingsChanged } from '$lib/ipc';
  import { languageLabel } from '$lib/lens-overlay';

  // The main window is frameless: this bar is its title bar (drag to
  // move, double-click to maximize) and carries minimize / close. Close
  // hides to the tray; quitting is in the tray menu and the palette.
  async function minimize() {
    try {
      await getCurrentWindow().minimize();
    } catch {
      /* pure frontend dev */
    }
  }

  const items = [
    { href: '/', label: 'ホーム' },
    { href: '/translate/', label: '翻訳' },
    { href: '/image/', label: '画像翻訳' },
    { href: '/history/', label: '履歴' },
    { href: '/plugins/', label: 'プラグイン' },
    { href: '/settings/', label: '設定' }
  ];

  /** 翻訳先言語（ライブ字幕・画面翻訳の共通設定）。 */
  let target = $state('');
  const apply = (s: any) => {
    const code = s?.subtitle?.target_language ?? '';
    target = code ? languageLabel(code) || code : '';
  };
  let unlisten: (() => void) | undefined;
  onMount(async () => {
    getSettings().then(apply).catch(() => {});
    try {
      unlisten = await onSettingsChanged(apply);
    } catch {
      /* pure frontend dev */
    }
  });
  onDestroy(() => unlisten?.());

  /** タブの傾き：斜めの切抜き幅（上端 4px）を右余白内へ収め、文字との干渉を防止。 */
  const TAB_K = 0.15;

  const active = (href: string) =>
    href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);
</script>

<nav class="sticky top-0 z-30" aria-label="メインメニュー" data-tauri-drag-region>
  <div class="bar" data-tauri-drag-region>
    <a href="/" class="logo" use:slant={0.4}>MOONLENS</a>
    <ul class="tabs" data-tauri-drag-region>
      {#each items as item (item.href)}
        <li>
          <a
            href={item.href}
            aria-current={active(item.href) ? 'page' : undefined}
            class="px-chip"
            data-tone={active(item.href) ? 'bone' : undefined}
            use:slant={TAB_K}
          >
            {item.label}
          </a>
        </li>
      {/each}
    </ul>
    {#if target}
      <a href="/settings/" class="px-chip target" data-tone="amber" use:slant={TAB_K} title="翻訳先言語：{target}（設定で変更）">{target}▶</a>
    {/if}
    <div class="ctls" data-tauri-drag-region>
      <button class="ctl" title="最小化" aria-label="最小化" onclick={minimize}>
        <Minus class="h-6 w-6" />
      </button>
      <button class="ctl close" title="非表示（常駐継続）" aria-label="非表示（常駐継続）" onclick={() => hideMain().catch(() => {})}>
        <X class="h-6 w-6" />
      </button>
    </div>
  </div>
</nav>

<style>
  nav {
    height: var(--ml-nav-h);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 100%;
    padding-right: 8px;
  }
  .logo {
    align-self: flex-start;
    display: flex;
    align-items: center;
    height: 64px;
    padding: 0 22px 0 30px; /* 斜めの左端（上端ほど右へ後退）と文字の干渉防止 */
    background: var(--ml-px-amber);
    color: var(--ml-px-black);
    font-size: 32px;
  }
  .tabs {
    display: flex;
    min-width: 0;
    flex: 1;
    gap: 2px;
    overflow-x: auto;
    padding-left: 8px;
    scrollbar-width: none;
  }
  .tabs li {
    flex-shrink: 0;
  }
  .px-chip {
    padding: 0 8px 0 12px;
  }
  .px-chip:not([data-tone]) {
    color: var(--ml-px-mid);
  }
  .px-chip:not([data-tone]):hover {
    background: var(--ml-px-dim);
    color: var(--ml-px-cream);
  }
  .target {
    flex-shrink: 0;
  }
  .ctls {
    display: flex;
    flex-shrink: 0;
    gap: 2px;
  }
  .ctl {
    padding: 4px;
    color: var(--ml-px-faint);
  }
  .ctl:hover {
    background: var(--ml-px-dim);
    color: var(--ml-px-cream);
  }
  .ctl.close:hover {
    background: var(--ml-px-red);
    color: var(--ml-px-black);
  }
</style>
