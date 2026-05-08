<script lang="ts">
  /**
   * MoonLens brand mark — moon inside a lens aperture, with optional
   * pulsing glow. Sized in pixels via `size`. Colours are pulled from
   * the active theme via CSS custom properties so the mark adapts when
   * the accent or ink palette switches.
   */
  interface Props {
    size?: number;
    glow?: boolean;
  }

  let { size = 26, glow = false }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 32 32"
  fill="none"
  style="overflow: visible"
  aria-hidden="true"
  data-testid="moonlens-mark"
>
  <defs>
    <radialGradient id="mlmoon" cx="0.42" cy="0.4" r="0.7">
      <stop offset="0%" stop-color="var(--accent)" stop-opacity="0.95" />
      <stop offset="100%" stop-color="var(--accent)" stop-opacity="0.45" />
    </radialGradient>
    <linearGradient id="mlring" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="var(--ink-1)" stop-opacity="0.9" />
      <stop offset="100%" stop-color="var(--ink-2)" stop-opacity="0.6" />
    </linearGradient>
  </defs>
  <circle
    cx="16"
    cy="16"
    r="13"
    stroke="url(#mlring)"
    stroke-width="1.4"
    fill="none"
  />
  <path
    d="M 16 5 A 11 11 0 1 0 16 27 A 8 8 0 1 1 16 5 Z"
    fill="url(#mlmoon)"
  />
  <g
    stroke="var(--ink-2)"
    stroke-width="1.1"
    stroke-linecap="round"
    opacity="0.55"
  >
    <line x1="16" y1="0.6" x2="16" y2="2.6" />
    <line x1="16" y1="29.4" x2="16" y2="31.4" />
    <line x1="0.6" y1="16" x2="2.6" y2="16" />
    <line x1="29.4" y1="16" x2="31.4" y2="16" />
  </g>
  {#if glow}
    <circle
      cx="16"
      cy="16"
      r="13"
      fill="none"
      stroke="var(--accent)"
      stroke-width="1"
      opacity="0.4"
    >
      <animate attributeName="r" from="13" to="17" dur="2s" repeatCount="indefinite" />
      <animate attributeName="opacity" from="0.4" to="0" dur="2s" repeatCount="indefinite" />
    </circle>
  {/if}
</svg>
