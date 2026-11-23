<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Pix } from '$lib/pixel/pix';
  import { dialGeometry, hitTest } from '$lib/pixel/dial';
  import { drawHome, homeRegions, type HomeRegion, type HomeView } from '$lib/pixel/home';

  /**
   * ホーム画面の描画層（2 倍ドット）。魔法陣と右側の情報欄を 1 枚の Canvas へ描画。
   * 再描画は表示内容・選択・ホバー・寸法の変化時のみ。常時アニメーションなし。
   */
  let {
    view,
    selected = $bindable(0),
    onactivate,
    onregion
  }: {
    view: Omit<HomeView, 'hover' | 'hot' | 'selected'>;
    selected?: number;
    onactivate: (index: number) => void;
    onregion: (region: HomeRegion) => void;
  } = $props();

  let canvas: HTMLCanvasElement;
  let host: HTMLDivElement;
  let pix: Pix | null = null;
  let dot = 2;
  let hover = $state<number | 'center' | null>(null);
  let hot = $state<HomeRegion | null>(null);
  let size = $state({ w: 0, h: 0 });
  let fontsReady = $state(false);
  let ro: ResizeObserver | null = null;

  /** ドット 1 個の CSS 寸法（2 倍表示）。端末画素の整数倍へ丸め、不均一な打点を防止。 */
  function dotSize() {
    const dpr = window.devicePixelRatio || 1;
    return Math.max(1, Math.round(2 * dpr)) / dpr;
  }

  function measure() {
    const r = host.getBoundingClientRect();
    dot = dotSize();
    size = { w: Math.floor(r.width / dot), h: Math.floor(r.height / dot) };
  }

  $effect(() => {
    const { w, h } = size;
    const state = { ...view, selected, hover, hot };
    if (!canvas || !fontsReady || w < 200 || h < 200) return;
    if (!pix || pix.w !== w || pix.h !== h) {
      pix = new Pix(canvas, w, h);
      canvas.style.width = `${w * dot}px`;
      canvas.style.height = `${h * dot}px`;
    }
    drawHome(pix, state);
  });

  function locate(e: MouseEvent): { dial: number | 'center' | null; region: HomeRegion | null } {
    if (!pix) return { dial: null, region: null };
    const r = canvas.getBoundingClientRect();
    const x = (e.clientX - r.left) / dot,
      y = (e.clientY - r.top) / dot;
    const regions = homeRegions(pix.w, pix.h);
    for (const [k, b] of Object.entries(regions))
      if (x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h) return { dial: null, region: k as HomeRegion };
    return { dial: hitTest(dialGeometry(pix.w, pix.h, view.items.length, selected), x, y), region: null };
  }
  function onMove(e: PointerEvent) {
    const l = locate(e);
    if (l.dial !== hover) hover = l.dial;
    if (l.region !== hot) hot = l.region;
  }
  function onClick(e: MouseEvent) {
    const l = locate(e);
    if (l.region) return onregion(l.region);
    if (l.dial === null) return;
    const index = l.dial === 'center' ? selected : l.dial;
    selected = index;
    hover = null;
    onactivate(index);
  }

  onMount(() => {
    measure();
    ro = new ResizeObserver(measure);
    ro.observe(host);
    Promise.all([
      document.fonts?.load('16px DotGothic16'),
      document.fonts?.load("700 44px 'BIZ UDGothic','Yu Gothic UI','Hiragino Sans','Noto Sans JP',sans-serif", '範囲全画面翻訳')
    ])
      .catch(() => {})
      .finally(() => (fontsReady = true));
  });
  onDestroy(() => ro?.disconnect());
</script>

<div class="home-canvas" bind:this={host}>
  <!-- 視覚表現。操作はキーボード（←→・Enter・数字）と支援技術向けボタン群でも可能。 -->
  <canvas
    bind:this={canvas}
    aria-hidden="true"
    class:pointer={hover !== null || hot !== null}
    onpointermove={onMove}
    onpointerleave={() => ((hover = null), (hot = null))}
    onclick={onClick}
  ></canvas>
</div>

<style>
  .home-canvas {
    position: absolute;
    inset: 0;
    overflow: hidden;
  }
  canvas {
    display: block;
    image-rendering: pixelated;
  }
  canvas.pointer {
    cursor: pointer;
  }
</style>
