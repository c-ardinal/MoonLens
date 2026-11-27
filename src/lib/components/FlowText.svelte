<script lang="ts">
  import type { LensBlock } from '$lib/ipc';
  import { flowLayout, type FlowResult, type Row } from '$lib/flow-layout';

  // Translation laid over the source rows of one block (Lens style). The
  // parent positions the masking box; this fills it.
  let {
    block,
    text,
    scale,
    pad = 2,
    grow = 0,
    background = 'transparent',
    outline = null,
    onlayout
  }: {
    block: LensBlock;
    text: string;
    /** Captured-image px per CSS px. */
    scale: number;
    /** Padding the box adds around the block bbox (CSS px). */
    pad?: number;
    /** Room to the right of the box the text may run into (CSS px). */
    grow?: number;
    /** Mask colour, painted behind text that runs past the box. */
    background?: string;
    /** Outline colour, for text written straight onto a picture. */
    outline?: string | null;
    /** The lines as placed, on every new layout. */
    onlayout?: (layout: FlowResult) => void;
  } = $props();

  let host = $state<HTMLElement | null>(null);
  let layout = $state<FlowResult | null>(null);

  let canvas: HTMLCanvasElement | null = null;

  function rowsOf(b: LensBlock): Row[] {
    const [bx, by] = b.bbox;
    return (b.lines ?? []).map(([x, y, w, h], i) => {
      const limit = b.row_limits?.[i];
      return {
        left: (x - bx) / scale + pad,
        top: (y - by) / scale + pad,
        width: w / scale,
        height: h / scale,
        ...(limit == null ? {} : { limit: (limit - bx) / scale + pad })
      };
    });
  }

  $effect(() => {
    if (!host) return;
    const width = block.bbox[2] / scale + pad * 2;
    const height = block.bbox[3] / scale + pad * 2;
    const cs = getComputedStyle(host);
    const weight = block.bold ? 700 : 500;
    canvas ??= document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    // Measure at 100 px and scale: widths are linear in the font size.
    ctx.font = `${weight} 100px ${cs.fontFamily}`;
    const placed = flowLayout(text, {
      width,
      height,
      rows: rowsOf(block),
      align: block.align ?? 'left',
      maxFont: (block.line_height / scale) * 0.9,
      grow,
      measure: (t) => ctx.measureText(t).width / 100
    });
    layout = placed;
    onlayout?.(placed);
  });
</script>

<div
  bind:this={host}
  class="flow"
  class:outlined={!!outline}
  style="font-weight: {block.bold ? 700 : 500}; --flow-outline: {outline ?? 'transparent'}"
>
  {#if layout}
    {#each layout.lines as l, i (i)}
      <span
        class="flow-line"
        style="left: {l.left}px; top: {l.top}px; width: {l.width}px; height: {l.height}px;
               line-height: {l.height}px; font-size: {layout.fontSize}px;
               text-align: {block.align ?? 'left'}"
        ><span
          class="flow-text"
          style="background: {l.textWidth > l.width ? background : 'transparent'};
                 padding-right: {l.textWidth > l.width ? pad : 0}px">{l.text}</span
        ></span
      >
    {/each}
  {/if}
</div>

<style>
  .flow {
    position: absolute;
    inset: 0;
    /* Clip text that does not fit vertically; a label may still run
       past the box to the right (grow). */
    overflow-x: visible;
    overflow-y: clip;
    font-family: var(--ml-font-plain);
  }
  .flow-line {
    position: absolute;
    white-space: pre;
    overflow: visible;
  }
  /* 縁取り：8 方向の影（静的描画のみ、文字サイズへ比例） */
  .flow.outlined .flow-line {
    --o: 0.06em;
    --d: 0.045em;
    text-shadow:
      var(--o) 0 var(--flow-outline),
      calc(-1 * var(--o)) 0 var(--flow-outline),
      0 var(--o) var(--flow-outline),
      0 calc(-1 * var(--o)) var(--flow-outline),
      var(--d) var(--d) var(--flow-outline),
      calc(-1 * var(--d)) var(--d) var(--flow-outline),
      var(--d) calc(-1 * var(--d)) var(--flow-outline),
      calc(-1 * var(--d)) calc(-1 * var(--d)) var(--flow-outline);
  }
  .flow-text {
    display: inline-block;
    height: 100%;
    border-radius: 3px;
  }
</style>
