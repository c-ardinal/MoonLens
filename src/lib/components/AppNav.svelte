<script lang="ts">
  import { page } from '$app/state';
  import { Home, History, Image as ImageIcon, Languages, Minus, Puzzle, Settings as SettingsIcon, X } from 'lucide-svelte';
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
    { href: '/', label: 'ホーム', icon: Home },
    { href: '/translate/', label: '翻訳', icon: Languages },
    { href: '/image/', label: '画像翻訳', icon: ImageIcon },
    { href: '/history/', label: '履歴', icon: History },
    { href: '/plugins/', label: 'プラグイン', icon: Puzzle },
    { href: '/settings/', label: '設定', icon: SettingsIcon }
  ];

  const active = (href: string) =>
    href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);
</script>

<nav
  class="sticky top-0 z-30 border-b border-border bg-surface/85 backdrop-blur-xl"
  aria-label="メインメニュー"
  data-tauri-drag-region
>
  <div class="mx-auto flex max-w-5xl items-center gap-2 py-2 pl-4 pr-2 sm:pl-6" data-tauri-drag-region>
    <a href="/" class="mr-2 shrink-0 text-base font-semibold tracking-tight">MoonLens</a>
    <ul class="scrollbar-minimal flex min-w-0 flex-1 items-center gap-1 overflow-x-auto" data-tauri-drag-region>
      {#each items as item (item.href)}
        <li class="shrink-0">
          <a
            href={item.href}
            aria-current={active(item.href) ? 'page' : undefined}
            class="inline-flex items-center gap-1.5 rounded-ml-sm px-3 py-1.5 text-sm transition {active(item.href)
              ? 'bg-accent-soft text-ink'
              : 'text-ink-muted hover:bg-surface-muted hover:text-ink'}"
          >
            <item.icon class="h-4 w-4" />
            {item.label}
          </a>
        </li>
      {/each}
    </ul>
    <div class="flex shrink-0 items-center" data-tauri-drag-region>
      <button
        class="rounded-ml-sm p-1.5 text-ink-faint transition hover:bg-surface-muted hover:text-ink"
        title="最小化"
        aria-label="最小化"
        onclick={minimize}
      >
        <Minus class="h-4 w-4" />
      </button>
      <button
        class="rounded-ml-sm p-1.5 text-ink-faint transition hover:bg-red-500/15 hover:text-red-500"
        title="非表示（常駐継続）"
        aria-label="非表示（常駐継続）"
        onclick={() => hideMain().catch(() => {})}
      >
        <X class="h-4 w-4" />
      </button>
    </div>
  </div>
</nav>
