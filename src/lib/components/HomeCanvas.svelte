<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { Pix } from '$lib/pixel/pix';
  import { dialGeometry, hitTest } from '$lib/pixel/dial';
  import { drawHome, drawHomeMotion, homeDot, homeRegions, type HomeRegion, type HomeView } from '$lib/pixel/home';
  import { motion } from '$lib/pixel/motion.svelte';

  /**
   * ホーム画面の描画層。2 枚の Canvas を重ねる構成。
   * - 下層（動作層）：ルーン環の回転・光点・波紋・中心核。12 fps で再描画（文字処理なし）。
   * - 上層（静止層）：文字・枠・ノード。状態変化時と選択切替の演出中のみ再描画。
   * 推論中・動き削減設定時・非表示時は演出を停止し、静止画のみ描画。
   */
  let {
    view,
    selected = $bindable(0),
    onactivate,
    onregion
  }: {
    view: Omit<HomeView, 'hover' | 'hot' | 'selected' | 'turn' | 'slide'>;
    selected?: number;
    onactivate: (index: number) => void;
    onregion: (region: HomeRegion) => void;
  } = $props();

  const FRAME_MS = 1000 / 12;
  const TURN_MS = 320;

  let under: HTMLCanvasElement;
  let canvas: HTMLCanvasElement;
  let host: HTMLDivElement;
  let top: Pix | null = null;
  let low: Pix | null = null;
  let dot = 2;
  let hover = $state<number | null>(null);
  let hot = $state<HomeRegion | null>(null);
  let size = $state({ w: 0, h: 0 });
  let fontsReady = $state(false);
  let ro: ResizeObserver | null = null;

  /** 選択切替の演出：表示位置 turn を from から selected へ補間。 */
  let anim = $state<{ from: number; to: number; start: number } | null>(null);
  let turn = $state<number | undefined>(undefined);
  let slide = $state(1);
  let shown = selected;
  let clock = 0;

  function measure() {
    const r = host.getBoundingClientRect();
    dot = homeDot(r.width, r.height, window.devicePixelRatio || 1);
    size = { w: Math.floor(r.width / dot), h: Math.floor(r.height / dot) };
  }

  function ensure() {
    const { w, h } = size;
    if (!top || top.w !== w || top.h !== h || low?.w !== w) {
      top = new Pix(canvas, w, h);
      low = new Pix(under, w, h);
      for (const c of [canvas, under]) {
        c.style.width = `${w * dot}px`;
        c.style.height = `${h * dot}px`;
      }
    }
  }
  const ok = () => canvas && under && fontsReady && size.w >= 200 && size.h >= 200;

  // 選択の変化を検知し、最短方向の回転演出を開始
  $effect(() => {
    const to = selected;
    if (to === shown) return;
    const n = view.items.length;
    let d = (((to - shown) % n) + n) % n;
    if (d > n / 2) d -= n;
    const from = to - d;
    shown = to;
    if (!motion.on) return;
    turn = from;
    slide = 0;
    anim = { from, to, start: performance.now() };
  });

  // 静止層
  $effect(() => {
    const state = { ...view, selected, hover, hot, turn, slide };
    void size;
    if (!ok()) return;
    ensure();
    drawHome(top!, state);
    if (!motion.on || anim) drawHomeMotion(low!, { items: view.items, selected, turn }, clock);
  });

  // 動作層のループ（12 fps）。停止条件では解除
  $effect(() => {
    if (!motion.on || !fontsReady) return;
    let raf = 0;
    let last = -1;
    const t0 = performance.now() - clock * 1000;
    const tick = (now: number) => {
      raf = requestAnimationFrame(tick);
      const f = Math.floor((now - t0) / FRAME_MS);
      if (f === last || !ok()) return;
      last = f;
      clock = (f * FRAME_MS) / 1000;
      if (anim) {
        const u = Math.min(1, (now - anim.start) / TURN_MS);
        const e = 1 - (1 - u) ** 3;
        turn = u >= 1 ? undefined : anim.from + (anim.to - anim.from) * e;
        slide = u;
        if (u >= 1) anim = null;
      }
      ensure();
      drawHomeMotion(low!, { items: view.items, selected, turn }, clock);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      // 演出途中の停止（推論開始など）：回転を完了状態へ確定し、表示と当り判定を一致
      if (untrack(() => anim)) {
        anim = null;
        turn = undefined;
        slide = 1;
      }
    };
  });

  function locate(e: MouseEvent): { dial: number | null; region: HomeRegion | null } {
    if (!top) return { dial: null, region: null };
    const r = canvas.getBoundingClientRect();
    const x = (e.clientX - r.left) / dot,
      y = (e.clientY - r.top) / dot;
    const regions = homeRegions(top.w, top.h);
    for (const [k, b] of Object.entries(regions))
      if (x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h) return { dial: null, region: k as HomeRegion };
    return { dial: hitTest(dialGeometry(top.w, top.h, view.items.length, selected), x, y), region: null };
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
    // 項目クリックで選択と同時に起動
    selected = l.dial;
    hover = null;
    onactivate(l.dial);
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
  <canvas bind:this={under} aria-hidden="true" class="layer"></canvas>
  <canvas
    bind:this={canvas}
    aria-hidden="true"
    class="layer"
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
  .layer {
    position: absolute;
    left: 0;
    top: 0;
    display: block;
    image-rendering: pixelated;
  }
  canvas.pointer {
    cursor: pointer;
  }
</style>
