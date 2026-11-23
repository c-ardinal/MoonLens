<script lang="ts">
  import { onMount } from 'svelte';
  import { Pix, TITLE_FONT_FAMILY } from '$lib/pixel/pix';

  /**
   * 斜めの巨大見出し帯：琥珀の帯＋骨白の段影＋2 値化した斜体文字。
   * 文字は高精細な太字を 1 ドット単位で 2 値化し、字形の潰れを回避。
   */
  let {
    text,
    note = '',
    height = 52,
    tag = 'h2',
    fit = false,
    el = $bindable(null)
  }: {
    text: string;
    note?: string;
    height?: number;
    tag?: 'h1' | 'h2';
    /** true：帯の長さを文字幅へ合わせる（各画面の見出し）。 */
    fit?: boolean;
    el?: HTMLElement | null;
  } = $props();

  const K = 0.3;
  let canvas: HTMLCanvasElement;
  let host: HTMLDivElement;
  let width = $state(0);
  let dot = 2;
  let ready = $state(false);

  function measure() {
    const dpr = window.devicePixelRatio || 1;
    // 端末画素の整数倍（最大 2 倍表示）。125% 表示は 2 端末画素 = 1.6 CSS px
    dot = Math.max(1, Math.floor(2 * dpr + 1e-6)) / dpr;
    width = Math.floor(host.getBoundingClientRect().width / dot);
  }

  $effect(() => {
    const w = width,
      label = text,
      sub = note,
      h = height;
    if (!canvas || !ready) return;
    const size = Math.round(h * 0.78);
    const font = `700 ${size}px ${TITLE_FONT_FAMILY}`;
    let bw = w;
    if (fit) {
      const g = canvas.getContext('2d');
      if (g) g.font = font;
      bw = Math.ceil((g?.measureText(label).width ?? 0) + h * 0.3 + 56);
    }
    if (bw < 40) return;
    const ph = h + 6;
    const p = new Pix(canvas, bw, ph);
    canvas.style.width = `${bw * dot}px`;
    canvas.style.height = `${ph * dot}px`;
    p.clear();
    p.slab(0, 6, bw - 10, h, K, 9); // 段影
    p.slab(10, 0, bw - 10, h, K, 5); // 帯
    p.text(label, 30, Math.round((h - size * 1.15) / 2) + 1, 0, { k: K, font });
    if (sub) p.text(sub, bw - 10, 4, 0, { align: 'right' });
  });

  onMount(() => {
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(host);
    document.fonts
      ?.load(`700 32px ${TITLE_FONT_FAMILY}`, text)
      .catch(() => {})
      .finally(() => (ready = true));
    return () => ro.disconnect();
  });
</script>

<div class="title" bind:this={host}>
  <div bind:this={el} class="anchor" aria-hidden="true"></div>
  <canvas bind:this={canvas} aria-hidden="true"></canvas>
  <svelte:element this={tag} class="sr-only">{text}</svelte:element>
</div>

<style>
  .title {
    position: relative;
    width: 100%;
  }
  .anchor {
    position: absolute;
    left: 12px;
    top: 0;
    width: 1px;
    height: 100%;
  }
  canvas {
    display: block;
    image-rendering: pixelated;
  }
</style>
