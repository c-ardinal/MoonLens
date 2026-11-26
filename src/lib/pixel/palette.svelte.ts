/**
 * ドット絵外観の配色。外観設定（テーマ・アクセントカラー）から 10 色を算出し、
 * CSS 変数（--ml-px-*）と Canvas 用パレット（PAL）へ同時に反映。
 * 明度の段階はテーマ単位で固定、色相・彩度のみアクセントカラーへ追従
 * （文字と背景の明度差を維持）。
 */
import { PAL } from './pix';

export type Scheme = 'dark' | 'light';

/** 既定のアクセントカラー（琥珀）。 */
export const DEFAULT_ACCENT = '#ffb547';

/** 基準配色（琥珀）。番号は PAL と共通：0 地、1 縞、2 枠、3 減光、4 中間、5 主色、6 文字、7 異常、8 正常、9 強調。 */
const BASE: Record<Scheme, readonly string[]> = {
  dark: ['#0b0805', '#1c1207', '#3a2810', '#8a5f24', '#c98a2e', '#ffb547', '#ffe0a6', '#ff6a4a', '#d6ef8a', '#fbfbf8'],
  light: ['#fbf4e4', '#f1e4c8', '#dcc9a0', '#a8875a', '#7d6338', '#a85a00', '#3a2810', '#c2321a', '#4f7a12', '#1c1207']
};
/** アクセントカラーへ追従する段（0〜6）。7〜9 は固定。 */
const TINTED = 7;
const NAMES = ['black', 'brown', 'dim', 'faint', 'mid', 'amber', 'cream', 'red', 'green', 'bone'] as const;
/** 彩度倍率の上限（低彩度の段の過剰な発色を防止）。 */
const MAX_CHROMA_RATIO = 1.6;

type Lch = { l: number; c: number; h: number };

const HEX = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;

function parseHex(hex: string): [number, number, number] {
  let s = hex.trim().slice(1);
  if (s.length === 3) s = [...s].map((ch) => ch + ch).join('');
  return [0, 2, 4].map((i) => parseInt(s.slice(i, i + 2), 16) / 255) as [number, number, number];
}
const toLinear = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const toGamma = (v: number) => (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055);

export function hexToOklch(hex: string): Lch {
  const [r, g, b] = parseHex(hex).map(toLinear);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return { l: L, c: Math.hypot(A, B), h: Math.atan2(B, A) };
}

function oklchToLinear({ l: L, c, h }: Lch): [number, number, number] {
  const A = c * Math.cos(h),
    B = c * Math.sin(h);
  const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3;
  const m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3;
  const s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s
  ];
}

/** sRGB の色域外は彩度のみ低減（明度・色相を維持）。 */
export function oklchToHex(lch: Lch): string {
  const inGamut = (rgb: number[]) => rgb.every((v) => v >= -1e-4 && v <= 1 + 1e-4);
  let rgb = oklchToLinear(lch);
  if (!inGamut(rgb)) {
    let lo = 0,
      hi = lch.c;
    for (let i = 0; i < 20; i++) {
      const mid = (lo + hi) / 2;
      if (inGamut(oklchToLinear({ ...lch, c: mid }))) lo = mid;
      else hi = mid;
    }
    rgb = oklchToLinear({ ...lch, c: lo });
  }
  return (
    '#' +
    rgb
      .map((v) =>
        Math.round(toGamma(Math.min(1, Math.max(0, v))) * 255)
          .toString(16)
          .padStart(2, '0')
      )
      .join('')
  );
}

/** 設定値からアクセントカラーを確定（不正値・未設定は既定色）。 */
export function normalizeAccent(accent: unknown): string {
  return typeof accent === 'string' && HEX.test(accent.trim()) ? accent.trim().toLowerCase() : DEFAULT_ACCENT;
}

/** テーマとアクセントカラーから 10 色を算出。既定色は基準配色と同一。 */
export function buildPalette(scheme: Scheme, accent?: unknown): string[] {
  const base = BASE[scheme];
  const want = normalizeAccent(accent);
  const [r, g, b] = parseHex(want);
  const [dr, dg, db] = parseHex(DEFAULT_ACCENT);
  if (r === dr && g === dg && b === db) return [...base];
  const ref = hexToOklch(DEFAULT_ACCENT);
  const to = hexToOklch(want);
  const ratio = Math.min(MAX_CHROMA_RATIO, to.c / ref.c);
  const turn = to.h - ref.h;
  return base.map((hex, i) => {
    if (i >= TINTED) return hex;
    const from = hexToOklch(hex);
    return oklchToHex({ l: from.l, c: from.c * ratio, h: from.h + turn });
  });
}

const state = $state({ version: 0, scheme: 'dark' as Scheme });

/** 現在の配色。version は配色変更の通知用（Canvas 描画の $effect から参照）。 */
export const palette = {
  get version() {
    return state.version;
  },
  get scheme() {
    return state.scheme;
  }
};

/** 配色を CSS 変数と Canvas 用パレットへ反映。同一配色の再適用は通知なし。 */
export function applyPalette(scheme: Scheme, accent: unknown, root: HTMLElement = document.documentElement) {
  const next = buildPalette(scheme, accent);
  root.dataset.scheme = scheme;
  next.forEach((hex, i) => root.style.setProperty(`--ml-px-${NAMES[i]}`, hex));
  if (next.every((hex, i) => PAL[i] === hex) && state.scheme === scheme) return;
  PAL.splice(0, PAL.length, ...next);
  state.scheme = scheme;
  state.version++;
}
