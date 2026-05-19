<!--
  Standalone text-translation page modelled after DeepL / Google
  Translate's two-pane layout. Routes through the same `run_translate`
  IPC the mini launcher uses, so all chain logic (provider routing,
  privacy gates, fallbacks) is shared.

  Differences from the launcher:
  - Full-page sized for arbitrary paste-in text (textarea grows)
  - Language pair selector with swap button
  - Manual translate button + opt-in auto-translate (debounced)
  - Persists last-used language pair in localStorage so a multilingual
    user doesn't re-pick on every visit
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    Languages,
    ArrowLeftRight,
    Copy,
    Check,
    X as XIcon,
    Loader2,
    AlertCircle,
  } from 'lucide-svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Pill from '$lib/components/Pill.svelte';
  import Toggle from '$lib/components/Toggle.svelte';
  import {
    addHistoryEntry,
    getDefaultLanguagePair,
    runTranslate,
  } from '$lib/ipc';
  import { extractText } from '$lib/result';

  /** Language pickers — kept short so the dropdown stays glanceable.
   *  Source allows `auto` (script detector on the backend decides per
   *  flow), target does not — picking `auto` as the destination is
   *  always a user error. */
  interface LangEntry {
    code: string;
    label: string;
  }
  const SOURCE_LANGS: LangEntry[] = [
    { code: 'auto', label: '自動検出' },
    { code: 'ja', label: '日本語' },
    { code: 'en', label: '英語' },
    { code: 'zh', label: '中国語' },
    { code: 'ko', label: '韓国語' },
    { code: 'fr', label: 'フランス語' },
    { code: 'de', label: 'ドイツ語' },
    { code: 'es', label: 'スペイン語' },
    { code: 'it', label: 'イタリア語' },
    { code: 'pt', label: 'ポルトガル語' },
    { code: 'ru', label: 'ロシア語' },
  ];
  const TARGET_LANGS: LangEntry[] = SOURCE_LANGS.filter((l) => l.code !== 'auto');

  const MAX_CHARS = 5000;
  const STORAGE_KEY = 'moonlens.translate.langpair';
  const AUTO_STORAGE_KEY = 'moonlens.translate.autoTranslate';
  const DEBOUNCE_MS = 600;

  let sourceLang = $state('auto');
  let targetLang = $state('ja');
  let inputText = $state('');
  let resultText = $state('');
  let translating = $state(false);
  let error = $state<string | null>(null);
  let lastProvider = $state<string | null>(null);
  let lastViaCloud = $state(false);
  let lastLatencyMs = $state<number | null>(null);
  let autoTranslate = $state(false);
  let copyHint = $state(false);
  let copyErrorHint = $state<string | null>(null);
  let copyHintTimer: ReturnType<typeof setTimeout> | undefined;
  let copyErrorTimer: ReturnType<typeof setTimeout> | undefined;
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  /** Guards against duplicate auto-history rows when the same input
   *  produces the same translation twice in a row (auto-translate
   *  debounce can re-fire if the user clicks the button again). The
   *  history store is best-effort; a duplicate row isn't fatal but
   *  pollutes the list. */
  let lastSavedKey = '';

  const charCount = $derived(inputText.length);
  const overLimit = $derived(charCount > MAX_CHARS);
  const canTranslate = $derived(
    !translating && inputText.trim().length > 0 && !overLimit,
  );

  function loadPreferences(): void {
    if (typeof window === 'undefined') return;
    try {
      const raw = window.localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as { source?: string; target?: string };
        if (typeof parsed.source === 'string') sourceLang = parsed.source;
        if (typeof parsed.target === 'string') targetLang = parsed.target;
      }
      autoTranslate = window.localStorage.getItem(AUTO_STORAGE_KEY) === '1';
    } catch {
      // localStorage may be unavailable in restricted contexts — fall
      // back to settings defaults silently.
    }
  }

  function savePreferences(): void {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ source: sourceLang, target: targetLang }),
      );
      window.localStorage.setItem(AUTO_STORAGE_KEY, autoTranslate ? '1' : '0');
    } catch {
      // ignore
    }
  }

  onMount(async () => {
    // Settings provide the initial defaults; localStorage (if present)
    // overrides them so a returning user keeps their last-used pair.
    try {
      const pair = await getDefaultLanguagePair();
      sourceLang = pair.source;
      targetLang = pair.target;
    } catch {
      // keep state defaults
    }
    loadPreferences();
  });

  onDestroy(() => {
    if (copyHintTimer) clearTimeout(copyHintTimer);
    if (copyErrorTimer) clearTimeout(copyErrorTimer);
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  function scheduleAutoTranslate(): void {
    if (!autoTranslate) return;
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      void translate();
    }, DEBOUNCE_MS);
  }

  async function translate(): Promise<void> {
    if (translating) return;
    const text = inputText.trim();
    if (!text) {
      resultText = '';
      lastProvider = null;
      lastViaCloud = false;
      lastLatencyMs = null;
      error = null;
      return;
    }
    if (overLimit) {
      error = `入力が長すぎます (${charCount} / ${MAX_CHARS} 文字)`;
      return;
    }
    translating = true;
    error = null;
    const started = performance.now();
    try {
      const res = await runTranslate({
        text,
        source_language: sourceLang,
        target_language: targetLang,
      });
      // TaskOutput is a serde-tagged enum: a `Translate` task always
      // produces `{ Text: "<translated>" }` (note: capitalised key —
      // serde's default for unit-style variants). Tolerate the
      // streaming variant too so a future Translate-as-stream path
      // wouldn't silently break this page.
      resultText = extractText(res);
      lastProvider = res?.used_provider ?? null;
      lastViaCloud = Boolean(res?.via_cloud);
      lastLatencyMs =
        typeof res?.latency_ms === 'number'
          ? Math.round(res.latency_ms)
          : Math.round(performance.now() - started);
      // Auto-persist every successful translation. Earlier UX had an
      // explicit "save" button next to the copy icon which users
      // mis-clicked (the two icons sit side-by-side). Auto-save
      // matches the launcher's behaviour for capture flows and means
      // the translation page's history record is always complete.
      if (resultText.trim()) {
        await maybeSaveHistory(text, resultText);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      resultText = '';
      lastProvider = null;
      lastViaCloud = false;
      lastLatencyMs = null;
    } finally {
      translating = false;
    }
  }

  function swapLanguages(): void {
    // `auto` cannot be a target — when the source is `auto` we promote
    // the current target as the new source and pick something sensible
    // for the new target so the swap remains useful instead of being a
    // no-op the user has to back out of.
    if (sourceLang === 'auto') {
      sourceLang = targetLang;
      targetLang = 'ja';
      if (sourceLang === 'ja') targetLang = 'en';
    } else {
      const prevSource = sourceLang;
      sourceLang = targetLang;
      targetLang = prevSource;
    }
    // Also swap the text content so a quick reverse-translate is one
    // click away — this is the DeepL/Google convention.
    if (resultText) {
      const prevInput = inputText;
      inputText = resultText;
      resultText = prevInput;
    }
    savePreferences();
    scheduleAutoTranslate();
  }

  function clearInput(): void {
    inputText = '';
    resultText = '';
    error = null;
    lastProvider = null;
    lastViaCloud = false;
    lastLatencyMs = null;
    if (debounceTimer) clearTimeout(debounceTimer);
  }

  async function copyResult(): Promise<void> {
    if (!resultText) return;
    try {
      await navigator.clipboard.writeText(resultText);
      copyHint = true;
      if (copyHintTimer) clearTimeout(copyHintTimer);
      copyHintTimer = setTimeout(() => (copyHint = false), 1500);
    } catch (e) {
      copyErrorHint = `コピー失敗: ${e instanceof Error ? e.message : String(e)}`;
      if (copyErrorTimer) clearTimeout(copyErrorTimer);
      copyErrorTimer = setTimeout(() => (copyErrorHint = null), 1800);
    }
  }

  async function maybeSaveHistory(src: string, dst: string): Promise<void> {
    const srcTrim = src.trim();
    const dstTrim = dst.trim();
    if (!srcTrim || !dstTrim) return;
    const key = `${srcTrim}␟${dstTrim}`;
    if (key === lastSavedKey) return;
    try {
      await addHistoryEntry({
        kind: 'translate',
        source_text: srcTrim,
        result_text: dstTrim,
        provider: lastProvider,
        via_cloud: lastViaCloud,
      });
      lastSavedKey = key;
    } catch {
      // History write is best-effort — a failed write shouldn't
      // surface as a translate-page error since the translation
      // itself succeeded. The user can still copy the result.
    }
  }

  function onInputKeyDown(e: KeyboardEvent): void {
    // Ctrl/Cmd + Enter = translate (industry-standard shortcut)
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void translate();
    }
  }

  function onInput(): void {
    error = null;
    scheduleAutoTranslate();
  }

  function onLangChange(): void {
    savePreferences();
    if (autoTranslate && inputText.trim()) {
      scheduleAutoTranslate();
    }
  }

  function onAutoToggle(v: boolean): void {
    autoTranslate = v;
    savePreferences();
    if (v && inputText.trim()) scheduleAutoTranslate();
  }
