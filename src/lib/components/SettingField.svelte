<script lang="ts">
  import { t } from '$lib/i18n';
  import { RotateCcw } from 'lucide-svelte';
  import type { Field } from '$lib/settings-meta';
  import HotkeyInput from './HotkeyInput.svelte';

  interface Props {
    field: Field;
    value: any;
    defaultValue?: any;
    /** The value clashes with another field (e.g. a duplicate shortcut). */
    invalid?: boolean;
    /** 設定画面の行表示：説明文は外部の説明欄へ委譲し、1 行で表示。 */
    compact?: boolean;
    onChange: (v: any) => void;
  }
  let { field, value, defaultValue, invalid = false, compact = false, onChange }: Props = $props();

  const id = $derived(`f-${field.path.replace(/\./g, '-')}`);
  // f32 defaults arrive as 0.9200000166893005; null and missing are the same.
  const same = (a: any, b: any) =>
    typeof a === 'number' && typeof b === 'number'
      ? Math.abs(a - b) < 1e-6
      : JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
  const isDefault = $derived(defaultValue === undefined || same(value, defaultValue));

  function fmt(n: number) {
    if (field.kind !== 'number') return String(n);
    const decimals = String(field.step).split('.')[1]?.length ?? 0;
    return Number(n).toFixed(decimals);
  }

  function clamp(n: number) {
    if (field.kind !== 'number' || Number.isNaN(n)) return value;
    const v = Math.min(field.max, Math.max(field.min, n));
    // Integer settings (u32 on the Rust side) must not receive 2.5.
    return Number.isInteger(field.step) ? Math.round(v) : v;
  }
</script>

<div
  class={compact
    ? 'flex min-h-[var(--ml-row-min)] items-center justify-between gap-6 py-[var(--ml-row-pad)]'
    : 'flex flex-col gap-2 py-3 sm:flex-row sm:items-center sm:justify-between sm:gap-6'}
>
  <div class="min-w-0 flex-1">
    <label for={id} class="text-sm font-medium {invalid ? 'text-red-500' : 'text-ink'}">
      {t(field.label)}{#if invalid}<span class="ml-2 text-xs font-normal">{t('重複')}</span>{/if}
    </label>
    {#if field.help && !compact}
      <p class="mt-0.5 text-xs leading-relaxed text-ink-muted">{t(field.help)}</p>
    {/if}
  </div>
  <!-- 設定画面：操作部は固定幅の右寄せ列（行間で左右端を統一） -->
  <div class={compact ? 'ctl' : 'flex min-w-0 shrink-0 items-center gap-2 sm:max-w-[55%]'}>
    {#if field.kind === 'toggle' && compact}
      <!-- ON／OFF の矩形札（チェックボックスは支援技術向けに保持） -->
      <label class="toggle-chips">
        <input
          {id}
          type="checkbox"
          class="sr-only"
          checked={!!value}
          onchange={(e) => onChange((e.currentTarget as HTMLInputElement).checked)}
        />
        <span class="tc" class:on={!!value}>ON</span>
        <span class="tc" class:on={!value}>OFF</span>
      </label>
    {:else if field.kind === 'toggle'}
      <label class="relative inline-flex cursor-pointer items-center">
        <input
          {id}
          type="checkbox"
          class="peer sr-only"
          checked={!!value}
          onchange={(e) => onChange((e.currentTarget as HTMLInputElement).checked)}
        />
        <span class="h-5 w-9 rounded-full bg-surface-muted ring-1 ring-border transition peer-checked:bg-accent"></span>
        <span
          data-grid-exempt
          class="absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white shadow transition peer-checked:translate-x-4"
        ></span>
      </label>
    {:else if field.kind === 'select'}
      <select
        {id}
        class="max-w-full rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm"
        value={value ?? ''}
        onchange={(e) => onChange((e.currentTarget as HTMLSelectElement).value)}
      >
        {#if !field.options.some((o) => o.value === value)}
          <option value={value ?? ''}>{value ?? t('未設定')}</option>
        {/if}
        {#each field.options as o (o.value)}
          <option value={o.value}>{t(o.label)}</option>
        {/each}
      </select>
    {:else if field.kind === 'number'}
      {#if field.slider}
        <input
          {id}
          type="range"
          class="w-36 accent-[var(--ml-accent)] sm:w-44"
          min={field.min}
          max={field.max}
          step={field.step}
          value={value ?? field.min}
          oninput={(e) => onChange(Number((e.currentTarget as HTMLInputElement).value))}
        />
        <span class="value-text min-w-24 whitespace-nowrap text-right text-sm tabular-nums text-ink-muted">
          {#if field.zeroLabel && !value}
            {t(field.zeroLabel)}
          {:else}
            {fmt(value ?? field.min)}{field.unit ? ` ${t(field.unit)}` : ''}
          {/if}
        </span>
      {:else}
        <input
          {id}
          type="number"
          class="w-36 rounded-ml-sm border border-border bg-surface px-2 py-1 text-right text-sm tabular-nums"
          min={field.min}
          max={field.max}
          step={field.step}
          value={value ?? 0}
          onchange={(e) => onChange(clamp(Number((e.currentTarget as HTMLInputElement).value)))}
        />
        <span class="value-text min-w-16 whitespace-nowrap text-sm text-ink-muted">
          {field.zeroLabel && !value ? t(field.zeroLabel) : t(field.unit ?? '')}
        </span>
      {/if}
    {:else if field.kind === 'color'}
      <input
        {id}
        type="color"
        class="h-8 w-12 cursor-pointer rounded-ml-sm border border-border bg-surface p-0.5"
        value={/^#[0-9a-f]{6}$/i.test(value ?? '') ? value : '#ffb547'}
        oninput={(e) => onChange((e.currentTarget as HTMLInputElement).value)}
      />
      <code class="font-mono text-xs text-ink-muted">{value}</code>
    {:else if field.kind === 'hotkey'}
      <HotkeyInput value={value ?? ''} label={t(field.label)} {onChange} />
    {/if}
    <button
      type="button"
      class="reset rounded-ml-sm p-1 text-ink-faint transition hover:bg-surface-muted hover:text-ink {isDefault
        ? 'invisible'
        : ''}"
      title={t('初期値に復元')}
      aria-label={t('{label}：初期値に復元', { label: t(field.label) })}
      onclick={() => onChange(defaultValue)}
    >
      <RotateCcw class="h-3.5 w-3.5" />
    </button>
  </div>
</div>

<style>
  .ctl {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: flex-end;
    gap: var(--ml-sp-2);
    /* 下限：最長のショートカット表記（Ctrl+Shift+Space）＋初期化ボタンを収容 */
    width: clamp(320px, 45%, 440px);
    min-width: 0;
  }
  .ctl select,
  .ctl > :global(button:not(.reset)) {
    flex: 1;
    min-width: 0;
  }
  .ctl input[type='range'] {
    flex: 1;
    width: auto;
    min-width: 0;
  }
</style>
