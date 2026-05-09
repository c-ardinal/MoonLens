<!--
  Single source of "one engine the user can pick" in the providers
  settings. Replaces the pre-8.x split where:
    - the embedded LLM card lived alone at the top of the providers
      tab and managed download / package / tier UI,
    - the chain sub-tabs further down had their own "+おすすめchip"
      / "削除" / "上下矢印" controls that referenced the same
      provider ids without any visual link to the embedded card.

  Now every engine is one card. The checkbox toggles whether the
  engine is in `providers.<task>.chain`, the priority badge mirrors
  its position in that chain, the ▼ button reveals engine-specific
  config (downloader, API key, endpoint, …) without the user having
  to know that "embedded.llm" and "Qwen 2.5 1.5B" refer to the
  same thing.
-->
<script lang="ts">
  import { ChevronDown, ChevronUp, ArrowUp, ArrowDown, HardDrive, Server, Cloud, Sparkles } from 'lucide-svelte';

  /** Engine kind drives the colour of the type pill so a quick scan
   *  tells the user which engines stay local vs. leave the device. */
  export type EngineKind = 'local' | 'external' | 'cloud';

  /** Status badge shape. Tone selects the colour: `ok` (準備済み),
   *  `warn` (要対応 — DL未済 / APIキー未設定), `pending` (進行中),
   *  `info` (中立)。 */
  export interface EngineStatusBadge {
    label: string;
    tone: 'ok' | 'warn' | 'pending' | 'info';
  }

  interface Props {
    /** Display name. e.g. "組み込みローカル LLM". */
    title: string;
    /** Short description shown beside the title. */
    subtitle?: string;
    /** Stable provider id mapped to `providers.<task>.chain`. */
    providerId: string;
    /** Engine class for the type pill colour. */
    kind: EngineKind;
    /** Whether the user has enabled this engine for the current task. */
    enabled: boolean;
    /** 1-based position in the chain, or null when not enabled. */
    priority: number | null;
    /** Total number of enabled engines, used to disable the "down"
     *  button on the last entry. */
    enabledCount: number;
    /** Status badge — null when no badge should be shown. */
    status?: EngineStatusBadge | null;
    /** Whether the recommended badge should appear. */
    recommended?: boolean;
    /** Controlled detail state — parent owns it so multiple cards can
     *  share an "expand all" button later if needed. */
    detailOpen?: boolean;
    /** Disable all interactions while the parent is mid-save. */
    disabled?: boolean;
    /** Toggle handler. Receives the new enabled state. */
    onToggle: (next: boolean) => void;
    /** Move handlers — only meaningful when `enabled` is true. */
    onMoveUp?: (() => void) | null;
    onMoveDown?: (() => void) | null;
    /** Detail toggle handler — null hides the chevron entirely. */
    onToggleDetail?: (() => void) | null;
  }

  const {
    title,
    subtitle = '',
    providerId,
    kind,
    enabled,
    priority,
    enabledCount,
    status = null,
    recommended = false,
    detailOpen = false,
    disabled = false,
    onToggle,
    onMoveUp = null,
    onMoveDown = null,
    onToggleDetail = null,
    children,
  }: Props & { children?: import('svelte').Snippet } = $props();

  const kindLabel = $derived(
    kind === 'local' ? 'ローカル' : kind === 'external' ? '外部サーバ' : 'クラウド',
  );
</script>

<article
  class="engine-card"
  class:enabled
  class:disabled-shell={disabled}
  data-kind={kind}
  data-provider-id={providerId}
