<!--
  Text translation page: two panes (source / result) with a language
  pair, swap, copy, Ctrl+Enter and opt-in auto-translate.

  Translation goes through the same `run_translate` command as the rest
  of the app (provider chain, privacy gates, fallbacks). Multi-line input
  is translated line by line so logs and code keep their structure;
  every line is sent with the lines already translated as history, so a
  context-aware engine reads it in context instead of blind.
-->
<script lang="ts">
  import { holdMotion } from '$lib/pixel/motion.svelte';
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import { onDestroy, onMount } from 'svelte';
  import { ArrowLeftRight, Check, Copy, Cpu, Cloud, Languages, Loader2, X } from 'lucide-svelte';
  import { getSettings, recordTranslation, runTranslate, type TaskResult } from '$lib/ipc';
  import { extractText } from '$lib/result';
  import { parseError, type UiError } from '$lib/errors';
  import { LANGUAGES } from '$lib/lens-overlay';
  import { translateLines, type LineHistory } from '$lib/translate-line';

  const SOURCE_LANGS = [{ code: 'auto', label: '自動判定' }, ...LANGUAGES];
  const TARGET_LANGS = LANGUAGES;

  const MAX_CHARS = 5000;
  const PAIR_KEY = 'moonlens.translate.pair';
  const AUTO_KEY = 'moonlens.translate.auto';
  const DEBOUNCE_MS = 600;

  let sourceLang = $state('auto');
  let targetLang = $state('ja');
  let inputText = $state('');
  let resultText = $state('');
  let translating = $state(false);
  // 推論中はメインウィンドウの演出を停止
  $effect(() => (translating ? holdMotion() : undefined));
  let error = $state<UiError | null>(null);
  let provider = $state<string | null>(null);
  let viaCloud = $state(false);
  let latencyMs = $state<number | null>(null);
  let autoTranslate = $state(false);
  let progress = $state<string | null>(null);
  let copied = $state(false);

  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  /** Bumped on every run and on clear, so a run whose input or language
   *  changed under it does not overwrite the newer state. */
  let runId = 0;
  /** A request that arrived while a run was in flight; replayed after. */
  let rerun = false;
  /** Last saved (source, result), to skip duplicate history rows. */
  let lastSavedKey = '';

  const charCount = $derived(inputText.length);
  const overLimit = $derived(charCount > MAX_CHARS);
  const canTranslate = $derived(!translating && inputText.trim().length > 0 && !overLimit);

  function readPrefs(): void {
    try {
      const raw = localStorage.getItem(PAIR_KEY);
      if (raw) {
        const p = JSON.parse(raw) as { source?: unknown; target?: unknown };
        if (typeof p.source === 'string' && SOURCE_LANGS.some((l) => l.code === p.source)) {
          sourceLang = p.source;
        }
        if (typeof p.target === 'string' && TARGET_LANGS.some((l) => l.code === p.target)) {
          targetLang = p.target;
        }
      }
      autoTranslate = localStorage.getItem(AUTO_KEY) === '1';
    } catch {
      /* storage unavailable: keep the settings defaults */
    }
  }

  function savePrefs(): void {
    try {
      localStorage.setItem(PAIR_KEY, JSON.stringify({ source: sourceLang, target: targetLang }));
      localStorage.setItem(AUTO_KEY, autoTranslate ? '1' : '0');
    } catch {
      /* ignore */
    }
  }

  onMount(async () => {
    // Settings give the defaults; the last pair used here wins.
    try {
      const s = await getSettings();
      const target = s?.subtitle?.target_language;
      if (typeof target === 'string' && TARGET_LANGS.some((l) => l.code === target)) {
        targetLang = target;
      }
    } catch {
      /* backend unreachable: keep state defaults */
    }
    readPrefs();
  });

  onDestroy(() => {
    runId += 1;
    if (copiedTimer) clearTimeout(copiedTimer);
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  function resetResult(): void {
    resultText = '';
    provider = null;
    viaCloud = false;
    latencyMs = null;
  }

  function scheduleAuto(): void {
    if (!autoTranslate) return;
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => void translate(), DEBOUNCE_MS);
  }

  function request(text: string, history: LineHistory): Promise<TaskResult> {
    return runTranslate({
      text,
      source_language: sourceLang,
      target_language: targetLang,
      history: history.map(([s, t]) => [s, t] as [string, string])
    });
  }

  async function translate(): Promise<void> {
    if (translating) {
      rerun = true;
      return;
    }
    const text = inputText.trim();
    if (!text) {
      resetResult();
      error = null;
      return;
    }
    if (overLimit) return;

    const id = ++runId;
    translating = true;
    progress = null;
    error = null;
    const started = performance.now();
    try {
      let output: string;
      let usedProvider: string | null;
      let cloud: boolean;
      if (inputText.includes('\n')) {
        // Raw input, not trimmed: blank lines the user left on purpose
        // stay in the output.
        const res = await translateLines(
          inputText,
          async (line, history) => {
            const r = await request(line, history);
            return {
              text: extractText(r).replace(/\s*\n+\s*/g, ' ').trim(),
              provider: r.used_provider ?? null,
              viaCloud: Boolean(r.via_cloud)
            };
          },
          (done, total) => {
            if (id === runId) progress = `${done} / ${total} 行`;
          }
        );
        output = res.output;
        usedProvider = res.firstProvider;
        cloud = res.firstViaCloud;
      } else {
        const r = await request(text, []);
        output = extractText(r).trim();
        usedProvider = r.used_provider ?? null;
        cloud = Boolean(r.via_cloud);
      }
      if (id !== runId) return;
      resultText = output;
      provider = usedProvider;
      viaCloud = cloud;
      latencyMs = Math.round(performance.now() - started);
      await saveHistory(inputText, output, usedProvider, cloud);
    } catch (e) {
      if (id !== runId) return;
      error = parseError(e);
      resetResult();
    } finally {
      translating = false;
      progress = null;
      if (rerun) {
        rerun = false;
        void translate();
      }
    }
  }

  async function saveHistory(
    src: string,
    dst: string,
    usedProvider: string | null,
    cloud: boolean
  ): Promise<void> {
    if (!src.trim() || !dst.trim()) return;
    const key = `${src.trim()}␟${dst.trim()}`;
    if (key === lastSavedKey) return;
    try {
      await recordTranslation({
        source_text: src,
        result_text: dst,
        provider: usedProvider,
        via_cloud: cloud
      });
      lastSavedKey = key;
    } catch {
      /* best effort: the translation itself succeeded */
    }
  }

  function swap(): void {
    if (sourceLang === 'auto') {
      sourceLang = targetLang;
      targetLang = targetLang === 'ja' ? 'en' : 'ja';
    } else {
      [sourceLang, targetLang] = [targetLang, sourceLang];
    }
    if (resultText) {
      [inputText, resultText] = [resultText, inputText];
      provider = null;
      latencyMs = null;
    }
    savePrefs();
    scheduleAuto();
  }

  function clearAll(): void {
    runId += 1;
    inputText = '';
    error = null;
    resetResult();
    if (debounceTimer) clearTimeout(debounceTimer);
  }

  async function copyResult(): Promise<void> {
    if (!resultText) return;
    try {
      await navigator.clipboard.writeText(resultText);
      copied = true;
      if (copiedTimer) clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copied = false), 1500);
    } catch (e) {
      error = { code: 'error', message: `コピー失敗：${String(e)}`, fix: null };
    }
  }

  function onKeyDown(e: KeyboardEvent): void {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void translate();
    }
  }

  function onLangChange(): void {
    savePrefs();
    if (inputText.trim()) scheduleAuto();
  }

  function onAutoChange(v: boolean): void {
    autoTranslate = v;
    savePrefs();
    if (v && inputText.trim()) scheduleAuto();
  }
