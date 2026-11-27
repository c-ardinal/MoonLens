<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { Pix } from '$lib/pixel/pix';
  import { ROTORS, dialGeometry, drawMoon, drawRotor, hitTest, moonSize, rotorSize } from '$lib/pixel/dial';
  import {
    drawHome,
    drawHomeMotion,
    homeDot,
    homeRegions,
    titleOffsets,
    type HomeRegion,
    type HomeView
  } from '$lib/pixel/home';
  import { motion } from '$lib/pixel/motion.svelte';
  import { palette } from '$lib/pixel/palette.svelte';
  import { textScale } from '$lib/pixel/text-scale.svelte';

  /**
   * ホーム画面の描画層。Canvas を重ねる構成。
   * - 下層（動作層）：光点・波紋・中心核の円。12 fps で再描画（文字処理なし）。
   * - 中層（中心の月）：寸法変化時のみ描画。回転体の背面、静止層は月の範囲を透過。
   * - 中層（回転体）：ルーン環・中心核の正方形。寸法変化時のみ描画し、CSS で等速回転（ドット単位の段差なし）。
   * - 上層（静止層）：文字・枠・ノード。状態変化時と選択切替の演出中のみ再描画。
   * 推論中・動き削減設定時・非表示時は演出を停止し、静止画のみ描画。
   * 選択切替：クリック・キー操作に加え、ホイールと魔法陣のドラッグ（回転）にも対応。
   */
  let {
    view,
    selected = $bindable(0),
    onactivate,
    onregion
  }: {
    view: Omit<HomeView, 'hover' | 'hot' | 'selected' | 'turn' | 'slide' | 'leave'>;
    selected?: number;
    onactivate: (index: number) => void;
    onregion: (region: HomeRegion) => void;
  } = $props();

  const FRAME_MS = 1000 / 12;
  const TURN_MS = 320;

  let under: HTMLCanvasElement;
  let canvas: HTMLCanvasElement;
  let moonEl: HTMLCanvasElement;
  const rotorEls: HTMLCanvasElement[] = [];
  let host: HTMLDivElement;
  let top: Pix | null = null;
  let low: Pix | null = null;
  let dot = $state(2);
  /** 静止層の内部解像度の倍率。文字の拡縮時は端末画素と等倍（輪郭書体を高精細で描画）。 */
  let ss = $state(1);
  let hover = $state<number | null>(null);
  let hot = $state<HomeRegion | null>(null);
  let size = $state({ w: 0, h: 0 });
  let fontsReady = $state(false);
  let ro: ResizeObserver | null = null;

  /** 選択切替の演出：表示位置 turn を from から selected へ補間。 */
  let anim = $state<{ from: number; to: number; start: number } | null>(null);
  let turn = $state<number | undefined>(undefined);
  let slide = $state(1);
  /** 退場側の機能名（切替前の項目）。演出の終了時に解除。 */
  let leave = $state<HomeView['leave']>(undefined);
  let shown = selected;
  let clock = 0;

  function measure() {
    const r = host.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    dot = homeDot(r.width, r.height, dpr);
    ss = textScale.zoom === 1 ? 1 : Math.max(1, Math.round(dot * dpr));
    size = { w: Math.floor(r.width / dot), h: Math.floor(r.height / dot) };
  }

  function ensure() {
    const { w, h } = size;
    if (!top || top.w !== w || top.h !== h || top.ss !== ss || low?.w !== w) {
      top = new Pix(canvas, w, h, ss);
      low = new Pix(under, w, h);
      for (const c of [canvas, under]) {
        c.style.width = `${w * dot}px`;
        c.style.height = `${h * dot}px`;
      }
    }
    top.textScale = textScale.zoom;
  }
  // 文字サイズ設定の変更：静止層の内部解像度を再計算
  $effect(() => {
    void textScale.zoom;
    if (host) untrack(measure);
  });
  const ok = () => canvas && under && fontsReady && size.w >= 200 && size.h >= 200;

  // 選択の変化を検知し、最短方向の回転演出を開始
  $effect(() => {
    const to = selected;
    if (to === shown) return;
    const n = view.items.length;
    let d = (((to - shown) % n) + n) % n;
    if (d > n / 2) d -= n;
    const from = to - d;
    const prev = shown;
    shown = to;
    if (!motion.on) return;
    // 演出途中の再切替：入場途中の機能名を現在位置から退場へ転用
    const running = untrack(() => (anim ? titleOffsets(slide, anim.to < anim.from ? -1 : 1).enter : 0));
    leave = { index: prev, from: running };
    turn = from;
    slide = 0;
    anim = { from, to, start: performance.now() };
  });

  // 静止層
  $effect(() => {
    const state = { ...view, selected, hover, hot, turn, slide, leave };
    void size;
    void ss;
    void textScale.zoom;
    void palette.version; // 配色変更時に再描画
    if (!ok()) return;
    ensure();
    drawHome(top!, state);
    if (!motion.on || anim) drawHomeMotion(low!, { items: view.items, selected, turn }, clock);
  });

  // 回転体：寸法の変化時のみ描画・配置。回転自体は CSS（等速・線形）
  const ring = $derived(size.w >= 200 && size.h >= 200 ? dialGeometry(size.w, size.h, view.items.length, 0) : null);
  $effect(() => {
    void palette.version;
    if (!ring || !fontsReady) return;
    const { cx, cy, r } = ring;
    /** 魔法陣の中心へ n 四方の Canvas を配置。 */
    const place = (el: HTMLCanvasElement, n: number) => {
      const half = (n - 1) / 2;
      Object.assign(el.style, {
        left: `${(cx - half) * dot}px`,
        top: `${(cy - half) * dot}px`,
        width: `${n * dot}px`,
        height: `${n * dot}px`
      });
      return new Pix(el, n, n);
    };
    if (moonEl) drawMoon(place(moonEl, moonSize(r)), r);
    ROTORS.forEach((o, i) => {
      const el = rotorEls[i];
      if (el) drawRotor(place(el, rotorSize(o.id, r)), o.id, r);
    });
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
        // 機能名の縦送りは項目の切替時のみ（同一項目への復帰は回転のみ）
        slide = leave ? u : 1;
        if (u >= 1) {
          anim = null;
          leave = undefined;
        }
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
        leave = undefined;
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
    // ドラッグ回転の終了直後のクリックは無視
    if (dragged) return void (dragged = false);
    const l = locate(e);
    if (l.region) return onregion(l.region);
    if (l.dial === null) return;
    // 項目クリックで選択と同時に起動
    selected = l.dial;
    hover = null;
    onactivate(l.dial);
  }

  /** 選択を d 個移動（ホイール）。 */
  function step(d: number) {
    const n = view.items.length;
    selected = (((selected + d) % n) + n) % n;
  }

  // ホイール：1 ノッチ（累積 60）で 1 項目、連続入力は 140 ms 間隔
  let wheelAcc = 0;
  let wheelAt = 0;
  function onWheel(e: WheelEvent) {
    e.preventDefault();
    wheelAcc += Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
    const now = performance.now();
    if (Math.abs(wheelAcc) < 60 || now - wheelAt < 140) return;
    wheelAt = now;
    step(Math.sign(wheelAcc));
    wheelAcc = 0;
  }

  // ドラッグ：魔法陣の中心まわりの回転角へ追従し、離した位置の最寄り項目を選択
  let drag = $state<{ base: number; last: number; total: number } | null>(null);
  let dragged = $state(false);
  let dragRaf = 0;
  const DRAG_START_DEG = 6;
  function pointerAngle(e: PointerEvent): number | null {
    if (!top) return null;
    const r = canvas.getBoundingClientRect();
    const x = (e.clientX - r.left) / dot,
      y = (e.clientY - r.top) / dot;
    const g = dialGeometry(top.w, top.h, view.items.length, selected);
    const d = Math.hypot(x - g.cx, y - g.cy);
    if (d < 20 || d > g.r + 24) return null;
    return (Math.atan2(y - g.cy, x - g.cx) * 180) / Math.PI;
  }
  function onDown(e: PointerEvent) {
    if (e.button !== 0 || locate(e).region) return;
    const a = pointerAngle(e);
    if (a === null) return;
    drag = { base: anim ? anim.to : selected, last: a, total: 0 };
    dragged = false;
  }
  function onDragMove(e: PointerEvent) {
    if (!drag) return;
    const a = pointerAngle(e) ?? drag.last;
    let d = a - drag.last;
    if (d > 180) d -= 360;
    if (d < -180) d += 360;
    drag.last = a;
    drag.total += d;
    if (!dragged && Math.abs(drag.total) < DRAG_START_DEG) return;
    if (!dragged) {
      dragged = true;
      anim = null;
      leave = undefined;
      canvas.setPointerCapture(e.pointerId);
    }
    const t = drag.base - drag.total / (360 / view.items.length);
    if (!dragRaf)
      dragRaf = requestAnimationFrame(() => {
        dragRaf = 0;
        turn = t;
        slide = 1;
      });
  }
  function onUp(e: PointerEvent) {
    if (!drag) return;
    const was = drag;
    drag = null;
    if (!dragged) return;
    cancelAnimationFrame(dragRaf);
    dragRaf = 0;
    if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
    const n = view.items.length;
    const from = was.base - was.total / (360 / n);
    const to = Math.round(from);
    const idx = ((to % n) + n) % n;
    const prev = shown;
    shown = idx;
    selected = idx;
    if (motion.on && Math.abs(to - from) > 0.01) {
      turn = from;
      leave = idx === prev ? undefined : { index: prev, from: 0 };
      slide = leave ? 0 : 1;
      anim = { from, to, start: performance.now() };
    } else {
      turn = undefined;
    }
  }

  onMount(() => {
    measure();
    ro = new ResizeObserver(measure);
    ro.observe(host);
    Promise.all([
      document.fonts?.load('16px DotGothic16'),
      // i18n-ignore（見出し書体の先行読込に使用する字種）
      document.fonts?.load("700 44px 'BIZ UDGothic','Yu Gothic UI','Hiragino Sans','Noto Sans JP',sans-serif", 'ウィンドウ画像テキスト音声翻訳')
    ])
      .catch(() => {})
      .finally(() => (fontsReady = true));
  });
  onDestroy(() => ro?.disconnect());
</script>

<div class="home-canvas" bind:this={host}>
  <!-- 視覚表現。操作はキーボード（←→・Enter・数字）と支援技術向けボタン群でも可能。 -->
  <canvas bind:this={under} aria-hidden="true" class="layer"></canvas>
  <canvas bind:this={moonEl} aria-hidden="true" class="layer fixed"></canvas>
  {#each ROTORS as o, i (o.id)}
    <canvas
      bind:this={rotorEls[i]}
      aria-hidden="true"
      class="layer rotor"
      class:paused={!motion.on}
      style="--a0: {o.start}deg; animation-duration: {360 / Math.abs(o.degPerSec)}s; animation-direction: {o.degPerSec < 0 ? 'reverse' : 'normal'}"
    ></canvas>
  {/each}
  <canvas
    bind:this={canvas}
    aria-hidden="true"
    class="layer"
    class:pointer={hover !== null || hot !== null}
    class:grabbing={dragged && drag !== null}
    onpointermove={(e) => (onMove(e), onDragMove(e))}
    onpointerdown={onDown}
    onpointerup={onUp}
    onpointercancel={onUp}
    onpointerleave={() => ((hover = null), (hot = null))}
    onwheel={onWheel}
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
  .fixed {
    pointer-events: none;
  }
  canvas.pointer {
    cursor: pointer;
  }
  canvas.grabbing {
    cursor: grabbing;
  }
  /* 回転体：GPU 合成の等速回転（再描画なし）。推論中・動き削減時は停止 */
  .rotor {
    pointer-events: none;
    transform-origin: 50% 50%;
    animation-name: spin;
    animation-timing-function: linear;
    animation-iteration-count: infinite;
  }
  .rotor.paused {
    animation-play-state: paused;
  }
  @keyframes spin {
    from {
      transform: rotate(var(--a0));
    }
    to {
      transform: rotate(calc(var(--a0) + 360deg));
    }
  }
</style>
