<script lang="ts">
  import { RotateCcw } from 'lucide-svelte';
  import type { Field } from '$lib/settings-meta';
  import HotkeyInput from './HotkeyInput.svelte';

  interface Props {
    field: Field;
    value: any;
    defaultValue?: any;
    onChange: (v: any) => void;
  }
  let { field, value, defaultValue, onChange }: Props = $props();

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

<div class="flex flex-col gap-2 py-3 sm:flex-row sm:items-center sm:justify-between sm:gap-6">
  <div class="min-w-0 flex-1">
    <label for={id} class="text-sm font-medium text-ink">{field.label}</label>
    {#if field.help}
      <p class="mt-0.5 text-xs leading-relaxed text-ink-muted">{field.help}</p>
    {/if}
  </div>
  <div class="flex min-w-0 shrink-0 items-center gap-2 sm:max-w-[55%]">
    {#if field.kind === 'toggle'}
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
          <option value={value ?? ''}>{value ?? '未設定'}</option>
        {/if}
        {#each field.options as o (o.value)}
          <option value={o.value}>{o.label}</option>
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
        <span class="w-24 text-right text-sm tabular-nums text-ink-muted">
          {#if field.zeroLabel && !value}
            {field.zeroLabel}
          {:else}
            {fmt(value ?? field.min)}{field.unit ? ` ${field.unit}` : ''}
          {/if}
        </span>
      {:else}
        <input
          {id}
          type="number"
          class="w-28 rounded-ml-sm border border-border bg-surface px-2 py-1 text-right text-sm tabular-nums"
          min={field.min}
          max={field.max}
          step={field.step}
          value={value ?? 0}
          onchange={(e) => onChange(clamp(Number((e.currentTarget as HTMLInputElement).value)))}
        />
        <span class="w-16 text-sm text-ink-muted">
          {field.zeroLabel && !value ? field.zeroLabel : (field.unit ?? '')}
        </span>
      {/if}
    {:else if field.kind === 'color'}
      <input
        {id}
        type="color"
        class="h-8 w-12 cursor-pointer rounded-ml-sm border border-border bg-surface p-0.5"
        value={/^#[0-9a-f]{6}$/i.test(value ?? '') ? value : '#8b5cf6'}
        oninput={(e) => onChange((e.currentTarget as HTMLInputElement).value)}
      />
      <code class="font-mono text-xs text-ink-muted">{value}</code>
    {:else if field.kind === 'hotkey'}
      <HotkeyInput value={value ?? ''} label={field.label} {onChange} />
    {/if}
    <button
      type="button"
      class="rounded-ml-sm p-1 text-ink-faint transition hover:bg-surface-muted hover:text-ink {isDefault
        ? 'invisible'
        : ''}"
      title="既定値に戻す"
      aria-label="{field.label}を既定値に戻す"
      onclick={() => onChange(defaultValue)}
    >
      <RotateCcw class="h-3.5 w-3.5" />
    </button>
  </div>
</div>
