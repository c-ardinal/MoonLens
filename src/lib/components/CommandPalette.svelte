<script lang="ts">
  import { t } from '$lib/i18n';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    Camera,
    Puzzle,
    History,
    Home,
    Image as ImageIcon,
    Languages,
    Settings as SettingsIcon,
    Mic,
    MicOff,
    Power
  } from 'lucide-svelte';
  import {
    getSettings,
    onHotkey,
    quitApp,
    openLens,
    startSubtitle,
    stopSubtitle,
    subtitleState
  } from '$lib/ipc';
  import { displayAccelerator } from '$lib/hotkey';
  import { errorText } from '$lib/errors';

  type Command = {
    id: string;
    label: string;
    hint?: string;
    icon: any;
    run: () => Promise<void> | void;
  };

  let open = $state(false);
  let q = $state('');
  let running = $state(false);
  let subtitleRunning = $state(false);
  let hotkeys = $state<Record<string, string>>({});
  let error = $state<string | null>(null);

  async function openWindow() {
    open = false;
    await openLens();
  }

  async function toggleSub() {
    open = false;
    if (subtitleRunning) await stopSubtitle();
    else await startSubtitle();
    subtitleRunning = !subtitleRunning;
  }

  const hint = (key: string) => (hotkeys[key] ? displayAccelerator(hotkeys[key]) : undefined);
  const go = (path: string) => () => {
    open = false;
    goto(path);
  };

  const commands: Command[] = $derived([
    {
      id: 'capture',
      label: t('ウィンドウ翻訳'),
      hint: hint('capture_region'),
      icon: Camera,
      run: openWindow
    },
    {
      id: 'subtitle',
      label: subtitleRunning ? t('音声翻訳 停止') : t('音声翻訳 開始'),
      hint: hint('toggle_subtitle'),
      icon: subtitleRunning ? MicOff : Mic,
      run: toggleSub
    },
    { id: 'home', label: t('ホーム'), icon: Home, run: go('/') },
    { id: 'image', label: t('画像翻訳'), icon: ImageIcon, run: go('/image/') },
    { id: 'translate', label: t('テキスト翻訳'), icon: Languages, run: go('/translate/') },
    { id: 'settings', label: t('設定'), icon: SettingsIcon, run: go('/settings/') },
    { id: 'history', label: t('履歴'), hint: hint('open_history'), icon: History, run: go('/history/') },
    { id: 'plugins', label: t('拡張機能管理'), icon: Puzzle, run: go('/settings/#section-plugins') },
    { id: 'quit', label: t('MoonLens 終了'), icon: Power, run: () => quitApp() }
  ]);

  const filtered = $derived(
    commands.filter((c) => c.label.toLowerCase().includes(q.toLowerCase()))
  );

  async function runWith(cmd: Command) {
    if (running) return;
    running = true;
    error = null;
    try {
      await cmd.run();
    } catch (e) {
      error = errorText(e);
      open = true;
    } finally {
      running = false;
    }
  }

  function show() {
    open = true;
    q = '';
    getSettings()
      .then((s) => (hotkeys = s?.hotkeys ?? {}))
      .catch(() => {});
  }

  onMount(() => {
    // The global hotkey (handled by the backend) brings the main window
    // up and asks for the palette.
    let unlistenHotkey: (() => void) | undefined;
    onHotkey((action) => {
      if (action === 'open_command_palette') show();
    })
      .then((u) => (unlistenHotkey = u))
      .catch(() => {});
    // Fetch initial subtitle state so the toggle label is accurate on
    // first open, then keep it in sync via the status API.
    subtitleState()
      .then((s) => {
        subtitleRunning = s;
      })
      .catch(() => {});
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        if (open) open = false;
        else show();
      } else if (e.key === 'Escape') {
        open = false;
      }
    };
    window.addEventListener('keydown', handler);
    return () => {
      window.removeEventListener('keydown', handler);
      unlistenHotkey?.();
    };
  });
</script>

{#if open}
  <div
    class="fixed inset-0 z-50 flex items-start justify-center bg-black/30 px-4 pt-[12vh] backdrop-blur-sm"
    role="presentation"
    onclick={() => (open = false)}
    onkeydown={(e) => e.key === 'Escape' && (open = false)}
    tabindex="-1"
  >
    <div
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      class="w-full max-w-lg rounded-ml border border-border bg-surface-elevated shadow-ml-lg"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <!-- svelte-ignore a11y_autofocus -->
      <input
        autofocus
        bind:value={q}
        placeholder={t('コマンド検索')}
        class="w-full rounded-t-ml border-b border-border bg-transparent px-4 py-3 text-sm focus:outline-none"
      />
      <ul class="scrollbar-minimal max-h-80 overflow-auto">
        {#each filtered as c (c.id)}
          <li>
            <button
              class="flex w-full items-center justify-between px-4 py-[var(--ml-row-pad)] text-left text-sm transition hover:bg-surface-muted disabled:opacity-50"
              disabled={running}
              onclick={() => runWith(c)}
            >
              <span class="flex items-center gap-2">
                <c.icon class="h-4 w-4 text-ink-muted" />
                {c.label}
              </span>
              {#if c.hint}
                <kbd class="font-mono text-xs text-ink-faint">{c.hint}</kbd>
              {/if}
            </button>
          </li>
        {:else}
          <li class="px-4 py-6 text-center text-sm text-ink-faint">{t('該当なし')}</li>
        {/each}
      </ul>
      {#if error}
        <p class="border-t border-border px-4 py-2 text-xs break-words text-red-500">{error}</p>
      {/if}
    </div>
  </div>
{/if}