</script>

<main class="mx-auto flex max-w-5xl flex-col gap-4 px-4 py-6 sm:px-6">
  <header class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
    <div class="min-w-0">
      <div class="page-title"><PixelTitle text="テキスト翻訳" tag="h1" height={40} fit /></div>
      <p class="text-sm text-ink-muted">複数行は行単位で翻訳（前行を文脈に参照）</p>
    </div>
    <label class="inline-flex shrink-0 cursor-pointer items-center gap-2 text-sm" title="入力停止 0.6 秒後に翻訳">
      <span class="text-ink-muted">自動翻訳</span>
      <span class="relative inline-flex items-center">
        <input
          type="checkbox"
          class="peer sr-only"
          checked={autoTranslate}
          onchange={(e) => onAutoChange((e.currentTarget as HTMLInputElement).checked)}
        />
        <span class="h-5 w-9 rounded-full bg-surface-muted ring-1 ring-border transition peer-checked:bg-accent"></span>
        <span
          class="absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white shadow transition peer-checked:translate-x-4"
        ></span>
      </span>
    </label>
  </header>

  <div class="flex flex-wrap items-end gap-2">
    <label class="flex min-w-0 flex-1 flex-col gap-1 text-xs text-ink-muted">
      翻訳元
      <select
        class="rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm text-ink"
        bind:value={sourceLang}
        onchange={onLangChange}
      >
        {#each SOURCE_LANGS as l (l.code)}
          <option value={l.code}>{l.label}</option>
        {/each}
      </select>
    </label>
    <button
      type="button"
      class="inline-flex h-12 w-12 shrink-0 items-center justify-center rounded-ml-sm border border-border bg-surface text-ink-muted transition hover:border-accent/60 hover:text-ink"
      title="言語入替"
      aria-label="言語入替"
      onclick={swap}
    >
      <ArrowLeftRight class="h-4 w-4" />
    </button>
    <label class="flex min-w-0 flex-1 flex-col gap-1 text-xs text-ink-muted">
      翻訳先
      <select
        class="rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm text-ink"
        bind:value={targetLang}
        onchange={onLangChange}
      >
        {#each TARGET_LANGS as l (l.code)}
          <option value={l.code}>{l.label}</option>
        {/each}
      </select>
    </label>
  </div>

  <div class="grid gap-3 sm:grid-cols-2">
    <section class="flex min-w-0 flex-col rounded-ml border border-border bg-surface-elevated shadow-ml">
      <textarea
        class="min-h-48 w-full flex-1 resize-none rounded-t-ml bg-transparent p-3 text-sm leading-relaxed outline-none"
        placeholder="翻訳対象の文章（Ctrl+Enter で翻訳）"
        aria-label="翻訳元テキスト"
        spellcheck="false"
        bind:value={inputText}
        oninput={() => {
          error = null;
          scheduleAuto();
        }}
        onkeydown={onKeyDown}
      ></textarea>
      <div class="flex items-center justify-between gap-2 h-14 border-t border-border px-3">
        <span class="text-xs tabular-nums {overLimit ? 'text-red-500' : 'text-ink-faint'}">
          {charCount.toLocaleString()} / {MAX_CHARS.toLocaleString()}{overLimit ? '（文字数超過）' : ''}
        </span>
        <div class="flex items-center gap-2">
          {#if inputText}
            <button
              type="button"
              class="rounded-ml-sm p-1.5 text-ink-faint transition hover:bg-surface-muted hover:text-ink"
              title="消去"
              aria-label="消去"
              onclick={clearAll}
            >
              <X class="h-4 w-4" />
            </button>
          {/if}
          <button
            type="button"
            class="inline-flex items-center gap-1.5 rounded-ml-sm border border-accent bg-accent px-3 py-1.5 text-sm font-medium text-white transition disabled:opacity-50"
            disabled={!canTranslate}
            onclick={() => void translate()}
          >
            {#if translating}
              <Loader2 class="h-4 w-4 animate-spin" />
              {progress ?? '翻訳中'}
            {:else}
              <Languages class="h-4 w-4" />
              翻訳
            {/if}
          </button>
        </div>
      </div>
    </section>

    <section
      class="flex min-w-0 flex-col rounded-ml border border-border bg-surface-elevated shadow-ml"
      aria-live="polite"
    >
      <div class="min-h-48 flex-1 p-3 text-sm leading-relaxed">
        {#if error}
          <div class="rounded-ml-sm border border-red-500/50 bg-red-500/10 p-3 text-red-500" role="alert">
            <p class="break-words">{error.message}</p>
            {#if error.fix}
              <a href={error.fix.route} class="mt-1 inline-block text-accent underline-offset-2 hover:underline">
                {error.fix.label}
              </a>
            {/if}
          </div>
        {:else if resultText}
          <p class="whitespace-pre-wrap break-words" data-testid="translate-result">{resultText}</p>
        {:else}
          <p class="text-ink-faint">{translating ? '翻訳中…' : '翻訳結果'}</p>
        {/if}
      </div>
      <div class="flex items-center justify-between gap-2 h-14 border-t border-border px-3">
        <span class="flex min-w-0 items-center gap-1 truncate font-mono text-xs text-ink-faint">
          {#if provider}
            {#if viaCloud}
              <Cloud class="h-3 w-3 shrink-0 text-accent" />
            {:else}
              <Cpu class="h-3 w-3 shrink-0" />
            {/if}
            <span class="truncate">{provider}</span>
            {#if latencyMs != null}<span class="shrink-0">· {latencyMs.toLocaleString()} ms</span>{/if}
          {/if}
        </span>
        <button
          type="button"
          class="inline-flex shrink-0 items-center gap-1 rounded-ml-sm px-2 py-1 text-xs text-ink-muted transition hover:bg-surface-muted hover:text-ink disabled:opacity-40"
          disabled={!resultText}
          onclick={copyResult}
        >
          {#if copied}
            <Check class="h-3.5 w-3.5 text-accent" />コピー完了
          {:else}
            <Copy class="h-3.5 w-3.5" />コピー
          {/if}
        </button>
      </div>
    </section>
  </div>
</main>
