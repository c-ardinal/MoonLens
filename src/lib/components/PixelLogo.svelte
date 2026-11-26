<script lang="ts">
  import { onMount } from 'svelte';
  import { Pix, TITLE_FONT_FAMILY } from '$lib/pixel/pix';
  import { palette } from '$lib/pixel/palette.svelte';

  /**
   * ロゴ：太字斜体の 2 値化文字＋段影（見出し帯と同系の書体処理）。背景の帯なし。
   * 2 倍ドット、端末画素の整数倍で描画。
   */
  const H = 32;
  const PAD = 8;
  const FONT = `700 23px ${TITLE_FONT_FAMILY}`;
  let canvas: HTMLCanvasElement;

  function draw() {
    const dpr = window.devicePixelRatio || 1;
    const dot = Math.max(1, Math.floor(2 * dpr + 1e-6)) / dpr;
    const probe = new Pix(document.createElement('canvas'), 1, 1);
    const tw = probe.textW('MOONLENS', FONT);
    const w = tw + PAD * 2;
    const p = new Pix(canvas, w, H);
    canvas.style.width = `${w * dot}px`;
    canvas.style.height = `${H * dot}px`;
    p.clear();
    // 字面を上下左右の中央へ配置
    const { x, y } = p.centerInSlab('MOONLENS', { x: 0, y: 0, w, h: H, k: 0 }, { k: 0.3, font: FONT });
    p.text('MOONLENS', x + 1, y + 1, 3, { k: 0.3, font: FONT }); // 段影
    p.text('MOONLENS', x, y, 5, { k: 0.3, font: FONT });
  }

  let mounted = $state(false);
  // 配色変更時に再描画
  $effect(() => {
    void palette.version;
    if (mounted) draw();
  });

  onMount(() => {
    mounted = true;
    document.fonts
      ?.load(FONT, 'MOONLENS')
      .then(draw)
      .catch(() => {});
  });
</script>

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
  canvas {
    display: block;
    image-rendering: pixelated;
  }
</style>
