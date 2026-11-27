<script lang="ts">
  import { t } from '$lib/i18n';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    Camera,
    Puzzle,
    History,
    Home,
    Clapperboard,
    Image as ImageIcon,
    Languages,
    Settings as SettingsIcon,
    Mic,
    MicOff,
    Pause,
    Power,
    LayoutGrid,
    Save
  } from 'lucide-svelte';
  import {
    getSettings,
    onHotkey,
    quitApp,
    openLens,
    listLayoutSets,
    restoreLayoutSet,
    saveLayoutSet,
    startSubtitle,
    stopSubtitle,
    subtitleState,
    toggleLensPause
  } from '$lib/ipc';
  import { displayAccelerator } from '$lib/hotkey';
  import { errorText } from '$lib/errors';
  import { notify } from '$lib/toast.svelte';
  import type { LayoutSet } from '$lib/layout-sets';

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

  /** 保存済みの配置セット（表示時に取得）。 */
  let layoutSets = $state<LayoutSet[]>([]);

  async function saveLayout() {
    const saved = await saveLayoutSet();
    open = false;
    notify(t('common:layout_saved', { name: saved.name, n: saved.windows.length }));
  }

  async function restoreLayout(set: LayoutSet) {
    const count = await restoreLayoutSet(set.id);
    open = false;
    notify(t('common:layout_restored', { name: set.name, n: count }));
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
      label: t('common:window_translation'),
      hint: hint('capture_region'),
      icon: Camera,
      run: openWindow
    },
    {
      id: 'window-pause',
      label: t('common:window_translation_pause_resume'),
      hint: hint('toggle_window_pause'),
      icon: Pause,
      run: async () => {
        open = false;
        await toggleLensPause();
      }
    },
    {
      id: 'subtitle',
      label: subtitleRunning ? t('command-palette:stop_audio_translation') : t('command-palette:start_audio_translation'),
      hint: hint('toggle_subtitle'),
      icon: subtitleRunning ? MicOff : Mic,
      run: toggleSub
    },
    { id: 'layout-save', label: t('command-palette:save_layout_set'), icon: Save, run: saveLayout },
    ...layoutSets.map((set) => ({
      id: `layout-restore:${set.id}`,
      label: t('command-palette:restore_layout_set', { name: set.name }),
      icon: LayoutGrid,
      run: () => restoreLayout(set)
    })),
    { id: 'home', label: t('common:home'), icon: Home, run: go('/') },
    { id: 'image', label: t('common:image_translation'), icon: ImageIcon, run: go('/image/') },
    { id: 'translate', label: t('common:text_translation'), icon: Languages, run: go('/translate/') },
    { id: 'file', label: t('common:file_subtitles'), icon: Clapperboard, run: go('/file/') },
    { id: 'settings', label: t('common:settings'), icon: SettingsIcon, run: go('/settings/') },
    { id: 'history', label: t('common:history'), hint: hint('open_history'), icon: History, run: go('/history/') },
    { id: 'plugins', label: t('command-palette:manage_extensions'), icon: Puzzle, run: go('/settings/#section-plugins') },
    { id: 'quit', label: t('command-palette:quit_moonlens'), icon: Power, run: () => quitApp() }
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
    listLayoutSets()
      .then((sets) => (layoutSets = sets))
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
        placeholder={t('command-palette:search_commands')}
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
          <li class="px-4 py-6 text-center text-sm text-ink-faint">{t('common:no_results')}</li>
        {/each}
      </ul>
      {#if error}
        <p class="border-t border-border px-4 py-2 text-xs break-words text-red-500">{error}</p>
      {/if}
    </div>
  </div>
{/if}
