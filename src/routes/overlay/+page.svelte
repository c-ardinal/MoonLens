<script lang="ts">
  import { t } from '$lib/i18n';
  import { onDestroy, onMount } from 'svelte';
  import { Radio, Sparkles, X } from 'lucide-svelte';
  import PrivacyBadge from '$lib/components/PrivacyBadge.svelte';
  import ActionRibbon from '$lib/components/ActionRibbon.svelte';
  import {
    getSettings,
    hideOverlay,
    onOverlay,
    onSettingsChanged,
    openLens,
    runExplain,
    openExternalUrl,
    runTranslate,
    type CaptureFlowRect,
    type Intent,
    type OverlayEvent,
    type TaskResult
  } from '$lib/ipc';
  import { extractText } from '$lib/result';
  import { errorText } from '$lib/errors';

  let source = $state<string | null>(null);
  let translation = $state('');
  let viaCloud = $state(false);
  let provider = $state('');
  let status = $state<'idle' | 'processing' | 'result' | 'error'>('idle');
  let statusMessage = $state('');
  let intent = $state<Intent | null>(null);
  let sourceRect = $state<CaptureFlowRect | null>(null);
  let unlisten: (() => void) | undefined;
  let unlistenSettings: (() => void) | undefined;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

  // Settings → オーバーレイ.
  let look = $state({
    maxWidth: 480,
    opacity: 0.92,
    blur: 24,
    saturation: 1.8,
    fontSize: 14,
    lineHeight: 1.5,
    autoHideMs: 0
  });

  function applySettings(s: any) {
    const o = s?.overlay ?? {};
    look = {
      maxWidth: o.max_width_px ?? 480,
      opacity: o.opacity ?? 0.92,
      blur: o.blur_px ?? 24,
      saturation: o.saturation ?? 1.8,
      fontSize: o.font_size_px ?? 14,
      lineHeight: o.line_height ?? 1.5,
      autoHideMs: o.auto_hide_ms ?? 0
    };
  }

  /** Hide the finished result after `auto_hide_ms`. */
  function scheduleAutoHide() {
    if (hideTimer) clearTimeout(hideTimer);
    if (look.autoHideMs > 0) {
      hideTimer = setTimeout(() => {
        if (status === 'result') hideOverlay().catch(() => {});
      }, look.autoHideMs);
    }
  }

  onMount(async () => {
    getSettings().then(applySettings).catch(() => {});
    unlistenSettings = await onSettingsChanged(applySettings);
    unlisten = await onOverlay((event: OverlayEvent) => {
      if (event.kind === 'processing') {
        status = 'processing';
        statusMessage = event.message;
      } else if (event.kind === 'result') {
        status = 'result';
        source = event.source_text;
        translation = extractText(event.result);
        viaCloud = event.result.via_cloud;
        provider = event.result.used_provider;
        intent = event.intent;
        sourceRect = event.source_rect;
        scheduleAutoHide();
      } else if (event.kind === 'chunk') {
        // Ollama streaming: append deltas as they arrive so the user
        // sees tokens live. The final `Result` event will replace the
        // accumulated string with the authoritative output.
        if (status !== 'result') {
          status = 'processing';
          translation = '';
        }
        translation += event.delta;
      } else if (event.kind === 'error') {
        status = 'error';
        statusMessage = event.message;
      } else {
        status = 'idle';
      }
    });
  });

  onDestroy(() => {
    unlisten?.();
    unlistenSettings?.();
    if (hideTimer) clearTimeout(hideTimer);
  });

  function copy() {
    if (translation) navigator.clipboard.writeText(translation).catch(() => {});
  }

  /** Continuous window translation of the same range; the popup steps aside. */
  async function keepTranslating() {
    if (!sourceRect) return;
    await openLens(sourceRect);
    await hideOverlay().catch(() => {});
  }

  /** Best-effort URL extractor: if the translated / decoded payload
   * looks like a URL, return it for the open-in-browser action. */
  function detectUrl(): string | null {
    const text = translation.trim();
    if (!text) return null;
    try {
      const u = new URL(text);
      if (u.protocol === 'http:' || u.protocol === 'https:') return text;
    } catch {
      /* fall through */
    }
    const m = text.match(/https?:\/\/\S+/);
    return m ? m[0] : null;
  }

  async function onRibbonAction(
    a: 'translate' | 'copy' | 'explain' | 'pin' | 'open_url'
  ) {
    if (a === 'copy') {
      copy();
      return;
    }
    if (a === 'pin') {
      await keepTranslating();
      return;
    }
    if (a === 'open_url') {
      const url = detectUrl();
      if (url) {
        // Route through the backend opener so it opens in the user's
        // default browser on every platform.
        try {
          await openExternalUrl(url);
        } catch (e) {
          status = 'error';
          statusMessage = errorText(e);
        }
      }
      return;
    }
    if (a === 'translate') {
      // Re-run translation on the currently displayed source_text.
      // Uses the target language from settings (it may have changed).
      if (source) {
        status = 'processing';
        statusMessage = t('common:translating');
        try {
          const settings = await getSettings().catch(() => null);
          const r = await runTranslate({
            text: source,
            source_language: 'auto',
            target_language: settings?.subtitle?.target_language ?? 'ja'
          });
          translation = extractText(r);
          viaCloud = r.via_cloud;
          provider = r.used_provider;
          status = 'result';
        } catch (e) {
          status = 'error';
          statusMessage = errorText(e);
        }
      }
      return;
    }
    if (a === 'explain') {
      const prompt = source
        // copy-lint-ignore i18n-ignore（LLM プロンプト）
        ? `次のテキストを簡潔に説明してください:\n\n${source}`
        // copy-lint-ignore i18n-ignore（LLM プロンプト）
        : `次の翻訳結果について背景を簡潔に説明してください:\n\n${translation}`;
      status = 'processing';
      statusMessage = t('overlay:explaining');
      try {
        const r = await runExplain(prompt);
        translation = extractText(r);
        viaCloud = r.via_cloud;
        provider = r.used_provider;
        status = 'result';
      } catch (e) {
        status = 'error';
        statusMessage = errorText(e);
      }
      return;
    }
  }

  async function dismiss() {
    await hideOverlay();
  }
