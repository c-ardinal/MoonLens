<script lang="ts">
  import { t } from '$lib/i18n';
  import { page } from '$app/state';
  import { Minus, X } from 'lucide-svelte';
  import PixelLogo from './PixelLogo.svelte';
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

  // 機能（翻訳・画像翻訳など）の入口はホームの魔法陣へ一本化。上部は画面単位の経路のみ
  /** label は文言のキー ID（表示時に `t()` で取得）。 */
  const items = [
    { href: '/', label: 'common:home' },
    { href: '/history/', label: 'common:history' },
    { href: '/settings/', label: 'common:settings' }
  ];

  /** 翻訳先言語（ライブ字幕・画面翻訳の共通設定）。 */
  let targetCode = $state('');
  const target = $derived(targetCode ? languageLabel(targetCode) || targetCode : '');
  const apply = (s: any) => {
    targetCode = s?.subtitle?.target_language ?? '';
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

  /** 翻訳・画像翻訳はホーム配下の機能画面：ホームを選択状態で表示。 */
  const HOME_PATHS = ['/', '/translate/', '/image/'];
  const active = (href: string) =>
    href === '/' ? HOME_PATHS.includes(page.url.pathname) : page.url.pathname.startsWith(href);
  /** 選択中タブの位置（該当なしは -1）：括弧カーソルの移動先。 */
  const current = $derived(items.findIndex((item) => active(item.href)));
</script>

<nav class="sticky top-0 z-30" aria-label={t('app-nav:main_menu')} data-tauri-drag-region>
  <div class="bar" data-tauri-drag-region>
    <a href="/" class="logo" aria-label={t('app-nav:moonlens_home')}><PixelLogo /></a>
    <ul class="tabs" data-tauri-drag-region>
      {#each items as item (item.href)}
        <li>
          <a
            href={item.href}
            aria-current={active(item.href) ? 'page' : undefined}
            class="px-chip"
          >
            {t(item.label)}
          </a>
        </li>
      {/each}
      {#if current >= 0}
        <li class="cursor" style="--i: {current}" aria-hidden="true"></li>
      {/if}
    </ul>
    {#if target}
      <a href="/settings/" class="px-chip target" title={t('app-nav:target_language_target_change_in_settings', { target })}>{target}<span class="px-tri" aria-hidden="true"></span></a>
    {/if}
    <div class="ctls" data-tauri-drag-region>
      <button class="ctl" title={t('app-nav:minimize')} aria-label={t('app-nav:minimize')} onclick={minimize}>
        <Minus class="h-6 w-6" />
      </button>
      <button class="ctl close" title={t('app-nav:hide_keeps_running_in_the_tray')} aria-label={t('app-nav:hide_keeps_running_in_the_tray')} onclick={() => hideMain().catch(() => {})}>
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
    gap: var(--ml-sp-1);
    height: 100%;
    padding-right: var(--ml-sp-2);
  }
  .logo {
    align-self: flex-start;
    display: block;
    flex-shrink: 0;
  }
  .tabs {
    position: relative;
    display: flex;
    min-width: 0;
    flex: 1;
    gap: var(--ml-sp-1);
    overflow-x: auto;
    padding-left: var(--ml-sp-2);
    scrollbar-width: none;
  }
  .tabs li {
    flex-shrink: 0;
  }
  /* 背景の塗りなし：選択中のタブと翻訳先言語は四隅の括弧（照準枠）で表示 */
  .px-chip {
    padding: 0 var(--ml-sp-3);
    background: none;
  }
  /* 画面切替タブ：等幅で整列（括弧カーソルの移動量は幅＋間隔の整数倍） */
  .tabs {
    --tab-w: 124px;
  }
  /* 英語表示：文字数の多い文言（Settings 等）用にタブ幅を拡大 */
  :global(html[lang='en']) .tabs {
    --tab-w: 156px;
  }
  .tabs .px-chip {
    justify-content: center;
    width: var(--tab-w);
    color: var(--ml-px-mid);
  }
  .tabs .px-chip:hover {
    color: var(--ml-px-cream);
  }
  .cursor,
  .target {
    --corner: var(--ml-px-amber);
    --h: linear-gradient(var(--corner), var(--corner));
    background:
      var(--h) left top / 10px 2px,
      var(--h) left top / 2px 10px,
      var(--h) right top / 10px 2px,
      var(--h) right top / 2px 10px,
      var(--h) left bottom / 10px 2px,
      var(--h) left bottom / 2px 10px,
      var(--h) right bottom / 10px 2px,
      var(--h) right bottom / 2px 10px;
    background-repeat: no-repeat;
  }
  .tabs .px-chip[aria-current] {
    color: var(--ml-px-bone);
  }
  /* 括弧カーソル：画面切替時に選択先のタブへ段階移動。推論中・動き削減時は即時 */
  .cursor {
    position: absolute;
    top: 0;
    left: var(--ml-sp-2);
    width: var(--tab-w);
    height: 100%;
    pointer-events: none;
    transform: translateX(calc(var(--i) * (var(--tab-w) + var(--ml-sp-1))));
    transition: transform 160ms steps(4, start);
  }
  :global(:root:not([data-motion='on'])) .cursor {
    transition: none;
  }
  .target {
    --corner: var(--ml-px-faint);
    flex-shrink: 0;
    margin-right: var(--ml-sp-2);
    color: var(--ml-px-amber);
  }
  .target:hover {
    --corner: var(--ml-px-amber);
    color: var(--ml-px-cream);
  }
  .ctls {
    display: flex;
    flex-shrink: 0;
    gap: var(--ml-sp-1);
  }
  .ctl {
    padding: var(--ml-sp-1);
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