>
  <header class="card-head">
    <label class="check-row">
      <input
        type="checkbox"
        checked={enabled}
        {disabled}
        onchange={(e) => onToggle((e.target as HTMLInputElement).checked)}
        aria-label={`${title} を有効化`}
      />
      <span class="priority-badge" class:invisible={!enabled} aria-hidden={!enabled}>
        {enabled && priority != null ? priority : '─'}
      </span>
      <span class="title-block">
        <span class="title-row">
          <span class="title">{title}</span>
          <span class="kind-pill kind-{kind}">
            {#if kind === 'local'}
              <HardDrive class="h-3 w-3" aria-hidden="true" />
            {:else if kind === 'external'}
              <Server class="h-3 w-3" aria-hidden="true" />
            {:else}
              <Cloud class="h-3 w-3" aria-hidden="true" />
            {/if}
            {kindLabel}
          </span>
          {#if recommended}
            <span class="recommended-pill" title="このティアが推奨されています">
              <Sparkles class="h-3 w-3" aria-hidden="true" />
              推奨
            </span>
          {/if}
        </span>
        {#if subtitle}
          <span class="subtitle">{subtitle}</span>
        {/if}
      </span>
    </label>

    <div class="card-actions">
      {#if status}
        <span class="status-badge tone-{status.tone}">{status.label}</span>
      {/if}
      {#if enabled && (onMoveUp || onMoveDown)}
        <span class="move-buttons">
          <button
            type="button"
            class="icon-btn"
            title="優先度を上げる"
            aria-label="優先度を上げる"
            disabled={disabled || !onMoveUp || priority === 1}
            onclick={onMoveUp ?? undefined}
          >
            <ArrowUp class="h-3.5 w-3.5" aria-hidden="true" />
          </button>
          <button
            type="button"
            class="icon-btn"
            title="優先度を下げる"
            aria-label="優先度を下げる"
            disabled={disabled || !onMoveDown || priority === enabledCount}
            onclick={onMoveDown ?? undefined}
          >
            <ArrowDown class="h-3.5 w-3.5" aria-hidden="true" />
          </button>
        </span>
      {/if}
      {#if onToggleDetail}
        <button
          type="button"
          class="icon-btn"
          aria-expanded={detailOpen}
          aria-label={detailOpen ? '詳細を閉じる' : '詳細を開く'}
          onclick={onToggleDetail}
          {disabled}
        >
          {#if detailOpen}
            <ChevronUp class="h-4 w-4" aria-hidden="true" />
          {:else}
            <ChevronDown class="h-4 w-4" aria-hidden="true" />
          {/if}
        </button>
      {/if}
    </div>
  </header>

  {#if detailOpen && children}
    <section class="card-detail">
      {@render children()}
    </section>
  {/if}
</article>

<style>
  .engine-card {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    transition: border-color 120ms ease, background 120ms ease;
  }
  .engine-card.enabled {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
    background: color-mix(in srgb, var(--accent-soft) 50%, var(--surface));
  }
  .engine-card.disabled-shell {
    opacity: 0.6;
    pointer-events: none;
  }

  .card-head {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
  }

  .check-row {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex: 1;
    cursor: pointer;
    min-width: 0;
  }
  .check-row input[type='checkbox'] {
    width: 1rem;
    height: 1rem;
    accent-color: var(--accent);
    flex-shrink: 0;
  }

  .priority-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    font-weight: 600;
    background: var(--accent);
    color: white;
    flex-shrink: 0;
  }
  .priority-badge.invisible {
    visibility: hidden;
  }

  .title-block {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    min-width: 0;
    flex: 1;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .title {
    font-weight: 500;
    font-size: 0.9375rem;
    color: var(--ink);
  }
  .subtitle {
    color: var(--ink-muted);
    font-size: 0.8125rem;
    line-height: 1.35;
  }

  .kind-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.0625rem 0.375rem;
    border-radius: 999px;
    font-size: 0.6875rem;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }
  .kind-pill.kind-local {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
  }
  .kind-pill.kind-external {
    background: color-mix(in srgb, #f59e0b 18%, transparent);
    color: #f59e0b;
  }
  .kind-pill.kind-cloud {
    background: color-mix(in srgb, #6366f1 18%, transparent);
    color: #6366f1;
  }

  .recommended-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.0625rem 0.375rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent);
    font-size: 0.6875rem;
    font-weight: 600;
  }

  .card-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .status-badge {
    padding: 0.125rem 0.5rem;
    border-radius: 999px;
    font-size: 0.75rem;
    font-weight: 500;
    white-space: nowrap;
  }
  .status-badge.tone-ok {
    background: color-mix(in srgb, #10b981 18%, transparent);
    color: #10b981;
  }
  .status-badge.tone-warn {
    background: color-mix(in srgb, #f59e0b 22%, transparent);
    color: #f59e0b;
  }
  .status-badge.tone-pending {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent);
  }
  .status-badge.tone-info {
    background: var(--surface-muted);
    color: var(--ink-muted);
  }

  .move-buttons {
    display: inline-flex;
    align-items: center;
    gap: 0.125rem;
  }

  .icon-btn {
    width: 1.75rem;
    height: 1.75rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-elevated);
    color: var(--ink-muted);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: border-color 120ms ease, color 120ms ease;
  }
  .icon-btn:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--accent) 60%, var(--border));
    color: var(--ink);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .card-detail {
    border-top: 1px solid var(--border);
    padding: 0.75rem 1rem 1rem 3.125rem; /* indent past the checkbox column */
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
    background: var(--surface-elevated);
  }
</style>
