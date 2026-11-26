/**
 * ドット絵の描画基盤。論理解像度の Canvas へ 1 画素単位で描画し、
 * 表示側は CSS の `image-rendering: pixelated` で整数倍に拡大する。
 * 文字は DotGothic16 を 16px で描いて 2 値化し、補間なしで打点する。
 */

/** 限定パレット（番号で参照）。外観設定の変更時は palette.svelte.ts が全色を差替え。 */
export const PAL: string[] = [
  '#0b0805', // 0 黒
  '#1c1207', // 1 焦茶
  '#3a2810', // 2 暗琥珀
  '#8a5f24', // 3 減光
  '#c98a2e', // 4 中琥珀
  '#ffb547', // 5 琥珀
  '#ffe0a6', // 6 淡琥珀
  '#ff6a4a', // 7 異常
  '#d6ef8a', // 8 正常
  '#fbfbf8' // 9 骨白
];

export type Color = number | string;
type Bitmap = readonly string[];

const BAYER = [
  [0, 8, 2, 10],
  [12, 4, 14, 6],
  [3, 11, 1, 9],
  [15, 7, 13, 5]
];

export const PIXEL_FONT = '16px DotGothic16';
/** 大型見出し用：高精細な太字を 1 ドット単位で 2 値化。 */
export const TITLE_FONT_FAMILY = "'BIZ UDGothic','Yu Gothic UI','Hiragino Sans','Noto Sans JP',sans-serif";

/** 2 値化の閾値（アルファ値）。 */
const INK_THRESHOLD = 115;

/** 斜めの帯：左下基準の x、上端 y、幅、高さ、傾き（上の行ほど右へ k 倍）。 */
export interface Slab {
  x: number;
  y: number;
  w: number;
  h: number;
  k: number;
}
/** 三角（Pix.tri）の寸法。 */
export const TRI_W = 4;
export const TRI_H = 8;

/** 字面の打点範囲（両端含む、描画原点基準）。 */
export interface InkBox {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** 2 値化後の打点範囲。斜体は text() と同じ行単位のずれを加算。打点なしは null。 */
export function inkBounds(d: ArrayLike<number>, w: number, h: number, k = 0): InkBox | null {
  let x0 = Infinity,
    y0 = Infinity,
    x1 = -Infinity,
    y1 = -Infinity;
  for (let j = 0; j < h; j++) {
    const o = Math.floor((h - 1 - j) * k);
    for (let i = 0; i < w; i++) {
      if (d[(j * w + i) * 4 + 3] <= INK_THRESHOLD) continue;
      x0 = Math.min(x0, i + o);
      x1 = Math.max(x1, i + o);
      y0 = Math.min(y0, j);
      y1 = Math.max(y1, j);
    }
  }
  return y1 < 0 ? null : { x0, y0, x1, y1 };
}

/** 字面の中心を帯（平行四辺形）の中心へ一致させる描画原点。 */
export function slabCenterOrigin(ink: InkBox, slab: Slab): { x: number; y: number } {
  const cx = slab.x + ((slab.h - 1) * slab.k) / 2 + slab.w / 2;
  const cy = slab.y + slab.h / 2;
  return {
    x: Math.round(cx - (ink.x0 + ink.x1 + 1) / 2),
    y: Math.round(cy - (ink.y0 + ink.y1 + 1) / 2)
  };
}

export class Pix {
  readonly g: CanvasRenderingContext2D;
  private scratch: HTMLCanvasElement | null = null;

  /** ss：内部解像度の倍率（整数）。描画座標は論理画素のまま、1 画素を ss×ss で打点。 */
  constructor(
    readonly canvas: HTMLCanvasElement,
    readonly w: number,
    readonly h: number,
    readonly ss = 1
  ) {
    canvas.width = w * ss;
    canvas.height = h * ss;
    const g = canvas.getContext('2d');
    if (!g) throw new Error('2d context unavailable');
    g.imageSmoothingEnabled = false;
    g.scale(ss, ss);
    this.g = g;
  }

