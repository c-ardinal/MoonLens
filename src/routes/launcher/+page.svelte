<!--
  Mini launcher window — a Spotlight-style quick-translate panel.
  Shown via the tray menu's "クリップボードを翻訳" item or the
  open_command_palette global hotkey. Auto-loads the clipboard's
  current text on activation so the common case is one keystroke
  away.

  Visually matches the Claude Design 2026.05 MiniLauncher mock:
  translucent rounded frame, search-style input row with logo, large
  translation result body, and a kbd-hint footer.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Cpu, Save, ArrowRight, Loader2, Settings } from 'lucide-svelte';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import MoonLensMark from '$lib/components/MoonLensMark.svelte';
  import {
    addHistoryEntry,
    formatLanguagePair,
    getDefaultLanguagePair,
    hideLauncher,
    runTranslate,
    showMainRoute,
  } from '$lib/ipc';

  let source = $state('');
  let result = $state('');
  /** User's default source/target codes from settings, resolved at
   *  mount and refreshed when the launcher regains focus. Initialised
   *  with the same defaults as the backend so the pill never renders
   *  blank during the first paint. */
  let sourceLang = $state('auto');
  let targetLang = $state('ja');
  /** Pre-formatted pair string for the header pill. Recomputed when
   *  either code changes via the $derived runes below. */
  const languagePair = $derived(formatLanguagePair(sourceLang, targetLang));
  /** Provider name reported by the backend (`embedded.llm`, `deepl`,
   *  etc.). Used for the history record and for the footer label so
   *  the user can see which chain link actually fired. */
  let lastProvider = $state<string | null>(null);
  /** True when the last translate completed via a cloud provider.
   *  Forwarded to the history entry so the privacy badge in the
   *  history list is accurate. */
  let lastViaCloud = $state(false);
  let elapsedMs = $state<number | null>(null);
  let translating = $state(false);
  let error = $state<string | null>(null);
  let savedFlash = $state(false);
  let savedErrorFlash = $state<string | null>(null);
  let inputEl: HTMLInputElement | undefined;
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  /** Tracks the most recently saved (source, result) pair so a
   *  rapid double-tap of ⌘S doesn't duplicate the same entry. The
   *  flash UX still fires so the user gets feedback either way. */
  let lastSavedKey = '';

  const win = getCurrentWebviewWindow();

  /** Pull the clipboard into the input on first show. Subsequent
   *  shows (hide → re-show via tray) re-poll so the user always sees
   *  the latest copied text, but a non-empty pending input is kept
   *  so the user doesn't lose freehand typing if they tab away and
   *  back. */
  async function pullClipboard(): Promise<void> {
    try {
      const txt = await navigator.clipboard.readText();
      if (typeof txt === 'string' && txt.length > 0 && source.length === 0) {
        source = txt;
        scheduleTranslate();
      }
    } catch {
      // best-effort — clipboard permission may not be granted in the
      // launcher webview yet (first-run prompt suppressed)
    }
  }

  function scheduleTranslate(): void {
    if (debounceTimer !== undefined) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => translate(), 220);
  }

  async function translate(): Promise<void> {
    if (translating) return;
    const text = source.trim();
    if (!text) {
      result = '';
      elapsedMs = null;
      lastProvider = null;
      lastViaCloud = false;
      return;
    }
    translating = true;
    error = null;
    const started = performance.now();
    try {
      const res = (await runTranslate({
        text,
        source_language: sourceLang,
        target_language: targetLang,
      })) as {
        text?: string;
        used_provider?: string;
        via_cloud?: boolean;
      };
      result = res.text ?? '';
      lastProvider = res.used_provider ?? null;
      lastViaCloud = Boolean(res.via_cloud);
      elapsedMs = Math.round(performance.now() - started);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      result = '';
      lastProvider = null;
      lastViaCloud = false;
    } finally {
      translating = false;
    }
  }

  async function copyResult(): Promise<boolean> {
    if (!result) return false;
    try {
      await navigator.clipboard.writeText(result);
      return true;
    } catch {
      savedErrorFlash = 'コピー失敗';
      setTimeout(() => (savedErrorFlash = null), 1500);
      return false;
    }
  }

  function flashSaved(): void {
    savedFlash = true;
    setTimeout(() => (savedFlash = false), 1200);
  }

  /** Persist the current translation to the history store. The
   *  storage layer auto-stamps `created_at`, so duplicates only
   *  matter for the user-perceived "did this already get saved?"
   *  question — we guard locally so back-to-back ⌘S doesn't write
   *  the same row twice. */
  async function saveToHistory(): Promise<void> {
    const src = source.trim();
    const dst = result.trim();
    if (!src || !dst) return;
    const key = `${src}${dst}`;
    if (key === lastSavedKey) {
      flashSaved();
      return;
    }
    try {
      await addHistoryEntry({
        kind: 'launcher',
        source_text: src,
        result_text: dst,
        provider: lastProvider,
        via_cloud: lastViaCloud,
      });
      lastSavedKey = key;
      flashSaved();
    } catch (e) {
      savedErrorFlash = e instanceof Error ? e.message : String(e);
      setTimeout(() => (savedErrorFlash = null), 1800);
    }
  }

  /** Hand off to the main app's settings page. The Rust side
   *  navigates the main webview and brings it to the foreground;
   *  we then close ourselves so the user doesn't have to chase the
   *  launcher off-screen. */
  async function openSettings(): Promise<void> {
    try {
      await showMainRoute('/settings/');
    } finally {
      void hideLauncher();
    }
  }

  function handleKey(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      void hideLauncher();
    } else if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void copyResult().then((ok) => {
        if (ok) void hideLauncher();
      });
    } else if (e.key === 's' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void saveToHistory();
    }
  }

  function onInput(): void {
    scheduleTranslate();
  }

  async function refreshLanguagePair(): Promise<void> {
    const pair = await getDefaultLanguagePair();
    sourceLang = pair.source;
    targetLang = pair.target;
  }

  onMount(() => {
    inputEl?.focus();
    void refreshLanguagePair();
    void pullClipboard();
    window.addEventListener('keydown', handleKey);
    // Re-pull the clipboard when the window regains focus, and
    // auto-hide when it loses focus — standard Spotlight behaviour.
    // The 120ms delay before hiding lets click-throughs on our own
    // footer buttons complete instead of being eaten by the focus
    // change.
    let blurTimer: ReturnType<typeof setTimeout> | undefined;
    const unlistenPromise = win.onFocusChanged((event) => {
      if (blurTimer !== undefined) clearTimeout(blurTimer);
      if (event.payload === true) {
        // The user may have changed the default pair in Settings
        // since the launcher was last open — re-pull on every focus
        // gain so the pill never drifts from the actual translate.
        void refreshLanguagePair();
        void pullClipboard();
        inputEl?.focus();
      } else {
        blurTimer = setTimeout(() => {
          void hideLauncher();
        }, 120);
      }
    });
    return () => {
      window.removeEventListener('keydown', handleKey);
      if (blurTimer !== undefined) clearTimeout(blurTimer);
      void unlistenPromise.then((u) => u());
    };
  });

  onDestroy(() => {
    if (debounceTimer !== undefined) clearTimeout(debounceTimer);
  });
