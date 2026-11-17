<script lang="ts">
  import { page } from '$app/state';
  import { Home, History, Puzzle, Settings as SettingsIcon } from 'lucide-svelte';

  const items = [
    { href: '/', label: 'ホーム', icon: Home },
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
>
  <div class="mx-auto flex max-w-5xl items-center gap-2 px-4 py-2 sm:px-6">
    <a href="/" class="mr-2 shrink-0 text-base font-semibold tracking-tight">MoonLens</a>
    <ul class="scrollbar-minimal flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
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
  </div>
</nav>
