<!--
  First-run modal that prompts the user to download the embedded
  LLM model. Surfaces immediately on first launch when:
    1. `providers.translate.chain` references `embedded.llm` (the
       default), AND
    2. The GGUF file is not yet on disk, AND
    3. The user has not already dismissed this prompt.

  The modal commits to a single decision: download now (with live
  progress), or skip. Both branches mark `embedded_model_setup_seen`
  so the modal does not re-pop on every restart — users who change
  their mind work through the Settings → Providers card. Without
  this gate the install→use flow would silently fail on the very
  first translation attempt; the whole point of shipping
  `embedded.llm` as the default is that it works out of the box.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Download, Sparkles, X } from 'lucide-svelte';
  import {
    downloadEmbeddedModel,
    getOnboardingStatus,
    markEmbeddedModelSetupSeen,
    modelTierStatus,
    onEmbeddedModelProgress,
    pickAndApplyEmbeddedModelPackage,
    setModelTier,
    type ModelProgress,
    type ModelTierStatus,
  } from '$lib/ipc';

  let show = $state(false);
  let modelPath = $state('');
  let downloading = $state(false);
  let importing = $state(false);
  let progress = $state<ModelProgress | null>(null);
  let error = $state<string | null>(null);
  let unlistenProgress: (() => void) | null = null;
  /** Recommendation snapshot from `modelTierStatus`. Used to:
   *  1. Show a "RAM 8GBなのでStandard推奨" rationale up front,
   *  2. Quietly switch the embedded provider to the recommended
   *     tier on first launch, BEFORE the user clicks ダウンロード,
   *     so the download fetches the right-sized model for the host. */
  let tierInfo = $state<ModelTierStatus | null>(null);

  onMount(async () => {
    try {
      const status = await getOnboardingStatus();
      show = status.needs_embedded_model_setup;
      modelPath = status.embedded_model_path;
    } catch (e) {
      // Onboarding is not critical to app function — fail silent
      // rather than blocking the layout shell on a startup error.
      console.warn('embedded onboarding status failed:', e);
    }
    if (show) {
      try {
        tierInfo = await modelTierStatus();
        // Auto-align the embedded provider with the recommended tier
        // on first launch — without this a 4 GB-RAM laptop would
        // download the 1.1 GB Standard tier even though Compact is
        // the right pick. Only fires when the current tier matches
        // the bundled default (so users who already hand-edited
        // their config keep their choice).
        if (
          tierInfo &&
          tierInfo.current &&
          tierInfo.current.id !== tierInfo.recommended.id &&
          tierInfo.current.id ===
            (await modelTierStatus())?.current?.id
        ) {
          // Re-fetched to double-check we are not racing a settings
          // save in flight.
          await setModelTier(tierInfo.recommended.id);
          tierInfo = await modelTierStatus();
        }
      } catch (e) {
        console.warn('tier info load failed:', e);
      }
    }
    try {
      unlistenProgress = await onEmbeddedModelProgress((p) => {
        progress = p;
      });
    } catch (e) {
      console.warn('embedded onboarding progress listener failed:', e);
    }
  });

  onDestroy(() => {
    if (unlistenProgress) {
      unlistenProgress();
      unlistenProgress = null;
    }
  });

  function formatBytes(bytes: number | null | undefined): string {
    if (bytes == null) return '不明';
    const mb = bytes / (1024 * 1024);
    if (mb < 1024) return `${mb.toFixed(1)} MB`;
    return `${(mb / 1024).toFixed(2)} GB`;
  }

  async function startDownload() {
    if (downloading || importing) return;
    downloading = true;
    error = null;
    progress = null;
    try {
      await downloadEmbeddedModel(false);
      // Persist "seen" only after success so a transient network
      // failure does not silence the prompt forever — a user who
      // gives up and clicks 後で still gets persisted, the failed
      // path keeps the prompt around for the next launch.
      await markEmbeddedModelSetupSeen();
      show = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      downloading = false;
    }
  }

  /** Restricted-network path: import a `.moonlens-model` package
   * that an admin produced on a PC with internet access and
   * distributed via fileserver. No HuggingFace round-trip needed. */
  async function startImport() {
    if (downloading || importing) return;
    importing = true;
    error = null;
    try {
      const imported = await pickAndApplyEmbeddedModelPackage();
      if (imported) {
        await markEmbeddedModelSetupSeen();
        show = false;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      importing = false;
    }
  }

  async function dismiss() {
    // Even on dismiss, mark seen — otherwise every launch would
    // re-pop. The Settings → Providers card carries the same
    // download CTA for users who change their mind.
    try {
      await markEmbeddedModelSetupSeen();
    } catch (e) {
      console.warn('mark embedded setup seen failed:', e);
    }
    show = false;
  }
</script>

{#if show}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
    role="dialog"
    aria-modal="true"
    aria-labelledby="embedded-onboarding-title"
  >
    <div
      class="w-full max-w-xl rounded-ml border border-border bg-surface-elevated p-6 shadow-ml-lg"
    >
      <header class="flex items-start justify-between gap-3">
        <div class="flex items-start gap-3">
          <div
            class="flex h-10 w-10 shrink-0 items-center justify-center rounded-ml bg-accent-soft text-accent"
          >
            <Sparkles class="h-5 w-5" aria-hidden="true" />
          </div>
          <div>
            <h2 id="embedded-onboarding-title" class="text-base font-semibold">
              ローカル翻訳モデルを準備します
            </h2>
            <p class="mt-1 text-sm text-ink-muted">
              MoonLens は外部サーバ不要で動作します。Qwen 2.5
              モデル (Apache 2.0) をダウンロードして、以降は
              完全オフラインで翻訳できます。
            </p>
            {#if tierInfo}
              <p class="mt-1 text-xs text-ink-muted">
                <strong>{tierInfo.recommended.display_name}</strong>
                {tierInfo.recommended.description}
                <br />
                <span class="text-ink-faint">
                  {tierInfo.recommended_reason} ・
                  ダウンロードサイズ:
                  {(tierInfo.recommended.size_mb / 1024).toFixed(1)} GB
                </span>
              </p>
            {/if}
            <p class="mt-1 text-xs text-ink-faint">
              インターネット制限環境では、別 PC で作成された
              <code>.moonlens-model</code> パッケージファイルから取込も可能です。
            </p>
            {#if modelPath}
              <p class="mt-2 text-xs text-ink-faint">
                保存先: <code class="text-ink-muted">{modelPath}</code>
              </p>
            {/if}
          </div>
        </div>
        {#if !downloading}
          <button
            class="inline-flex h-7 w-7 items-center justify-center rounded-ml-sm text-ink-muted hover:bg-surface hover:text-ink"
            onclick={dismiss}
            aria-label="閉じる"
          >
            <X class="h-4 w-4" />
          </button>
        {/if}
      </header>

      {#if downloading && progress}
        <div class="mt-4 rounded-ml border border-border bg-surface p-3">
          <div class="flex items-center justify-between text-xs text-ink-muted">
            <span>{progress.filename}</span>
            <span>
              {formatBytes(progress.downloaded)}
              {#if progress.total}
                / {formatBytes(progress.total)}
              {/if}
              {#if progress.fraction != null}
                ({(progress.fraction * 100).toFixed(1)}%)
              {/if}
            </span>
          </div>
          <progress
            class="mt-2 w-full"
            value={progress.fraction ?? 0}
            max="1"
          ></progress>
        </div>
      {:else if downloading}
        <div class="mt-4 text-sm text-ink-muted">
          ダウンロードを開始しています…
        </div>
      {/if}

      {#if error}
        <div class="mt-4 rounded-ml border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-500">
          ダウンロードに失敗しました: {error}
        </div>
      {/if}

      <footer class="mt-6 flex items-center justify-end gap-2">
        <button
          class="rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted hover:border-accent/60 hover:text-ink"
          onclick={dismiss}
          disabled={downloading || importing}
        >
          後で (設定画面から)
        </button>
        <button
          class="rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted hover:border-accent/60 hover:text-ink disabled:opacity-50"
          onclick={startImport}
          disabled={downloading || importing}
          title=".moonlens-model パッケージファイルから取り込む (社内配布用)"
        >
          {importing ? '取込中…' : 'パッケージから取込'}
        </button>
        <button
          class="inline-flex items-center gap-1.5 rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white hover:opacity-90 disabled:opacity-50"
          onclick={startDownload}
          disabled={downloading || importing}
        >
          <Download class="h-4 w-4" aria-hidden="true" />
          {downloading ? 'ダウンロード中…' : 'ダウンロードして開始'}
        </button>
      </footer>
    </div>
  </div>
{/if}