  col(c: Color): string {
    return typeof c === 'number' ? PAL[c] : c;
  }
  clear() {
    this.g.clearRect(0, 0, this.w, this.h);
  }
  px(x: number, y: number, c: Color) {
    this.g.fillStyle = this.col(c);
    this.g.fillRect(x | 0, y | 0, 1, 1);
  }
  rect(x: number, y: number, w: number, h: number, c: Color) {
    this.g.fillStyle = this.col(c);
    this.g.fillRect(x | 0, y | 0, w | 0, h | 0);
  }
  hl(x: number, y: number, w: number, c: Color) {
    this.rect(x, y, w, 1, c);
  }
  vl(x: number, y: number, h: number, c: Color) {
    this.rect(x, y, 1, h, c);
  }

  /** Bresenham の直線。dash = [描画数, 空白数]。 */
  line(x0: number, y0: number, x1: number, y1: number, c: Color, dash?: [number, number]) {
    x0 = Math.round(x0);
    y0 = Math.round(y0);
    x1 = Math.round(x1);
    y1 = Math.round(y1);
    const dx = Math.abs(x1 - x0),
      dy = -Math.abs(y1 - y0),
      sx = x0 < x1 ? 1 : -1,
      sy = y0 < y1 ? 1 : -1;
    let e = dx + dy,
      k = 0;
    for (;;) {
      if (!dash || k % (dash[0] + dash[1]) < dash[0]) this.px(x0, y0, c);
      k++;
      if (x0 === x1 && y0 === y1) break;
      const e2 = 2 * e;
      if (e2 >= dy) {
        e += dy;
        x0 += sx;
      }
      if (e2 <= dx) {
        e += dx;
        y0 += sy;
      }
    }
  }

  /** 中点円。 */
  circle(cx: number, cy: number, r: number, c: Color, dash?: [number, number]) {
    let x = r,
      y = 0,
      err = 1 - r;
    const pts: [number, number][] = [];
    while (x >= y) {
      pts.push([x, y], [y, x], [-y, x], [-x, y], [-x, -y], [-y, -x], [y, -x], [x, -y]);
      y++;
      if (err < 0) err += 2 * y + 1;
      else {
        x--;
        err += 2 * (y - x) + 1;
      }
    }
    pts.sort((a, b) => Math.atan2(a[1], a[0]) - Math.atan2(b[1], b[0]));
    pts.forEach(([dx, dy], k) => {
      if (!dash || k % (dash[0] + dash[1]) < dash[0]) this.px(cx + dx, cy + dy, c);
    });
  }
  fcircle(cx: number, cy: number, r: number, c: Color) {
    for (let y = -r; y <= r; y++) {
      const w = Math.floor(Math.sqrt(r * r - y * y));
      this.hl(cx - w, cy + y, 2 * w + 1, c);
    }
  }

  /** 菱形（45 度の正方形）。 */
  diamond(cx: number, cy: number, r: number, c: Color, fill?: Color | null, dash?: [number, number]) {
    if (fill !== undefined && fill !== null)
      for (let y = -r; y <= r; y++) {
        const w = r - Math.abs(y);
        this.hl(cx - w, cy + y, 2 * w + 1, fill);
      }
    let k = 0;
    for (let i = 0; i <= r; i++) {
      const on = !dash || k++ % (dash[0] + dash[1]) < dash[0];
      if (!on) continue;
      this.px(cx + i, cy - r + i, c);
      this.px(cx + r - i, cy + i, c);
      this.px(cx - i, cy + r - i, c);
      this.px(cx - r + i, cy - i, c);
    }
  }

