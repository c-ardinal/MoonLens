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
    background = 'transparent'
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
  } = $props();

  let host = $state<HTMLElement | null>(null);
  let layout = $state<FlowResult | null>(null);

  let canvas: HTMLCanvasElement | null = null;

  function rowsOf(b: LensBlock): Row[] {
    const [bx, by] = b.bbox;
    return (b.lines ?? []).map(([x, y, w, h]) => ({
      left: (x - bx) / scale + pad,
      top: (y - by) / scale + pad,
      width: w / scale,
      height: h / scale
    }));
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
    layout = flowLayout(text, {
      width,
      height,
      rows: rowsOf(block),
      align: block.align ?? 'left',
      maxFont: (block.line_height / scale) * 0.9,
      grow,
      measure: (t) => ctx.measureText(t).width / 100
    });
  });
</script>

<div bind:this={host} class="flow" style="font-weight: {block.bold ? 700 : 500}">
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
    font-family: var(--ml-font-ui);
  }
  .flow-line {
    position: absolute;
    white-space: pre;
    overflow: visible;
  }
  .flow-text {
    display: inline-block;
    height: 100%;
    border-radius: 3px;
  }
</style>