</script>

<PageHeader title="テキスト翻訳" subtitle="貼り付けたテキストをそのまま翻訳">
  {#snippet actions()}
    <label class="auto-toggle" title="入力停止 0.6 秒後に自動で翻訳">
      <span class="auto-toggle-label">自動翻訳</span>
      <Toggle
        checked={autoTranslate}
        onChange={onAutoToggle}
        aria-label="自動翻訳"
      />
    </label>
  {/snippet}
</PageHeader>

<div class="ml-content translate-page">
  <div class="lang-bar fadein">
    <label class="lang-pick">
      <span class="lang-pick-label">元の言語</span>
      <select
        class="lang-select"
        bind:value={sourceLang}
        onchange={onLangChange}
      >
        {#each SOURCE_LANGS as l (l.code)}
          <option value={l.code}>{l.label}</option>
        {/each}
      </select>
    </label>

    <button
      class="swap-btn"
      type="button"
      onclick={swapLanguages}
      aria-label="言語を入れ替え"
      title="言語を入れ替え"
    >
      <ArrowLeftRight class="h-4 w-4" aria-hidden="true" />
    </button>

    <label class="lang-pick">
      <span class="lang-pick-label">翻訳先</span>
      <select
        class="lang-select"
        bind:value={targetLang}
        onchange={onLangChange}
      >
        {#each TARGET_LANGS as l (l.code)}
          <option value={l.code}>{l.label}</option>
        {/each}
      </select>
    </label>
  </div>

  <div class="panes fadein">
    <!-- Input pane -->
    <section class="pane">
      <header class="pane-head">
        <Languages class="h-3.5 w-3.5" aria-hidden="true" />
        <span class="pane-head-title">入力</span>
        <span class="char-count" class:over={overLimit}>
          {charCount.toLocaleString()} / {MAX_CHARS.toLocaleString()}
        </span>
        {#if inputText}
          <button
            class="pane-head-btn"
            type="button"
            onclick={clearInput}
            aria-label="入力をクリア"
            title="入力をクリア"
          >
            <XIcon class="h-3.5 w-3.5" aria-hidden="true" />
          </button>
        {/if}
      </header>
      <textarea
        class="pane-textarea"
        bind:value={inputText}
        oninput={onInput}
        onkeydown={onInputKeyDown}
        placeholder={'翻訳したいテキストを入力\n\n⌘/Ctrl+Enter で翻訳'}
        spellcheck="false"
        rows="14"
      ></textarea>
    </section>

    <!-- Result pane -->
    <section class="pane">
      <header class="pane-head">
        <Languages class="h-3.5 w-3.5" aria-hidden="true" />
        <span class="pane-head-title">翻訳</span>
        {#if lastProvider}
          <Pill>
            {lastProvider}
            {#if lastViaCloud}<span class="cloud-flag">cloud</span>{/if}
          </Pill>
        {/if}
        {#if lastLatencyMs !== null}
          <span class="latency mono small muted">{lastLatencyMs} ms</span>
        {/if}
        <div style="flex: 1"></div>
        <button
          class="pane-head-btn"
          type="button"
          onclick={copyResult}
          disabled={!resultText}
          aria-label="結果をコピー"
          title={copyHint ? 'コピー済み' : '結果をコピー'}
        >
          {#if copyHint}
            <Check class="h-3.5 w-3.5" aria-hidden="true" />
          {:else}
            <Copy class="h-3.5 w-3.5" aria-hidden="true" />
          {/if}
        </button>
      </header>
      <div class="pane-result" class:placeholder={!resultText && !translating}>
        {#if translating && !resultText}
          <div class="result-loading">
            <Loader2 class="h-4 w-4 spin" aria-hidden="true" />
            <span class="muted">翻訳中…</span>
          </div>
        {:else if error}
          <div class="result-error">
            <AlertCircle class="h-4 w-4" aria-hidden="true" />
            <span>{error}</span>
          </div>
        {:else if resultText}
          <p class="result-text">{resultText}</p>
        {:else}
          <span class="result-placeholder">
            ここに翻訳結果が表示されます
          </span>
        {/if}
      </div>
    </section>
  </div>

  <div class="action-bar fadein">
    <div class="action-left">
      {#if copyErrorHint}
        <span class="action-msg error" aria-live="polite">{copyErrorHint}</span>
      {:else if resultText}
        <span class="action-msg muted-msg" aria-live="polite">
          履歴に自動保存されます
        </span>
      {/if}
    </div>
    <button
      class="btn primary"
      type="button"
      onclick={translate}
      disabled={!canTranslate}
    >
      {#if translating}
        <Loader2 class="h-3.5 w-3.5 spin" aria-hidden="true" />
        翻訳中…
      {:else}
        <Languages class="h-3.5 w-3.5" aria-hidden="true" />
        翻訳する
        <span class="kbd-inline">⌘↵</span>
      {/if}
    </button>
  </div>
</div>

<style>
  .translate-page {
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
    padding: 0 28px 24px;
    min-height: 0;
  }

  .auto-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--ink-2);
    cursor: pointer;
  }
  .auto-toggle-label {
    user-select: none;
  }

  .lang-bar {
    display: flex;
    align-items: end;
    gap: 12px;
  }
  .lang-pick {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .lang-pick-label {
    font-size: 11px;
    color: var(--ink-3);
    letter-spacing: 0.02em;
  }
  .lang-select {
    width: 100%;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    background: var(--bg-1);
    color: var(--ink-0);
    font-size: 13px;
    cursor: pointer;
    transition: border-color var(--dur-fast), background var(--dur-fast);
  }
  .lang-select:hover {
    border-color: var(--ink-3);
  }
  .lang-select:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }
  .swap-btn {
    flex: 0 0 auto;
    width: 36px;
    height: 36px;
    margin-bottom: 1px;
    display: grid;
    place-items: center;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--bg-1);
    color: var(--ink-1);
    cursor: pointer;
    transition: background var(--dur-fast), color var(--dur-fast),
      border-color var(--dur-fast);
  }
  .swap-btn:hover {
    background: var(--bg-2);
    color: var(--ink-0);
    border-color: var(--ink-3);
  }

  .panes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    min-height: 0;
  }
  @media (max-width: 760px) {
    .panes {
      grid-template-columns: 1fr;
    }
  }
  .pane {
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    overflow: hidden;
    min-height: 0;
  }
  .pane-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    background: var(--bg-2);
    color: var(--ink-2);
    font-size: 12px;
    flex-wrap: wrap;
  }
  .pane-head-title {
    font-weight: 500;
    color: var(--ink-1);
  }
  .char-count {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-3);
  }
  .char-count.over {
    color: #d87a7a;
  }
  .pane-head-btn {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: var(--r-sm);
    border: 0;
    background: transparent;
    color: var(--ink-2);
    cursor: pointer;
    transition: background var(--dur-fast), color var(--dur-fast);
  }
  .pane-head-btn:hover:not(:disabled) {
    background: var(--bg-1);
    color: var(--ink-0);
  }
  .pane-head-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
  .latency {
    font-size: 10.5px;
  }
  .cloud-flag {
    margin-left: 4px;
    font-size: 10px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .pane-textarea {
    flex: 1;
    width: 100%;
    box-sizing: border-box;
    padding: 16px;
    background: transparent;
    color: var(--ink-0);
    border: 0;
    resize: vertical;
    font-family: var(--font-sans);
    font-size: 15px;
    line-height: 1.7;
    min-height: 280px;
  }
  .pane-textarea:focus {
    outline: none;
  }
  .pane-textarea::placeholder {
    color: var(--ink-3);
    white-space: pre-line;
  }

  .pane-result {
    flex: 1;
    padding: 16px;
    min-height: 280px;
    color: var(--ink-0);
    font-size: 15px;
    line-height: 1.7;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .pane-result.placeholder {
    display: grid;
    place-items: center;
  }
  .result-placeholder {
    color: var(--ink-3);
    font-size: 13px;
  }
  .result-text {
    margin: 0;
  }
  .result-loading,
  .result-error {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .result-error {
    color: #d87a7a;
  }

  .action-bar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .action-left {
    flex: 1;
    min-width: 0;
  }
  .action-msg {
    font-size: 12px;
    color: var(--ink-2);
  }
  .action-msg.muted-msg {
    color: var(--ink-3);
  }
  .action-msg.error {
    color: #d87a7a;
  }
  .kbd-inline {
    margin-left: 8px;
    opacity: 0.6;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  /* Lucide icons don't ship animation utilities; reuse the global
     `ml-spin` keyframe from components.css. `.spin` is applied to the
     <svg> root rendered by lucide-svelte, so the selector has to be
     `:global` for Svelte's scoped CSS to keep it. */
  :global(.spin) {
    animation: ml-spin 0.9s linear infinite;
  }
  /* `.small` is the only typography utility we declare locally — the
     project convention is each page redeclares it (see capture, lens,
     audio) so font size context can be tweaked per surface. */
  .small {
    font-size: 11px;
  }
</style>