  /**
   * 右向きの三角（幅 TRI_W・高さ TRI_H の階段形）。
   * 字形の「▶」は字面の中心が文字より下方へ偏るため、図形で描画。
   */
  tri(x: number, y: number, c: Color) {
    for (let j = 0; j < TRI_H; j++) this.hl(x, y + j, Math.min(j, TRI_H - 1 - j) + 1, c);
  }

  /** 字面の垂直中心（text() の y からの距離）。図形を文字の中央へ揃える基準。 */
  inkMidY(s: string, font = PIXEL_FONT): number {
    const r = this.raster(s, font);
    const ink = r && inkBounds(r.d, r.w, r.h);
    return ink ? (ink.y0 + ink.y1 + 1) / 2 : TRI_H;
  }

  /** 2 色の網点塗り。level は b 色の比率（0〜1）。 */
  dither(x: number, y: number, w: number, h: number, a: Color, b: Color, level: number) {
    for (let j = 0; j < h; j++)
      for (let i = 0; i < w; i++) this.px(x + i, y + j, BAYER[(y + j) & 3][(x + i) & 3] / 16 < level ? b : a);
  }

  /** 文字列ビットマップ（'#' = 着色）。 */
  icon(rows: Bitmap, x: number, y: number, c: Color, scale = 1) {
    rows.forEach((r, j) =>
      [...r].forEach((ch, i) => {
        if (ch === '#') this.rect(x + i * scale, y + j * scale, scale, scale, c);
      })
    );
  }

  textW(s: string, font = PIXEL_FONT): number {
    this.g.font = font;
    return Math.ceil(this.g.measureText(s).width);
  }

  /** 文字を原寸で描画し 2 値化前の画素列を取得。 */
  private raster(s: string, font: string): { d: Uint8ClampedArray; w: number; h: number; tw: number } | null {
    const size = Number(/(\d+)px/.exec(font)?.[1] ?? 16);
    const tw = this.textW(s, font),
      th = Math.ceil(size * 1.15);
    const t = (this.scratch ??= document.createElement('canvas'));
    t.width = tw + 2;
    t.height = th;
    const tg = t.getContext('2d', { willReadFrequently: true });
    if (!tg) return null;
    tg.clearRect(0, 0, t.width, t.height);
    tg.font = font;
    tg.textBaseline = 'top';
    tg.fillStyle = '#fff';
    tg.fillText(s, 0, 1);
    return { d: tg.getImageData(0, 0, t.width, t.height).data, w: t.width, h: t.height, tw };
  }

  /**
   * 2 値化した文字。k > 0 で斜体（上の行ほど右へ移動）。
   * 斜体は原寸ビットマップの行単位でずらしてから拡大し、字形の崩れを回避。
   */
  text(
    s: string,
    x: number,
    y: number,
    c: Color,
    opt: { scale?: number; align?: 'left' | 'center' | 'right'; k?: number; font?: string } = {}
  ): number {
    const scale = opt.scale ?? 1,
      k = opt.k ?? 0;
    const r = this.raster(s, opt.font ?? PIXEL_FONT);
    if (!r) return 0;
    const { d, tw } = r;
    if (opt.align === 'center') x -= (tw * scale) / 2;
    else if (opt.align === 'right') x -= tw * scale;
    x = Math.round(x);
    y = Math.round(y);
    this.g.fillStyle = this.col(c);
    for (let j = 0; j < r.h; j++) {
      const o = Math.floor((r.h - 1 - j) * k) * scale;
      for (let i = 0; i < r.w; i++)
        if (d[(j * r.w + i) * 4 + 3] > INK_THRESHOLD) this.g.fillRect(x + i * scale + o, y + j * scale, scale, scale);
    }
    return tw * scale;
  }

  /**
   * 斜めの帯の中央へ字面を配置する描画原点（text() の x・y）。
   * 字体・環境で字面の位置が異なるため、2 値化後の打点範囲を実測して算出。
   */
  centerInSlab(s: string, slab: Slab, opt: { k?: number; font?: string } = {}): { x: number; y: number } {
    const r = this.raster(s, opt.font ?? PIXEL_FONT);
    const ink = r && inkBounds(r.d, r.w, r.h, opt.k ?? 0);
    return ink ? slabCenterOrigin(ink, slab) : { x: slab.x, y: slab.y };
  }

