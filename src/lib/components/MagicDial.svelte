<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Pix } from '$lib/pixel/pix';
  import { drawDial, dialGeometry, hitTest, type DialItem } from '$lib/pixel/dial';

  /**
   * ルーン魔法陣のメニュー（ホーム背景層）。
   * 再描画は選択・ホバー・寸法の変化時のみ。常時アニメーションなし。
   */
  let {
    items,
    selected = $bindable(0),
    anchor = null,
    reserve = 440,
    onactivate
  }: {
    items: DialItem[];
    selected?: number;
    /** 接続線の終点となる見出し帯。 */
    anchor?: HTMLElement | null;
    /** 右側の情報欄に確保する幅（CSS px）。 */
    reserve?: number;
    onactivate: (index: number) => void;
  } = $props();

  let canvas: HTMLCanvasElement;
  let host: HTMLDivElement;
  let pix: Pix | null = null;
  let dot = 2;
  let hover = $state<number | 'center' | null>(null);
  let size = $state({ w: 0, h: 0 });
  let ro: ResizeObserver | null = null;

  /**
   * ドット 1 個の CSS 寸法：HTML 側の 16 ドット文字（16px）と同一の格子。
   * 端末画素の整数倍へ丸め、拡大率 125% 等でも不均一な打点を防止。
   */
  function dotSize() {
    const dpr = window.devicePixelRatio || 1;
    return Math.max(1, Math.round(dpr)) / dpr;
  }

  function measure() {
    const r = host.getBoundingClientRect();
    dot = dotSize();
    size = { w: Math.floor(r.width / dot), h: Math.floor(r.height / dot) };
  }

  function anchorPoint() {
    if (!anchor || !canvas) return undefined;
    const a = anchor.getBoundingClientRect(),
      c = canvas.getBoundingClientRect();
    return { x: Math.round((a.left - c.left) / dot), y: Math.round((a.top + a.height * 0.7 - c.top) / dot) };
  }

  $effect(() => {
    const { w, h } = size;
    const state = { items, selected, hover };
    if (!canvas || w < 40 || h < 40) return;
    if (!pix || pix.w !== w || pix.h !== h) {
      pix = new Pix(canvas, w, h);
      canvas.style.width = `${w * dot}px`;
      canvas.style.height = `${h * dot}px`;
    }
    drawDial(pix, { ...state, anchor: anchorPoint() }, Math.round(reserve / dot));
  });

  function toLogical(e: PointerEvent) {
    const r = canvas.getBoundingClientRect();
    return { x: (e.clientX - r.left) / dot, y: (e.clientY - r.top) / dot };
  }
  function onMove(e: PointerEvent) {
    if (!pix) return;
    const { x, y } = toLogical(e);
    const next = hitTest(dialGeometry(pix.w, pix.h, items.length, selected, Math.round(reserve / dot)), x, y);
    if (next !== hover) hover = next;
  }
  function onClick(e: MouseEvent) {
    if (!pix) return;
    const { x, y } = toLogical(e as PointerEvent);
    const hit = hitTest(dialGeometry(pix.w, pix.h, items.length, selected, Math.round(reserve / dot)), x, y);
    if (hit === null) return;
    const index = hit === 'center' ? selected : hit;
    selected = index;
    hover = null;
    onactivate(index);
  }

  onMount(() => {
    measure();
    ro = new ResizeObserver(measure);
    ro.observe(host);
    // 書体の読込完了後に文字を再描画
    document.fonts?.load('16px DotGothic16').then(() => (size = { ...size }));
  });
  onDestroy(() => ro?.disconnect());
</script>

<div class="dial" bind:this={host}>
  <!-- 操作はキーボード（←→・Enter・数字）とボタン群でも可能。Canvas は視覚表現。 -->
  <canvas
    bind:this={canvas}
    aria-hidden="true"
    class:pointer={hover !== null}
    onpointermove={onMove}
    onpointerleave={() => (hover = null)}
    onclick={onClick}
  ></canvas>
</div>

<style>
  .dial {
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