</script>

<section
  aria-label={t('common:translated_text')}
  class="pointer-events-auto m-2 rounded-ml border border-border p-4 shadow-ml-lg"
  style="max-width: min({look.maxWidth}px, calc(100vw - 16px));
         background: color-mix(in oklab, var(--ml-surface-elevated) {Math.round(look.opacity * 100)}%, transparent);
         backdrop-filter: blur({look.blur}px) saturate({look.saturation});
         -webkit-backdrop-filter: blur({look.blur}px) saturate({look.saturation});"
  onpointerenter={() => hideTimer && clearTimeout(hideTimer)}
  onpointerleave={() => status === 'result' && scheduleAutoHide()}
>
  <header class="mb-2 flex items-center justify-between gap-2">
    <PrivacyBadge {viaCloud} {provider} />
    <div class="flex items-center gap-1 text-ink-faint">
      {#if status === 'processing'}
        <Sparkles class="h-4 w-4 animate-pulse" />
      {/if}
      <button
        class="rounded-ml-sm p-1 transition hover:bg-surface-muted disabled:opacity-40"
        disabled={!sourceRect}
        onclick={keepTranslating}
        title={t('overlay:continuous_retranslates_the_same_area_as_the')}
      >
        <Radio class="h-4 w-4 text-ink-faint" />
      </button>
      <button
        class="rounded-ml-sm p-1 transition hover:bg-surface-muted"
        onclick={dismiss}
        title={t('common:close')}
      >
        <X class="h-4 w-4 text-ink-faint" />
      </button>
    </div>
  </header>

  {#if status === 'processing'}
    <p class="text-sm text-ink-muted">{statusMessage || t('overlay:working')}</p>
  {:else if status === 'error'}
    <p class="text-sm text-red-500">{statusMessage}</p>
  {:else if status === 'result'}
    {#if source}
      <p class="font-mono text-xs leading-relaxed break-words text-ink-faint">{source}</p>
      <hr class="my-2 border-border" />
    {/if}
    <p
      class="whitespace-pre-wrap break-words"
      style="font-size: {look.fontSize}px; line-height: {look.lineHeight}"
    >{translation}</p>
    <ActionRibbon {intent} onAction={onRibbonAction} />
  {:else}
    <p class="text-sm text-ink-faint">{t('overlay:shows_window_translation_results')}</p>
  {/if}
</section>
