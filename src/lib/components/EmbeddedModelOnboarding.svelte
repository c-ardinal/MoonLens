<!--
  First-run modal that prepares the embedded LLM. Pops on first
  launch when:
    1. `providers.translate.chain` references `embedded.llm` (the
       default), AND
    2. At least one GGUF required by the user's saved routing
       tier (`providers.user_tier`) is missing, AND
    3. The user has not already dismissed this prompt.

  Phase 27 refactor: the modal now drives the **routing-tier**
  surface rather than the legacy single-model picker. It:
    - Auto-aligns `user_tier` with the HW recommendation
      (Compact ≤ 6 GB RAM / Standard ≤ 12 GB / Quality otherwise)
      before downloading, so the right size set is fetched.
    - Fetches per-model state via `routedModelStatus` and
      downloads each missing GGUF sequentially via
      `downloadRoutedModel`. Standard / Quality tiers need TWO
      files — the previous single-file download silently left
      one missing on first run.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Download, Sparkles, X } from 'lucide-svelte';
  import {
    downloadRoutedModel,
    getOnboardingStatus,
    getUserTier,
    markEmbeddedModelSetupSeen,
    modelTierStatus,
    onEmbeddedModelProgress,
    pickAndApplyEmbeddedModelPackage,
    routedModelStatus,
    setUserTier,
    type ModelProgress,
    type ModelTierStatus,
    type RoutedModelStatus,
    type UserTier,
    type UserTierStatus,
  } from '$lib/ipc';

  let show = $state(false);
  let modelPath = $state('');
  let downloading = $state(false);
  let importing = $state(false);
  let progress = $state<ModelProgress | null>(null);
  let error = $state<string | null>(null);
  let unlistenProgress: (() => void) | null = null;
  /** Hardware-derived recommendation snapshot. Used for the
   *  "RAM 8GB なので Standard 推奨" rationale and to bootstrap
   *  the user_tier alignment below. */
  let tierInfo = $state<ModelTierStatus | null>(null);
  /** Currently-saved routing tier. Either matches the HW
   *  recommendation already (in which case we just download)
   *  or differs (we silently switch on first launch). */
  let userTier = $state<UserTierStatus | null>(null);
  /** Per-model presence for the active routing tier. The
   *  modal's download loop walks this list, skipping any
   *  rows that are already present (warm-cache restart). */
  let models = $state<RoutedModelStatus[]>([]);
  /** Which model row's bytes are currently flowing. Used in
   *  the progress display to show "(2/3) gemma-4-E2B-it..."
   *  context alongside the byte counter. */
  let currentDownloadIndex = $state<number | null>(null);

  /** Map `ModelTier.class` → `UserTier`. The two enums are
   *  parallel by design (both Compact/Standard/Quality) so the
   *  cast is total. */
  function classToUserTier(klass: ModelTierStatus['recommended']['class']): UserTier {
    if (klass === 'compact') return 'compact';
    if (klass === 'quality') return 'quality';
    return 'standard';
  }

  onMount(async () => {
    try {
      const status = await getOnboardingStatus();
      modelPath = status.embedded_model_path;
      // Even if `needs_embedded_model_setup` is true (legacy
      // single-file check), the routing surface might already
      // be complete via a previously-downloaded other tier. We
      // fetch the routed model state first, then short-circuit
      // if everything is in fact present.
    } catch (e) {
      console.warn('embedded onboarding status failed:', e);
    }
    try {
      tierInfo = await modelTierStatus();
      userTier = await getUserTier();
      // Auto-align user_tier with the HW recommendation on the
      // very first launch (only when the user has not yet
      // diverged from the default). Without this, a 4 GB-RAM
      // laptop with the bundled "standard" default would
      // download the 4.2 GB Standard set even though Compact is
      // the right pick.
      if (tierInfo && userTier && userTier.current === 'standard') {
        const recommended = classToUserTier(tierInfo.recommended.class);
        if (recommended !== userTier.current) {
          await setUserTier(recommended);
          userTier = await getUserTier();
        }
      }
      models = await routedModelStatus();
    } catch (e) {
      console.warn('routing tier load failed:', e);
    }
    // Show the modal when at least one required model is
    // missing. If everything is present (e.g. import flow from
    // a previous launch already populated all GGUFs) the modal
    // is suppressed; the Settings → Providers card remains
    // available for users who want to override.
    show = models.some((m) => !m.present);
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

  function formatSizeMb(mb: number): string {
    if (mb < 1024) return `${mb} MB`;
    return `${(mb / 1024).toFixed(1)} GB`;
  }

  function totalRequiredMb(): number {
    return models.filter((m) => !m.present).reduce((a, m) => a + m.size_mb, 0);
  }

  async function startDownload() {
    if (downloading || importing) return;
    downloading = true;
    error = null;
    progress = null;
    try {
      // Download each missing tier sequentially. Sequential
      // (not parallel) because the progress event channel
      // is single-stream and parallel downloads would
      // interleave bytes confusingly in the UI. The disk
      // bandwidth saved by parallelism is minimal for 1-3
      // GGUFs of this size.
      for (let i = 0; i < models.length; i++) {
        const m = models[i];
        if (m.present) continue;
        currentDownloadIndex = i;
        await downloadRoutedModel(m.tier_id, false);
      }
      currentDownloadIndex = null;
      models = await routedModelStatus();
      await markEmbeddedModelSetupSeen();
      show = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      downloading = false;
    }
  }

  /** Restricted-network path: import a `.moonlens-model` package
   *  that an admin produced on a PC with internet access and
   *  distributed via fileserver. The legacy single-file package
   *  only covers ONE model, so on Standard/Quality tiers the
   *  import + download flow can coexist (import the JA bridge
   *  via package, fetch the multilingual model online). */
  async function startImport() {
    if (downloading || importing) return;
    importing = true;
    error = null;
    try {
      const imported = await pickAndApplyEmbeddedModelPackage();
      if (imported) {
        models = await routedModelStatus();
        // Only dismiss the modal when EVERY required model is now
        // present. Otherwise leave the modal open so the user
        // sees the remaining download CTAs.
        if (models.every((m) => m.present)) {
          await markEmbeddedModelSetupSeen();
          show = false;
        }
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      importing = false;
    }
  }

  async function dismiss() {
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
              翻訳モデルを準備します
            </h2>
            <p class="mt-1 text-sm text-ink-muted">
              MoonLens は外部サーバ不要で動作します。お使いの PC に最適な
              翻訳モデル一式をダウンロードして、以降は完全オフラインで
              翻訳できます。
            </p>
            {#if tierInfo && userTier}
              <p class="mt-2 text-xs text-ink-muted">
                推奨ティア:
                <strong>
                  {userTier.current === 'compact'
                    ? 'Compact (EN↔JA 専用)'
                    : userTier.current === 'quality'
                      ? 'Quality (高品質・多言語)'
                      : 'Standard (バランス)'}
                </strong>
                <span class="text-ink-faint">— {tierInfo.recommended_reason}</span>
              </p>
            {/if}
            {#if models.length > 0}
              <ul class="mt-2 space-y-1 text-xs text-ink-muted">
                {#each models as m, idx (m.tier_id)}
                  <li class="flex items-center gap-2">
                    <span aria-hidden="true">
                      {#if m.present}
                        ✅
                      {:else if downloading && currentDownloadIndex === idx}
                        ⏳
                      {:else}
                        ⬇️
                      {/if}
                    </span>
                    <span class="flex-1">{m.display_name}</span>
                    <span class="text-ink-faint">{formatSizeMb(m.size_mb)}</span>
                  </li>
                {/each}
              </ul>
              <p class="mt-1 text-xs text-ink-faint">
                合計ダウンロードサイズ: 約 {formatSizeMb(totalRequiredMb())}
              </p>
            {/if}
            <p class="mt-2 text-xs text-ink-faint">
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
            <span>
              {#if currentDownloadIndex != null && models[currentDownloadIndex]}
                ({currentDownloadIndex + 1}/{models.length})
                {models[currentDownloadIndex].display_name}
              {:else}
                {progress.filename}
              {/if}
            </span>
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
