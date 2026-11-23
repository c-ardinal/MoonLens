<script lang="ts">
  import { page } from '$app/state';
  import { Minus, X } from 'lucide-svelte';
  import { slant } from '$lib/pixel/slant';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { hideMain } from '$lib/ipc';

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

  const active = (href: string) =>
    href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);
</script>

<nav class="sticky top-0 z-30 bg-surface" aria-label="メインメニュー" data-tauri-drag-region>
  <div class="flex items-center gap-1 py-2 pr-2" data-tauri-drag-region>
    <a href="/" class="px-chip logo" data-tone="amber" use:slant={0.4}>MOONLENS</a>
    <ul class="scrollbar-minimal flex min-w-0 flex-1 items-center gap-1 overflow-x-auto pl-2" data-tauri-drag-region>
      {#each items as item (item.href)}
        <li class="shrink-0">
          <a
            href={item.href}
            aria-current={active(item.href) ? 'page' : undefined}
            class="px-chip"
            data-tone={active(item.href) ? 'bone' : undefined}
            use:slant
          >
            {item.label}
          </a>
        </li>
      {/each}
    </ul>
    <div class="flex shrink-0 items-center gap-1" data-tauri-drag-region>
      <button class="ctl" title="最小化" aria-label="最小化" onclick={minimize}>
        <Minus class="h-4 w-4" />
      </button>
      <button
        class="ctl close"
        title="非表示（常駐継続）"
        aria-label="非表示（常駐継続）"
        onclick={() => hideMain().catch(() => {})}
      >
        <X class="h-4 w-4" />
      </button>
    </div>
  </div>
  <div class="rule" aria-hidden="true"></div>
</nav>

<style>
  .logo {
    padding-left: 24px;
    padding-right: 28px;
    margin-left: -8px;
  }
  .px-chip:not([data-tone]):hover {
    background: var(--ml-px-dim);
    color: var(--ml-px-cream);
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
  .rule {
    height: 2px;
    margin: 0 16px;
    background: var(--ml-px-dim);
  }
</style>
