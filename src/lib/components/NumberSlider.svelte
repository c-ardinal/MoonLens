<script lang="ts">
  /**
   * Range + numeric input pair. The number input is the source of
   * truth so users can paste exact values; the slider is a visual
   * aid bound to the same underlying state.
   */
  interface Props {
    value: number;
    onChange: (next: number) => void;
    min: number;
    max: number;
    step?: number;
    suffix?: string;
    formatter?: (value: number) => string;
    'aria-label'?: string;
  }

  let {
    value,
    onChange,
    min,
    max,
    step = 1,
    suffix,
    formatter,
    'aria-label': ariaLabel
  }: Props = $props();

  function formatted(v: number): string {
    return formatter ? formatter(v) : v.toString();
  }

  function clamp(v: number): number {
    if (Number.isNaN(v)) return min;
    return Math.min(max, Math.max(min, v));
  }
</script>

<div class="slider-wrap">
  <input
    type="range"
    class="slider"
    aria-label={ariaLabel ?? 'value'}
    {min}
    {max}
    {step}
    value={value}
    oninput={(e) => onChange(clamp(Number((e.target as HTMLInputElement).value)))}
  />
  <output class="slider-value mono">
    {formatted(value)}{#if suffix}<span class="suffix">{suffix}</span>{/if}
  </output>
</div>

<style>
  .slider-wrap {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    min-width: 220px;
  }
  .slider {
    flex: 1;
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    cursor: pointer;
    height: 18px;
  }
  .slider::-webkit-slider-runnable-track {
    height: 3px;
    background: var(--gauge-track);
    border-radius: 2px;
  }
  .slider::-moz-range-track {
    height: 3px;
    background: var(--gauge-track);
    border-radius: 2px;
  }
  .slider::-webkit-slider-thumb {
    appearance: none;
    -webkit-appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent);
    border: 2px solid var(--bg-1);
    box-shadow: 0 0 0 1px var(--accent);
    margin-top: -6px;
    cursor: pointer;
  }
  .slider::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    border: 2px solid var(--bg-1);
    box-shadow: 0 0 0 1px var(--accent);
    cursor: pointer;
  }
  .slider:focus-visible {
    outline: none;
  }
  .slider:focus-visible::-webkit-slider-thumb {
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .slider-value {
    min-width: 56px;
    text-align: right;
    font-size: 11.5px;
    color: var(--ink-1);
    font-variant-numeric: tabular-nums;
  }
  .suffix {
    margin-left: 2px;
    color: var(--ink-3);
    font-size: 10.5px;
  }
</style>
