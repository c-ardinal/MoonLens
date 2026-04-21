<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    Camera,
    Puzzle,
    History,
    Focus,
    Settings as SettingsIcon,
    Mic,
    MicOff,
    Image,
    OctagonX
  } from 'lucide-svelte';
  import {
    captureAndProcess,
    newLensWindow,
    runReverseImageSearch,
    startSubtitle,
    stopAll,
    stopSubtitle,
    subtitleState
  } from '$lib/ipc';

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

  async function runCapture() {
    open = false;
    await captureAndProcess();
  }

  async function runOpenLens() {
    open = false;
    await newLensWindow();
  }

  async function copyAndSearchClipboard() {
    open = false;
    const text = await navigator.clipboard.readText().catch(() => '');
    if (!text) return;
    // Treat clipboard text as a URL/ID for reverse image search —
    // opens the default search engine in the user's browser.
    await runReverseImageSearch(text);
  }

  async function toggleSub() {
    open = false;
    if (subtitleRunning) await stopSubtitle();
    else await startSubtitle();
    subtitleRunning = !subtitleRunning;
  }

  async function panicStopAll() {
    open = false;
    await stopAll();
  }

  const commands: Command[] = $derived([
    {
      id: 'capture',
      label: '領域をキャプチャ',
      hint: '⌘⇧Space',
      icon: Camera,
      run: runCapture
    },
    { id: 'lens', label: 'Lens ウィンドウを開く', icon: Focus, run: runOpenLens },
    {
      id: 'subtitle',
      label: subtitleRunning ? 'ライブ字幕を停止' : 'ライブ字幕を開始',
      hint: '⌘⇧S',
      icon: subtitleRunning ? MicOff : Mic,
      run: toggleSub
    },
    {
      id: 'reverse',
      label: 'クリップボードの URL で画像検索',
      icon: Image,
      run: copyAndSearchClipboard
    },
    {
      id: 'settings',
      label: '設定を開く',
      hint: '⌘,',
      icon: SettingsIcon,
      run: () => {
        open = false;
        goto('/settings/');
      }
    },
    {
      id: 'history',
      label: '履歴を開く',
      hint: '⌘⇧H',
      icon: History,
      run: () => {
        open = false;
        goto('/history/');
      }
    },
    {
      id: 'plugins',
      label: 'プラグインを管理',
      icon: Puzzle,
      run: () => {
        open = false;
        goto('/plugins/');
      }
    },
    {
      id: 'stop_all',
      label: '全停止 (Lens・字幕・Overlay)',
      hint: '⌘⌥Esc',
      icon: OctagonX,
      run: panicStopAll
    }
  ]);

  const filtered = $derived(
    commands.filter((c) => c.label.toLowerCase().includes(q.toLowerCase()))
  );

  async function runWith(cmd: Command) {
    if (running) return;
    running = true;
    try {
      await cmd.run();
    } finally {
      running = false;
    }
  }

  onMount(() => {
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
        open = !open;
        if (open) q = '';
      } else if (e.key === 'Escape') {
        open = false;
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  });
</script>

{#if open}
  <div
    class="fixed inset-0 z-50 flex items-start justify-center bg-black/30 p-16 backdrop-blur-sm"
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
      <input
        bind:value={q}
        placeholder="コマンドを検索…"
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
          <li class="px-4 py-6 text-center text-sm text-ink-faint">該当するコマンドなし</li>
        {/each}
      </ul>
    </div>
  </div>
{/if}
