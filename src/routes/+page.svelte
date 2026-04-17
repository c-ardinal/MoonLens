<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { Keyboard, Puzzle, History, Settings as SettingsIcon } from 'lucide-svelte';
  import { onHotkey } from '$lib/ipc';

  type Nav = { href: string; label: string; icon: any; hint: string };

  const nav: Nav[] = [
    { href: '/settings/', label: '設定', icon: SettingsIcon, hint: '⌘,' },
    { href: '/plugins/', label: 'プラグイン', icon: Puzzle, hint: '' },
    { href: '/history/', label: '履歴', icon: History, hint: '⌘⇧H' },
    { href: '/subtitle/', label: '字幕', icon: Keyboard, hint: '⌘⇧S' }
  ];

  onMount(() => {
    let off: (() => void) | undefined;
    onHotkey((action) => {
      if (action === 'open_history') goto('/history/');
      if (action === 'open_command_palette') goto('/settings/');
    })
      .then((fn) => (off = fn))
      .catch(() => {});
    return () => off?.();
  });
</script>

<main class="mx-auto flex max-w-3xl flex-col gap-6 p-8">
  <header class="flex items-baseline justify-between">
    <h1 class="text-2xl font-semibold tracking-tight">MoonLens</h1>
    <p class="text-sm text-ink-muted">Lens + 字幕 + 検索、すべて1つのホットキーで。</p>
  </header>

  <section class="grid grid-cols-2 gap-3">
    {#each nav as item}
      <a
        href={item.href}
        class="group flex items-center justify-between rounded-ml border border-border bg-surface-elevated p-4 shadow-ml transition hover:border-accent/40"
      >
        <span class="flex items-center gap-3">
          <item.icon class="h-5 w-5 text-ink-muted group-hover:text-accent" />
          <span class="font-medium">{item.label}</span>
        </span>
        {#if item.hint}
          <kbd class="font-mono text-xs text-ink-faint">{item.hint}</kbd>
        {/if}
      </a>
    {/each}
  </section>

  <footer class="text-xs text-ink-faint">
    キャプチャ: ⌘⇧Space / ピン留め: ⌘⇧P / 字幕: ⌘⇧S / 検索: ⌘⇧G
  </footer>
</main>
