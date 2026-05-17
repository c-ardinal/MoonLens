<!--
  First-run onboarding for the embedded translation models. Replaces
  the previous compact modal with the Claude Design 2026.05 split-
  pane layout: a branded left rail with step indicator and a wider
  main panel that lays out the per-engine model cards.

  Behavior is unchanged: shows when the routing tier still has at
  least one GGUF missing, auto-aligns `user_tier` with the hardware
  recommendation on first launch, sequentially downloads each
  required model, and supports the offline `.moonlens-model` import
  path.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Download, Check, X, Cpu, Sparkles, Package } from 'lucide-svelte';
  import MoonLensMark from '$lib/components/MoonLensMark.svelte';
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
  let tierInfo = $state<ModelTierStatus | null>(null);
  let userTier = $state<UserTierStatus | null>(null);
  let models = $state<RoutedModelStatus[]>([]);
  let currentDownloadIndex = $state<number | null>(null);

  /** Step counter shown in the left rail. 0=welcome, 1=tier,
   *  2=models (active during this flow), 3=done. We start at 2
   *  because welcome/tier are auto-handled by the alignment logic
   *  below, then jump to 3 once all required models land. */
  const step = $derived(
    downloading
      ? 2
      : models.length > 0 && models.every((m) => m.present)
        ? 3
        : 2
  );

  function classToUserTier(klass: ModelTierStatus['recommended']['class']): UserTier {
    if (klass === 'compact') return 'compact';
    if (klass === 'quality') return 'quality';
    return 'standard';
  }

  onMount(async () => {
    try {
      const status = await getOnboardingStatus();
      modelPath = status.embedded_model_path;
    } catch (e) {
      console.warn('embedded onboarding status failed:', e);
    }
    try {
      tierInfo = await modelTierStatus();
      userTier = await getUserTier();
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

  function tierLabel(t: UserTier | null | undefined): string {
    if (t === 'compact') return 'Compact';
    if (t === 'quality') return 'Quality';
    return 'Standard';
  }

  function tierDescription(t: UserTier | null | undefined): string {
    if (t === 'compact') return 'EN↔JA 専用 · 軽量';
    if (t === 'quality') return '多言語 · 最高品質';
    return 'バランス · 多言語対応';
  }

  async function startDownload() {
    if (downloading || importing) return;
    downloading = true;
    error = null;
    progress = null;
    try {
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

  async function startImport() {
    if (downloading || importing) return;
    importing = true;
    error = null;
    try {
      const imported = await pickAndApplyEmbeddedModelPackage();
      if (imported) {
        models = await routedModelStatus();
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

  const steps: Array<{ id: number; label: string }> = [
    { id: 0, label: 'ようこそ' },
    { id: 1, label: 'ティアの選択' },
    { id: 2, label: 'モデルのダウンロード' },
    { id: 3, label: '準備完了' },
  ];
</script>

{#if show}
  <div
    class="onb-backdrop"
    role="dialog"
    aria-modal="true"
    aria-labelledby="onb-title"
  >
    <div class="onb-shell" data-theme="dark">
      <!-- Left rail — branded panel with step indicator -->
      <aside class="onb-rail">
        <div class="onb-moon-glow" aria-hidden="true"></div>
        <div class="onb-ring onb-ring-1" aria-hidden="true"></div>
        <div class="onb-ring onb-ring-2" aria-hidden="true"></div>

        <div class="onb-brand">
          <MoonLensMark size={28} />
          <span class="onb-brand-name">MoonLens</span>
        </div>

        <div class="onb-rail-body">
          <div class="onb-kicker">WELCOME</div>
          <h2 class="onb-hero">
            目に映るすべてを、<br />日本語で読む。
          </h2>
          <ol class="onb-steps">
            {#each steps as s (s.id)}
              <li
                class="onb-step"
                class:done={s.id < step}
                class:active={s.id === step}
                class:future={s.id > step}
              >
                <span class="onb-step-mark">
                  {#if s.id < step}
                    <Check class="h-2.5 w-2.5" aria-hidden="true" />
                  {:else}
                    {s.id + 1}
                  {/if}
                </span>
                <span class="onb-step-label">{s.label}</span>
              </li>
            {/each}
          </ol>
        </div>
      </aside>

      <!-- Main panel -->
      <main class="onb-main">
        <button
          class="onb-close"
          type="button"
          onclick={dismiss}
          disabled={downloading}
          aria-label="閉じる"
        >
          <X class="h-3.5 w-3.5" aria-hidden="true" />
        </button>

        <div class="onb-main-body scroll">
          <div class="onb-kicker">STEP {step + 1} / {steps.length}</div>
          <h1 id="onb-title" class="onb-title">
            {#if step === 3}
              準備が完了しました
            {:else}
              翻訳モデルをダウンロード
            {/if}
          </h1>
          <p class="onb-lead">
            {#if step === 3}
              すべてのモデルがダウンロード済みです。MoonLens
              は外部サーバ不要で動作し、翻訳はオフラインで完結します。
            {:else}
              すべてのモデルはあなたのデバイス上に保存され、翻訳は完全オフラインで行われます。あとから設定で変更できます。
            {/if}
          </p>

          {#if tierInfo && userTier}
            <div class="onb-tier-card">
              <div class="onb-tier-icon">
                <Sparkles class="h-3.5 w-3.5" aria-hidden="true" />
              </div>
              <div class="onb-tier-info">
                <div class="onb-tier-name">
                  推奨ティア: <strong>{tierLabel(userTier.current)}</strong>
                  <span class="onb-tier-pill">{tierDescription(userTier.current)}</span>
                </div>
                <div class="onb-tier-reason">
                  <Cpu class="h-3 w-3" aria-hidden="true" />
                  {tierInfo.recommended_reason}
                </div>
              </div>
            </div>
          {/if}

          {#if models.length > 0}
            <div class="onb-models">
              {#each models as m, idx (m.tier_id)}
                <div
                  class="onb-model-card"
                  class:present={m.present}
                  class:downloading={downloading && currentDownloadIndex === idx}
                >
                  <div class="onb-model-head">
                    <span class="onb-model-name">{m.display_name}</span>
                    <span class="onb-model-size mono">{formatSizeMb(m.size_mb)}</span>
                  </div>
                  <div class="onb-model-state">
                    {#if m.present}
                      <span class="onb-state-icon onb-state-present">
                        <Check class="h-3 w-3" aria-hidden="true" />
                      </span>
                      <span class="onb-state-text">ダウンロード済み</span>
                    {:else if downloading && currentDownloadIndex === idx}
                      <span class="onb-state-icon onb-state-loading">
                        <Download class="h-3 w-3" aria-hidden="true" />
                      </span>
                      <span class="onb-state-text">ダウンロード中…</span>
                      {#if progress}
                        <span class="onb-state-progress mono">
                          {formatBytes(progress.downloaded)}
                          {#if progress.total}
                            / {formatBytes(progress.total)}
                          {/if}
                          {#if progress.fraction != null}
                            ({(progress.fraction * 100).toFixed(1)}%)
                          {/if}
                        </span>
                      {/if}
                    {:else}
                      <span class="onb-state-icon onb-state-pending">
                        <Download class="h-3 w-3" aria-hidden="true" />
                      </span>
                      <span class="onb-state-text">未ダウンロード</span>
                    {/if}
                  </div>
                  {#if downloading && currentDownloadIndex === idx && progress?.fraction != null}
                    <div class="onb-progress-track">
                      <div
                        class="onb-progress-fill"
                        style:width="{(progress.fraction * 100).toFixed(1)}%"
                      ></div>
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}

          {#if error}
            <div class="onb-error" role="alert">
              ダウンロードに失敗しました: {error}
            </div>
          {/if}

          <div class="onb-summary">
            <Download class="h-4 w-4" aria-hidden="true" />
            <div class="onb-summary-body">
              <div class="onb-summary-title">
                合計ダウンロード <span class="mono">{formatSizeMb(totalRequiredMb())}</span>
              </div>
              <div class="onb-summary-sub">
                所要時間の目安: 6〜12分 (100 Mbps接続時)
              </div>
            </div>
          </div>

          {#if modelPath}
            <p class="onb-path">保存先: <code>{modelPath}</code></p>
          {/if}
        </div>

        <footer class="onb-footer">
          <button
            class="btn ghost"
            type="button"
            onclick={dismiss}
            disabled={downloading || importing}
          >
            後で (設定画面から)
          </button>
          <button
            class="btn"
            type="button"
            onclick={startImport}
            disabled={downloading || importing}
            title=".moonlens-model パッケージファイルから取り込む"
          >
            <Package class="h-3.5 w-3.5" aria-hidden="true" />
            {importing ? '取込中…' : 'パッケージから取込'}
          </button>
          <button
            class="btn primary lg"
            type="button"
            onclick={startDownload}
            disabled={downloading || importing}
          >
            <Download class="h-3.5 w-3.5" aria-hidden="true" />
            {downloading ? 'ダウンロード中…' : 'ダウンロード開始'}
          </button>
        </footer>
      </main>
    </div>
  </div>
{/if}

<style>
  /* ============================================================
     Backdrop + shell
     ============================================================ */
  .onb-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    animation: onb-fade-in 220ms ease;
  }
  @keyframes onb-fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }
  .onb-shell {
    width: 100%;
    max-width: 880px;
    height: min(640px, calc(100vh - 48px));
    display: grid;
    grid-template-columns: 280px minmax(0, 1fr);
    background: var(--bg-0);
    color: var(--ink-0);
    border-radius: 14px;
    overflow: hidden;
    border: 1px solid var(--line);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55), 0 0 0 1px rgba(255, 255, 255, 0.04);
    animation: onb-pop 320ms cubic-bezier(0.22, 0.61, 0.36, 1);
  }
  @keyframes onb-pop {
    from { transform: translateY(8px) scale(0.98); opacity: 0; }
    to   { transform: translateY(0) scale(1); opacity: 1; }
  }
  @media (max-width: 720px) {
    .onb-shell {
      grid-template-columns: 1fr;
      grid-template-rows: auto minmax(0, 1fr);
      max-width: calc(100vw - 32px);
    }
  }

  /* ============================================================
     Left rail — branded panel
     ============================================================ */
  .onb-rail {
    position: relative;
    overflow: hidden;
    padding: 28px 24px;
    display: flex;
    flex-direction: column;
    background: linear-gradient(180deg, var(--bg-1) 0%, var(--bg-0) 100%);
    border-right: 1px solid var(--line);
  }
  .onb-moon-glow {
    position: absolute;
    right: -120px;
    top: -120px;
    width: 320px;
    height: 320px;
    border-radius: 50%;
    background: radial-gradient(circle at 30% 30%, var(--accent-soft) 0%, transparent 65%);
    pointer-events: none;
  }
  .onb-ring {
    position: absolute;
    border: 1px solid var(--line-strong);
    border-radius: 50%;
    pointer-events: none;
  }
  .onb-ring-1 {
    left: -100px;
    bottom: -100px;
    width: 260px;
    height: 260px;
    opacity: 0.4;
  }
  .onb-ring-2 {
    left: -60px;
    bottom: -60px;
    width: 180px;
    height: 180px;
    opacity: 0.3;
  }

  .onb-brand {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .onb-brand-name {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .onb-rail-body {
    position: relative;
    margin-top: auto;
  }
  .onb-kicker {
    font-size: 10.5px;
    color: var(--ink-3);
    letter-spacing: 0.1em;
    margin-bottom: 8px;
    font-weight: 500;
    text-transform: uppercase;
  }
  .onb-hero {
    font-size: 22px;
    font-weight: 600;
    line-height: 1.35;
    letter-spacing: -0.015em;
    margin: 0 0 22px;
    color: var(--ink-0);
  }
  .onb-steps {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 12.5px;
  }
  .onb-step {
    display: flex;
    align-items: center;
    gap: 10px;
    transition: opacity 220ms ease;
  }
  .onb-step.future {
    opacity: 0.4;
  }
  .onb-step-mark {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 10px;
    font-weight: 600;
    background: var(--bg-3);
    color: var(--ink-1);
    border: 1.5px solid transparent;
    transition: background 220ms ease, border-color 220ms ease, color 220ms ease;
  }
  .onb-step.active .onb-step-mark {
    background: transparent;
    border-color: var(--accent);
    color: var(--accent);
  }
  .onb-step.done .onb-step-mark {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .onb-step-label {
    color: var(--ink-1);
  }
  .onb-step.active .onb-step-label {
    color: var(--ink-0);
    font-weight: 550;
  }
  .onb-step.future .onb-step-label {
    color: var(--ink-3);
  }

  /* ============================================================
     Main panel
     ============================================================ */
  .onb-main {
    display: flex;
    flex-direction: column;
    min-height: 0;
    position: relative;
  }
  .onb-close {
    position: absolute;
    top: 14px;
    right: 14px;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    background: transparent;
    border: 0;
    color: var(--ink-2);
    cursor: pointer;
    display: grid;
    place-items: center;
    z-index: 2;
    transition: background var(--dur-fast), color var(--dur-fast);
  }
  .onb-close:hover:not(:disabled) {
    background: var(--bg-2);
    color: var(--ink-0);
  }
  .onb-close:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .onb-main-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 36px 40px 16px;
  }
  .onb-title {
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.02em;
    margin: 0 0 8px;
    color: var(--ink-0);
  }
  .onb-lead {
    font-size: 13px;
    color: var(--ink-2);
    margin: 0 0 24px;
    line-height: 1.65;
    max-width: 520px;
  }

  /* Tier card */
  .onb-tier-card {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: var(--accent-soft);
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    margin-bottom: 20px;
  }
  .onb-tier-icon {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--accent);
    display: grid;
    place-items: center;
    flex: 0 0 26px;
  }
  .onb-tier-info {
    flex: 1;
    min-width: 0;
  }
  .onb-tier-name {
    font-size: 12.5px;
    font-weight: 550;
    color: var(--ink-0);
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .onb-tier-name strong {
    font-weight: 600;
    color: var(--ink-0);
  }
  .onb-tier-pill {
    font-size: 11px;
    color: var(--ink-2);
    background: rgba(255, 255, 255, 0.04);
    padding: 2px 8px;
    border-radius: 999px;
    font-weight: 500;
  }
  .onb-tier-reason {
    font-size: 11px;
    color: var(--ink-2);
    margin-top: 4px;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  /* Model cards */
  .onb-models {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 24px;
  }
  .onb-model-card {
    padding: 14px 16px;
    border-radius: var(--r-md);
    background: var(--bg-2);
    border: 1px solid var(--line);
    transition: border-color var(--dur-base) ease, background var(--dur-base) ease;
  }
  .onb-model-card.present {
    background: color-mix(in srgb, #5bb97d 6%, var(--bg-2));
    border-color: color-mix(in srgb, #5bb97d 22%, var(--line));
  }
  .onb-model-card.downloading {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--line));
    background: color-mix(in srgb, var(--accent-soft) 80%, var(--bg-2));
  }
  .onb-model-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 6px;
  }
  .onb-model-name {
    flex: 1;
    font-size: 13px;
    font-weight: 550;
    color: var(--ink-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .onb-model-size {
    font-size: 11.5px;
    color: var(--ink-2);
    font-variant-numeric: tabular-nums;
    flex: 0 0 auto;
  }
  .onb-model-state {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11.5px;
    color: var(--ink-2);
  }
  .onb-state-icon {
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border-radius: 5px;
  }
  .onb-state-present {
    background: color-mix(in srgb, #5bb97d 18%, transparent);
    color: #5bb97d;
  }
  .onb-state-loading {
    background: var(--accent-soft);
    color: var(--accent-strong);
    animation: onb-pulse 1.6s ease-in-out infinite;
  }
  .onb-state-pending {
    background: var(--bg-3);
    color: var(--ink-2);
  }
  @keyframes onb-pulse {
    0%, 100% { opacity: 0.75; }
    50%      { opacity: 1; }
  }
  .onb-state-text {
    flex: 0 0 auto;
  }
  .onb-state-progress {
    margin-left: auto;
    color: var(--ink-1);
    font-size: 11px;
  }
  .onb-progress-track {
    margin-top: 10px;
    height: 4px;
    background: var(--bg-3);
    border-radius: 999px;
    overflow: hidden;
  }
  .onb-progress-fill {
    height: 100%;
    background: linear-gradient(
      90deg,
      var(--accent) 0%,
      var(--accent-strong) 100%
    );
    border-radius: 999px;
    transition: width 220ms ease;
  }

  /* Summary + path */
  .onb-summary {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    margin-bottom: 12px;
  }
  .onb-summary :global(svg) {
    color: var(--accent);
    flex: 0 0 auto;
  }
  .onb-summary-body {
    flex: 1;
    min-width: 0;
  }
  .onb-summary-title {
    font-size: 12.5px;
    font-weight: 550;
    color: var(--ink-0);
  }
  .onb-summary-sub {
    font-size: 11px;
    color: var(--ink-2);
    margin-top: 2px;
  }
  .onb-path {
    font-size: 11px;
    color: var(--ink-3);
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .onb-path code {
    font-family: var(--font-mono);
    color: var(--ink-2);
    font-size: 10.5px;
  }
  .onb-error {
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: rgba(216, 122, 122, 0.1);
    color: #d87a7a;
    border: 1px solid rgba(216, 122, 122, 0.3);
    font-size: 12px;
    margin-bottom: 12px;
  }

  /* Footer */
  .onb-footer {
    flex: 0 0 auto;
    padding: 14px 24px;
    border-top: 1px solid var(--line);
    background: var(--bg-1);
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: flex-end;
    flex-wrap: wrap;
  }
  .onb-footer :global(.btn) {
    flex: 0 0 auto;
  }
  .onb-footer :global(.btn.ghost) {
    margin-right: auto;
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
