<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    Camera,
    Puzzle,
    History,
    Focus,
    Home,
    Maximize,
    Settings as SettingsIcon,
    Mic,
    MicOff,
    Power
  } from 'lucide-svelte';
  import {
    getSettings,
    newLensWindow,
    onHotkey,
    quitApp,
    startRegionSelection,
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

  async function runCapture(full: boolean) {
    open = false;
    await startRegionSelection(full);
  }

  async function runOpenLens() {
    open = false;
    await newLensWindow();
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
      label: '範囲翻訳',
      hint: hint('capture_region'),
      icon: Camera,
      run: () => runCapture(false)
    },
    {
      id: 'capture-full',
      label: '全画面翻訳',
      hint: hint('capture_window'),
      icon: Maximize,
      run: () => runCapture(true)
    },
    { id: 'lens', label: 'Lens ウィンドウ', icon: Focus, run: runOpenLens },
    {
      id: 'subtitle',
      label: subtitleRunning ? 'ライブ字幕 停止' : 'ライブ字幕 開始',
      hint: hint('toggle_subtitle'),
      icon: subtitleRunning ? MicOff : Mic,
      run: toggleSub
    },
    { id: 'home', label: 'ホーム', icon: Home, run: go('/') },
    { id: 'settings', label: '設定', icon: SettingsIcon, run: go('/settings/') },
    { id: 'history', label: '履歴', hint: hint('open_history'), icon: History, run: go('/history/') },
    { id: 'plugins', label: 'プラグイン管理', icon: Puzzle, run: go('/plugins/') },
    { id: 'quit', label: 'MoonLens 終了', icon: Power, run: () => quitApp() }
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
        placeholder="コマンド検索"
        class="w-full rounded-t-ml border-b border-border bg-transparent px-4 py-3 text-sm focus:outline-none"
      />
      <ul class="scrollbar-minimal max-h-80 overflow-auto">
        {#each filtered as c (c.id)}
          <li>
            <button
              class="flex w-full items-center justify-between px-4 py-2 text-left text-sm transition hover:bg-surface-muted disabled:opacity-50"
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
          <li class="px-4 py-6 text-center text-sm text-ink-faint">該当なし</li>
        {/each}
      </ul>
      {#if error}
        <p class="border-t border-border px-4 py-2 text-xs break-words text-red-500">{error}</p>
      {/if}
    </div>
  </div>
{/if}
