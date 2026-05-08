<script lang="ts">
  /**
   * Sectioned 232px-wide sidebar matching the Claude Design v2026.05
   * spec: brand block at top, sectioned nav, footer engine status.
   * Falls back to a 56px icon-only rail when collapsed (kept from the
   * previous shell so users who liked the compact mode keep it).
   */
  import { page } from '$app/stores';
  import { goto } from '$lib/navigation';
  import {
    Home,
    Camera,
    Focus,
    Mic,
    History,
    Puzzle,
    Settings as SettingsIcon,
    PanelLeft,
    PanelLeftClose,
    OctagonX,
    Cpu
  } from 'lucide-svelte';
  import {
    listLensWindows,
    onSubtitle,
    stopAll,
    subtitleState
  } from '$lib/ipc';
  import { onDestroy, onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import MoonLensMark from './MoonLensMark.svelte';

  type NavEntry =
    | { kind: 'item'; href: string; label: string; icon: typeof Home; hint?: string; badge?: string }
    | { kind: 'section'; label: string };

  const items: NavEntry[] = [
    { kind: 'section', label: '機能' },
    { kind: 'item', href: '/', label: 'ホーム', icon: Home, hint: '⌘1' },
    { kind: 'item', href: '/capture/', label: '画像翻訳', icon: Camera, hint: '⌘⇧S' },
    { kind: 'item', href: '/lens/', label: 'Lensウィンドウ', icon: Focus, hint: '⌘L' },
    { kind: 'item', href: '/audio/', label: '音声字幕', icon: Mic, hint: '⌘⇧A' },
    { kind: 'section', label: 'ライブラリ' },
    { kind: 'item', href: '/history/', label: '翻訳履歴', icon: History, hint: '⌘⇧H' },
    { kind: 'item', href: '/plugins/', label: 'プラグイン', icon: Puzzle },
    { kind: 'section', label: 'システム' },
    { kind: 'item', href: '/settings/', label: '設定', icon: SettingsIcon, hint: '⌘,' }
  ];

  const STORAGE_KEY = 'moonlens.sidebar.collapsed';

  let collapsed = $state(false);
  let stopping = $state(false);
  let stopReport = $state<string | null>(null);
  let stopReportTimer: ReturnType<typeof setTimeout> | undefined;
  let lensCount = $state(0);
  let subtitleRunning = $state(false);
  let appVersion = $state<string | null>(null);
  let unlistenSubtitle: (() => void) | undefined;
  let lensPollTimer: ReturnType<typeof setInterval> | undefined;

  // The footer status is the only place inside the chrome where the
  // user sees "is the engine actually doing anything right now". Keep
  // it lightweight: poll the lens registry every two seconds (cheap
  // Vec clone) and subscribe to the subtitle event stream so we
  // reflect tray-menu / hotkey toggles immediately.
  onMount(async () => {
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = null;
    }
    try {
      subtitleRunning = await subtitleState();
    } catch {
      subtitleRunning = false;
    }
    try {
      const all = await listLensWindows();
      lensCount = all.length;
    } catch {
      lensCount = 0;
    }
    unlistenSubtitle = await onSubtitle((event) => {
      if (event.kind === 'started') subtitleRunning = true;
      else if (event.kind === 'stopped') subtitleRunning = false;
    });
    lensPollTimer = setInterval(async () => {
      try {
        lensCount = (await listLensWindows()).length;
      } catch {
        /* keep last value */
      }
    }, 2_000);
  });

  onDestroy(() => {
    unlistenSubtitle?.();
    if (lensPollTimer !== undefined) clearInterval(lensPollTimer);
  });

  const engineActive = $derived(subtitleRunning || lensCount > 0);
  const engineMeta = $derived.by(() => {
    if (subtitleRunning && lensCount > 0) return `Lens ${lensCount} · 字幕`;
    if (lensCount > 0) return `Lens ${lensCount}`;
    if (subtitleRunning) return '字幕';
    return '待機';
  });

  $effect(() => {
    if (typeof window === 'undefined') return;
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved === '1') collapsed = true;
  });

  function toggle() {
    collapsed = !collapsed;
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(STORAGE_KEY, collapsed ? '1' : '0');
    }
  }

  function isActive(href: string): boolean {
    const pathname = $page.url.pathname;
    if (href === '/') return pathname === '/';
    return pathname.startsWith(href);
  }

  function navigate(href: string) {
    goto(href);
  }

  async function panicStopAll() {
    if (stopping) return;
    stopping = true;
    try {
      const r = await stopAll();
      const parts: string[] = [];
      if (r.lens_count > 0) parts.push(`Lens ${r.lens_count}件`);
      if (r.subtitle_was_running) parts.push('字幕');
      if (r.overlay_was_open) parts.push('Overlay');
      stopReport = parts.length === 0 ? '何も動いていません' : `${parts.join('・')} を停止`;
    } catch (e) {
      stopReport = `停止失敗: ${e}`;
    } finally {
      stopping = false;
      if (stopReportTimer) clearTimeout(stopReportTimer);
      stopReportTimer = setTimeout(() => (stopReport = null), 3000);
    }
  }
</script>

