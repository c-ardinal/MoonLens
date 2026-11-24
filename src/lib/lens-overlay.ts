/**
 * Pure helpers for the frozen-frame selector's in-place translation
 * overlay. Kept free of DOM / Tauri so they can be unit tested.
 */

import type { LensBlock } from './ipc';

/** Rect in image pixels (e.g. the part of an image shown on screen). */
export interface FrameRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface CssRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Normalised rect from two drag points (CSS px). */
export function dragRect(x0: number, y0: number, x1: number, y1: number): CssRect {
  return {
    left: Math.min(x0, x1),
    top: Math.min(y0, y1),
    width: Math.abs(x1 - x0),
    height: Math.abs(y1 - y0)
  };
}

/**
 * Convert a CSS-px selection to frame pixels. `scale` is frame px per
 * CSS px (frame width / viewport width), which also covers HiDPI.
 */
export function toFrameRect(r: CssRect, scale: number): FrameRect {
  const x = Math.max(0, Math.round(r.left * scale));
  const y = Math.max(0, Math.round(r.top * scale));
  return {
    x,
    y,
    w: Math.max(0, Math.round((r.left + r.width) * scale) - x),
    h: Math.max(0, Math.round((r.top + r.height) * scale) - y)
  };
}

/** Inverse of {@link toFrameRect}. */
export function toCssRect(r: FrameRect, scale: number): CssRect {
  return {
    left: r.x / scale,
    top: r.y / scale,
    width: r.w / scale,
    height: r.h / scale
  };
}

/**
 * Position of a block (bbox relative to the cropped selection) on screen
 * in CSS px. The box is padded slightly so the mask fully covers
 * anti-aliased glyph edges of the original text.
 */
export function blockCssRect(block: LensBlock, crop: FrameRect, scale: number, pad = 2): CssRect {
  const [bx, by, bw, bh] = block.bbox;
  return {
    left: (crop.x + bx) / scale - pad,
    top: (crop.y + by) / scale - pad,
    width: bw / scale + pad * 2,
    height: bh / scale + pad * 2
  };
}

/**
 * Distribute a partially streamed translation of `"p1\n\np2\n\n…"` over
 * `n` blocks. Paragraphs that have not arrived yet stay null. With a
 * single block everything streamed so far belongs to it.
 */
export function splitStreaming(text: string, n: number): (string | null)[] {
  const out: (string | null)[] = new Array(n).fill(null);
  if (n === 0) return out;
  const trimmed = text.replace(/^\s+/, '');
  if (!trimmed) return out;
  if (n === 1) {
    out[0] = trimmed.trim();
    return out;
  }
  const parts = trimmed.split(/\n[ \t]*\n/);
  for (let i = 0; i < Math.min(n, parts.length); i++) {
    const p = parts[i].trim();
    if (p) out[i] = p;
  }
  // More paragraphs than blocks means the translator split a paragraph;
  // fold the overflow into the last block rather than dropping text.
  if (parts.length > n) {
    out[n - 1] = parts
      .slice(n - 1)
      .map((p) => p.trim())
      .filter(Boolean)
      .join(' ');
  }
  return out;
}

/**
 * Room (image px) a single-row block may take to its right when its
 * translation is longer than the source: up to its own width again
 * (at least eight line heights),
 * stopping short of the next block on the same row and of `limitRight`
 * (the right edge of the captured area, in the blocks' coordinates).
 * Multi-row blocks keep their width so paragraphs keep their margins.
 */
export function growRoom(blocks: LensBlock[], i: number, limitRight: number): number {
  const b = blocks[i];
  const [x, y, w, h] = b.bbox;
  if ((b.lines?.length ?? 1) > 1 || w <= 0) return 0;
  let right = Math.min(limitRight, x + w + Math.max(w, h * 8));
  blocks.forEach((o, j) => {
    const [ox, oy, ow, oh] = o.bbox;
    if (j === i || ow <= 0) return;
    if (ox >= x + w - 1 && oy < y + h && oy + oh > y) right = Math.min(right, ox - h * 0.4);
  });
  return Math.max(0, right - (x + w));
}

export const rgb = (c: [number, number, number] | null | undefined, fallback: string) =>
  c ? `rgb(${c[0]}, ${c[1]}, ${c[2]})` : fallback;

/**
 * Where to put the action bar: below the selection if there is room,
 * otherwise above it, otherwise inside its bottom edge. Horizontally it
 * is clamped to the viewport.
 */
export function toolbarPosition(
  sel: CssRect,
  bar: { width: number; height: number },
  viewport: { width: number; height: number },
  gap = 10
): { left: number; top: number } {
  let top = sel.top + sel.height + gap;
  if (top + bar.height > viewport.height - gap) {
    top = sel.top - bar.height - gap;
    if (top < gap) top = Math.max(gap, sel.top + sel.height - bar.height - gap);
  }
  let left = sel.left + sel.width / 2 - bar.width / 2;
  left = Math.min(Math.max(gap, left), Math.max(gap, viewport.width - bar.width - gap));
  return { left, top };
}

/** Text for "copy all": translations (or sources) in reading order. */
export function joinedText(blocks: LensBlock[], which: 'translation' | 'source'): string {
  return blocks
    .map((b) => (which === 'translation' ? (b.translation ?? b.source) : b.source))
    .filter(Boolean)
    .join('\n\n');
}

/**
 * Apply a partially streamed translation to the blocks that were sent
 * to the translator. `streaming` lists the blocks of the current
 * request in order (a long capture is translated in parts). Without it,
 * blocks already in the target language or served from the translation
 * cache were left out of the request, so paragraphs map onto the
 * remaining blocks in order.
 */
export function applyStreaming(
  blocks: LensBlock[],
  text: string,
  streaming?: number[]
): LensBlock[] {
  const pending =
    streaming ??
    blocks.map((b, i) => (b.same_language || b.from_cache ? -1 : i)).filter((i) => i >= 0);
  const parts = splitStreaming(text, pending.length);
  const out = blocks.slice();
  pending.forEach((bi, k) => {
    if (parts[k] != null) out[bi] = { ...out[bi], translation: parts[k] };
  });
  return out;
}

/** Target languages offered in the selector's toolbar. */
export const LANGUAGES: { code: string; label: string }[] = [
  { code: 'ja', label: '日本語' },
  { code: 'en', label: '英語' },
  { code: 'zh-Hans', label: '中国語（簡体）' },
  { code: 'zh-Hant', label: '中国語（繁体）' },
  { code: 'ko', label: '韓国語' },
  { code: 'fr', label: 'フランス語' },
  { code: 'de', label: 'ドイツ語' },
  { code: 'es', label: 'スペイン語' },
  { code: 'pt', label: 'ポルトガル語' },
  { code: 'it', label: 'イタリア語' },
  { code: 'ru', label: 'ロシア語' },
  { code: 'vi', label: 'ベトナム語' },
  { code: 'th', label: 'タイ語' },
  { code: 'id', label: 'インドネシア語' }
];

/** Display name for a language tag, falling back to the tag itself. */
export function languageLabel(code: string | null | undefined): string {
  if (!code) return '';
  const exact = LANGUAGES.find((l) => l.code.toLowerCase() === code.toLowerCase());
  if (exact) return exact.label;
  const primary = code.split(/[-_]/)[0].toLowerCase();
  if (primary === 'zh') return '中国語';
  return LANGUAGES.find((l) => l.code === primary)?.label ?? code;
}

/** True when every block was already in the target language. */
export function allSameLanguage(blocks: LensBlock[]): boolean {
  return blocks.length > 0 && blocks.every((b) => b.same_language);
}