  /** 斜めの帯（平行四辺形）。上の行ほど右へ移動。 */
  slab(x: number, y: number, w: number, h: number, k: number, c: Color) {
    for (let j = 0; j < h; j++) this.hl(x + Math.floor((h - 1 - j) * k), y + j, w, c);
  }

  /** 斜めの枠線。 */
  slabFrame(x: number, y: number, w: number, h: number, k: number, c: Color) {
    for (let j = 0; j < h; j++) {
      const o = Math.floor((h - 1 - j) * k);
      if (j === 0 || j === h - 1) this.hl(x + o, y + j, w, c);
      else {
        this.px(x + o, y + j, c);
        this.px(x + o + w - 1, y + j, c);
      }
    }
  }

  /** 斜めの窓：段影・本体・外枠・内枠。 */
  slantWin(x: number, y: number, w: number, h: number, k: number, opt: { shadow?: Color; bg?: Color } = {}) {
    if (opt.shadow !== undefined) this.slab(x + 5, y + 5, w, h, k, opt.shadow);
    this.slab(x, y, w, h, k, opt.bg ?? 0);
    this.slabFrame(x, y, w, h, k, 5);
    this.slabFrame(x + 3, y + 2, w - 6, h - 4, k, 3);
  }

  /** 幅 w で折返す文字列。戻り値は次の行の y。maxLines 超過分は省略記号。 */
  wrap(s: string, x: number, y: number, w: number, c: Color, lh = 18, maxLines = Infinity): number {
    const lines: string[] = [];
    let line = '';
    for (const ch of s) {
      // 行頭禁則：句読点・長音・閉じ括弧は前行へ追込み
      const hang = /[、。，．ー・）」』！？]/.test(ch);
      if (ch === '\n' || (this.textW(line + ch) > w && !hang)) {
        lines.push(line);
        line = ch === '\n' ? '' : ch;
      } else line += ch;
    }
    if (line) lines.push(line);
    const shown = lines.slice(0, maxLines);
    if (lines.length > maxLines) shown[maxLines - 1] = this.fit(shown[maxLines - 1] + '…', w);
    shown.forEach((l, i) => this.text(l, x, y + i * lh, c));
    return y + shown.length * lh;
  }

  /** 幅 w に収まるよう末尾を省略。長文も計測回数は対数（二分探索）。 */
  fit(s: string, w: number): string {
    const chars = [...s];
    // 1 文字は最低 4 ドット幅：w / 4 文字超は確実に超過のため先に切詰め
    const cap = Math.min(chars.length, Math.ceil(w / 4) + 1);
    if (cap === chars.length && this.textW(s) <= w) return s;
    let lo = 1,
      hi = cap; // lo：収まる最長（最低 1 文字）、hi：探索上限
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (this.textW(chars.slice(0, mid).join('') + '…') <= w) lo = mid;
      else hi = mid - 1;
    }
    return chars.slice(0, lo).join('') + '…';
  }
}

/** 斜め縞の網点タイル。縞の周期とタイル寸法を一致させ、継ぎ目なしで敷詰め可能。 */
export function stripeTile(): HTMLCanvasElement {
  const period = 56,
    width = 22,
    level = 0.55,
    k = 0.6;
  const w = period,
    h = 280; // 280 * 0.6 = 168 = 周期の 3 倍
  const c = document.createElement('canvas');
  const p = new Pix(c, w, h);
  p.rect(0, 0, w, h, 0);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const v = (((x + y * k) % period) + period) % period;
      if (v < width && BAYER[y & 3][x & 3] / 16 < level) p.px(x, y, 1);
    }
  return c;
}