<aside class="ml-sidebar" class:collapsed aria-label="主ナビゲーション">
  <div class="ml-brand">
    <span class="ml-brand-mark">
      <MoonLensMark size={collapsed ? 22 : 22} />
    </span>
    {#if !collapsed}
      <span class="ml-brand-name">MoonLens</span>
      {#if appVersion}
        <span class="ml-brand-ver">v{appVersion}</span>
      {/if}
    {/if}
  </div>

  <nav class="ml-nav">
    {#each items as entry, i (i)}
      {#if entry.kind === 'section'}
        {#if !collapsed}
          <div class="ml-nav-section">{entry.label}</div>
        {:else}
          <div class="ml-nav-spacer" aria-hidden="true"></div>
        {/if}
      {:else}
        {@const active = isActive(entry.href)}
        <button
          type="button"
          class="ml-nav-item"
          class:active
          aria-current={active ? 'page' : undefined}
          onclick={() => navigate(entry.href)}
          title={collapsed ? entry.label : undefined}
        >
          <span class="ml-nav-icon">
            <entry.icon class="h-4 w-4" aria-hidden="true" />
          </span>
          {#if !collapsed}
            <span class="ml-nav-label">{entry.label}</span>
            {#if entry.badge}
              <span class="ml-nav-badge">{entry.badge}</span>
            {:else if entry.hint}
              <span class="ml-nav-kbd">{entry.hint}</span>
            {/if}
          {/if}
        </button>
      {/if}
    {/each}
  </nav>

  <div class="ml-side-footer">
    {#if !collapsed}
      <div class="ml-engine-status" title={engineActive ? 'ローカルエンジン稼働中' : 'ローカルエンジン待機中'}>
        <span class="ml-status-dot" class:cold={!engineActive}></span>
        <span>ローカルエンジン</span>
        <span class="ml-engine-meta">{engineMeta}</span>
      </div>
    {:else}
      <div class="ml-engine-status compact" title={engineActive ? 'ローカルエンジン稼働中' : 'ローカルエンジン待機中'}>
        <Cpu class="h-3 w-3" aria-hidden="true" />
        <span class="ml-status-dot" class:cold={!engineActive}></span>
      </div>
    {/if}

    <button
      type="button"
      class="ml-foot-btn ml-stop"
      onclick={panicStopAll}
      disabled={stopping}
      aria-label="全停止"
      title={collapsed ? '全停止' : '全 Lens + 字幕 + Overlay を停止 (⌘⌥Esc)'}
    >
      <OctagonX class="h-4 w-4" aria-hidden="true" />
      {#if !collapsed}
        <span>{stopping ? '停止中…' : '全停止'}</span>
        <span class="ml-nav-kbd">⌘⌥Esc</span>
      {/if}
    </button>

    {#if stopReport && !collapsed}
      <p class="ml-stop-report" aria-live="polite">{stopReport}</p>
    {/if}

    <button
      type="button"
      class="ml-foot-btn"
      onclick={toggle}
      aria-label={collapsed ? 'サイドバーを展開' : 'サイドバーを折り畳む'}
      title={collapsed ? '展開' : '折り畳む'}
    >
      {#if collapsed}
        <PanelLeft class="h-4 w-4" aria-hidden="true" />
      {:else}
        <PanelLeftClose class="h-4 w-4" aria-hidden="true" />
        <span>折り畳む</span>
      {/if}
    </button>
  </div>
</aside>

<style>
  .ml-sidebar {
    width: 232px;
    background: var(--bg-1);
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    padding: var(--s-3) var(--s-2);
    gap: var(--s-1);
    min-height: 0;
    flex: 0 0 232px;
    transition: width var(--dur-base) var(--ease-out);
  }
  .ml-sidebar.collapsed {
    width: 56px;
    flex: 0 0 56px;
  }

  .ml-brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 14px;
  }
  .ml-brand-mark {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    flex: 0 0 26px;
  }
  .ml-brand-name {
    font-weight: 600;
    font-size: 14px;
    letter-spacing: -0.01em;
    color: var(--ink-0);
  }
  .ml-brand-ver {
    font-size: 10.5px;
    color: var(--ink-3);
    font-family: var(--font-mono);
    margin-left: auto;
    padding-top: 3px;
  }

  .ml-nav {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-top: 4px;
  }

  .ml-nav-spacer {
    height: 8px;
  }

  .ml-nav-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ml-side-footer {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 6px 4px;
  }
  .ml-engine-status {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--r-md);
    background: var(--bg-2);
    border: 1px solid var(--line);
    font-size: 11.5px;
    color: var(--ink-1);
  }
  .ml-engine-status.compact {
    justify-content: center;
    padding: 6px;
  }
  .ml-status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #5bb97d;
    box-shadow: 0 0 0 3px rgba(91, 185, 125, 0.15);
  }
  .ml-status-dot.cold {
    background: var(--ink-3);
    box-shadow: none;
  }
  .ml-engine-meta {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ink-3);
    margin-left: auto;
  }

  .ml-foot-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: var(--r-sm);
    color: var(--ink-1);
    font-size: 12px;
    font-weight: 450;
    background: transparent;
    border: 0;
    cursor: pointer;
    text-align: left;
    transition: background var(--dur-fast), color var(--dur-fast);
  }
  .ml-foot-btn:hover:not(:disabled) {
    background: var(--bg-2);
    color: var(--ink-0);
  }
  .ml-foot-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .ml-foot-btn.ml-stop:hover:not(:disabled) {
    color: #d87a7a;
    background: rgba(216, 122, 122, 0.08);
  }
  .ml-foot-btn span {
    flex: 1;
  }

  .ml-stop-report {
    margin: 0 4px;
    padding: 4px 8px;
    border-radius: var(--r-sm);
    background: var(--bg-2);
    color: var(--ink-2);
    font-size: 10.5px;
  }
</style>