</script>

<div class="launcher" role="dialog" aria-labelledby="launcher-title" aria-modal="true">
  <div class="launcher-shell" data-tauri-drag-region>
    <!-- Input row -->
    <div class="launcher-input-row">
      <MoonLensMark size={18} />
      <input
        bind:this={inputEl}
        bind:value={source}
        oninput={onInput}
        class="launcher-input"
        type="text"
        spellcheck="false"
        autocomplete="off"
        placeholder="翻訳したいテキストを入力 (クリップボードから自動入力)"
        aria-label="翻訳元テキスト"
      />
      <span class="launcher-pair">
        {#if translating}
          <Loader2 class="h-3 w-3 launcher-spin" aria-hidden="true" />
        {/if}
        {languagePair}
      </span>
    </div>

    <!-- Result body -->
    <div class="launcher-body">
      <div class="launcher-kicker" id="launcher-title">翻訳結果</div>
      {#if error}
        <div class="launcher-error" role="alert">{error}</div>
      {:else if result}
        <div class="launcher-result">{result}</div>
      {:else if translating}
        <div class="launcher-placeholder">翻訳中…</div>
      {:else}
        <div class="launcher-placeholder">
          テキストを入力すると、ここに翻訳が表示されます
        </div>
      {/if}
    </div>

    <!-- Footer with kbd hints -->
    <footer class="launcher-foot">
      <button
        type="button"
        class="launcher-foot-action"
        onclick={() =>
          void copyResult().then((ok) => {
            if (ok) void hideLauncher();
          })}
        disabled={!result}
        title="翻訳結果をコピーして閉じる"
      >
        <span class="launcher-kbd">↵</span>
        コピー
        <ArrowRight class="h-3 w-3" aria-hidden="true" />
      </button>
      <span class="launcher-foot-sep" aria-hidden="true"></span>
      <button
        type="button"
        class="launcher-foot-action"
        onclick={() => void saveToHistory()}
        disabled={!result}
        title="翻訳結果を履歴に保存"
      >
        <span class="launcher-kbd">⌘</span>
        <span class="launcher-kbd">S</span>
        履歴に保存
        <Save class="h-3 w-3" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="launcher-foot-action"
        onclick={openSettings}
        title="設定を開く"
      >
        <Settings class="h-3 w-3" aria-hidden="true" />
        設定
      </button>
      <span class="launcher-foot-spacer"></span>
      {#if lastProvider}
        <span class="launcher-provider mono" title="使用したプロバイダ">
          {lastProvider}
        </span>
        <span class="launcher-foot-sep" aria-hidden="true"></span>
      {/if}
      <Cpu class="h-3 w-3" aria-hidden="true" />
      {#if elapsedMs !== null}
        <span class="mono">{elapsedMs} ms</span>
      {:else}
        <span class="mono">—</span>
      {/if}
      {#if savedFlash}
        <span class="launcher-saved-flash">保存しました</span>
      {/if}
      {#if savedErrorFlash}
        <span class="launcher-saved-error">{savedErrorFlash}</span>
      {/if}
    </footer>
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
  }

  .launcher {
    width: 100vw;
    height: 100vh;
    display: grid;
    place-items: center;
    padding: 8px;
    box-sizing: border-box;
  }

  .launcher-shell {
    width: 100%;
    max-width: 504px;
    border-radius: 14px;
    background: rgba(15, 17, 21, 0.78);
    backdrop-filter: blur(34px) saturate(160%);
    -webkit-backdrop-filter: blur(34px) saturate(160%);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow:
      0 30px 80px rgba(0, 0, 0, 0.6),
      0 0 0 1px rgba(255, 255, 255, 0.04) inset;
    overflow: hidden;
    color: #e5e7eb;
    font-family: var(--font-sans, 'Geist', 'Noto Sans JP', sans-serif);
    animation: launcher-pop 220ms cubic-bezier(0.22, 0.61, 0.36, 1);
  }
  @keyframes launcher-pop {
    from {
      transform: translateY(-6px) scale(0.98);
      opacity: 0;
    }
    to {
      transform: translateY(0) scale(1);
      opacity: 1;
    }
  }

  /* Input row */
  .launcher-input-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    -webkit-app-region: no-drag;
  }
  .launcher-input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    color: #fff;
    font-size: 14px;
    font-family: inherit;
    padding: 0;
  }
  .launcher-input::placeholder {
    color: #7a808a;
  }
  .launcher-pair {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 9px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    color: #aab1bb;
    font-size: 11px;
    font-weight: 500;
    font-family: var(--font-mono, 'Geist Mono', monospace);
    letter-spacing: 0.02em;
    flex: 0 0 auto;
  }
  :global(.launcher-spin) {
    animation: launcher-spin 0.9s linear infinite;
  }
  @keyframes launcher-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Result body */
  .launcher-body {
    padding: 16px 18px;
    min-height: 64px;
  }
  .launcher-kicker {
    font-size: 10.5px;
    color: #7a808a;
    letter-spacing: 0.08em;
    margin-bottom: 6px;
    font-weight: 500;
    text-transform: uppercase;
  }
  .launcher-result {
    font-size: 17px;
    line-height: 1.5;
    font-weight: 500;
    font-family: 'Noto Sans JP', var(--font-sans, sans-serif);
    color: #fff;
    word-break: break-word;
    overflow-wrap: anywhere;
  }
  .launcher-placeholder {
    font-size: 13.5px;
    color: #7a808a;
    line-height: 1.5;
  }
  .launcher-error {
    font-size: 12.5px;
    padding: 8px 10px;
    border-radius: 6px;
    background: rgba(216, 122, 122, 0.14);
    border: 1px solid rgba(216, 122, 122, 0.35);
    color: #f1a1a1;
  }

  /* Footer */
  .launcher-foot {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    font-size: 10.5px;
    color: #7a808a;
    -webkit-app-region: no-drag;
  }
  .launcher-foot-action {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border-radius: 5px;
    background: transparent;
    border: 0;
    color: inherit;
    font-size: 10.5px;
    font-family: inherit;
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease;
  }
  .launcher-foot-action:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
  }
  .launcher-foot-action:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .launcher-foot-sep {
    width: 1px;
    height: 10px;
    background: rgba(255, 255, 255, 0.1);
  }
  .launcher-foot-spacer {
    flex: 1;
  }
  .launcher-kbd {
    display: inline-flex;
    align-items: center;
    padding: 1px 5px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-bottom-width: 2px;
    font-family: var(--font-mono, 'Geist Mono', monospace);
    font-size: 10px;
    color: #c8cacd;
    line-height: 1.3;
  }
  .launcher-provider {
    color: #aab1bb;
    font-size: 10px;
    letter-spacing: 0.02em;
  }
  .launcher-saved-flash {
    color: #7dd3a0;
    font-weight: 500;
    animation: launcher-fade 1.2s ease forwards;
  }
  .launcher-saved-error {
    color: #f1a1a1;
    font-weight: 500;
    animation: launcher-fade 1.8s ease forwards;
  }
  @keyframes launcher-fade {
    0% { opacity: 0; transform: translateY(2px); }
    20% { opacity: 1; transform: translateY(0); }
    80% { opacity: 1; }
    100% { opacity: 0; }
  }
  .mono {
    font-family: var(--font-mono, 'Geist Mono', monospace);
  }
</style>
