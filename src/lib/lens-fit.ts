/**
 * ウィンドウ翻訳：字幕領域の自動整形（DOM 非依存）。
 *
 * 常時翻訳の読取結果（ブロックの位置と原文）を一定時間集計し、文字が反復して
 * 出現する帯（字幕領域）を算出。座標は観測範囲の左上を原点とする物理ピクセル。
 *
 * 判定の要点：
 * - 全標本で同一位置・同一内容のブロックは固定表示（HUD・ロゴ等）と判定し、除外。
 * - 残余（変動ブロック）を縦方向の近接で集団化し、標本 1 件当りの文字数が最大の集団を採用。
 *   評価は標本 1 件当りの文字数のため、秒単位で変化する時計等は字幕より優先されない。
 * - 採用集団へ隣接する固定表示（話者名の札等）は帯へ算入。
 * - 変動ブロックが皆無の場合は全ブロックの外接範囲（静的な文面への整形）。
 */

import type { LensBlock } from './ipc';

export type Box = [number, number, number, number];

export interface FitBlock {
  bbox: Box;
  text: string;
  lineHeight: number;
}

/** 1 回の読取結果。 */
export type Sample = FitBlock[];

/** 観測の最短時間：経過後、文面の変化回数が充足した時点で確定。 */
export const OBSERVE_MIN_MS = 15_000;
/** 観測の最長時間：変化回数に関係なく確定。 */
export const OBSERVE_MAX_MS = 45_000;
/** 最短時間での確定に必要な標本数。 */
export const SAMPLES_WANTED = 4;

/** 同一位置と判定する重複率（IoU）。 */
const SAME_PLACE = 0.8;
/** 同一内容と判定する類似度（文字バイグラムの Dice 係数）。OCR の揺動を許容。 */
const SAME_TEXT = 0.7;
/** 整形不要と判定する面積比（帯／観測範囲）。 */
const WORTH_FITTING = 0.9;

const normalize = (s: string) => s.replace(/\s+/g, '');

/** 読取結果を標本へ変換（文字の無いブロックは除外）。 */
export function toSample(blocks: LensBlock[]): Sample {
  return blocks
    .filter((b) => b.bbox[2] > 0 && b.bbox[3] > 0 && normalize(b.source).length > 0)
    .map((b) => ({
      bbox: b.bbox,
      text: normalize(b.source),
      lineHeight: b.line_height > 0 ? b.line_height : b.bbox[3]
    }));
}

const key = (s: Sample) => s.map((b) => `${b.bbox.join(',')}:${b.text}`).join('|');

/**
 * 標本を追加。空の標本と、直前と同一の標本（翻訳完了時の再通知）は追加しない。
 */
export function addSample(samples: Sample[], blocks: LensBlock[]): Sample[] {
  const s = toSample(blocks);
  if (!s.length) return samples;
  const last = samples[samples.length - 1];
  if (last && key(last) === key(s)) return samples;
  return [...samples, s];
}

/** 観測の確定条件。 */
export function observed(elapsedMs: number, samples: number): boolean {
  return elapsedMs >= OBSERVE_MAX_MS || (elapsedMs >= OBSERVE_MIN_MS && samples >= SAMPLES_WANTED);
}

/** 観測の残余時間（秒、切上）。標本が不足の間は最長時間が基準。 */
export function remainingSeconds(elapsedMs: number, samples: number): number {
  const limit = samples >= SAMPLES_WANTED ? OBSERVE_MIN_MS : OBSERVE_MAX_MS;
  return Math.max(0, Math.ceil((limit - elapsedMs) / 1000));
}

function iou(a: Box, b: Box): number {
  const w = Math.min(a[0] + a[2], b[0] + b[2]) - Math.max(a[0], b[0]);
  const h = Math.min(a[1] + a[3], b[1] + b[3]) - Math.max(a[1], b[1]);
  if (w <= 0 || h <= 0) return 0;
  const inter = w * h;
  return inter / (a[2] * a[3] + b[2] * b[3] - inter);
}

function bigrams(s: string): Map<string, number> {
  const m = new Map<string, number>();
  const chars = [...s];
  if (chars.length === 1) m.set(chars[0], 1);
  for (let i = 0; i + 1 < chars.length; i++) {
    const g = chars[i] + chars[i + 1];
    m.set(g, (m.get(g) ?? 0) + 1);
  }
  return m;
}

function similar(a: string, b: string): boolean {
  if (a === b) return true;
  const [x, y] = [bigrams(a), bigrams(b)];
  let common = 0;
  let total = 0;
  for (const [g, n] of x) {
    common += Math.min(n, y.get(g) ?? 0);
    total += n;
  }
  for (const n of y.values()) total += n;
  return total > 0 && (2 * common) / total >= SAME_TEXT;
}

const same = (a: FitBlock, b: FitBlock) => iou(a.bbox, b.bbox) >= SAME_PLACE && similar(a.text, b.text);

function union(boxes: Box[]): Box {
  const x0 = Math.min(...boxes.map((b) => b[0]));
  const y0 = Math.min(...boxes.map((b) => b[1]));
  const x1 = Math.max(...boxes.map((b) => b[0] + b[2]));
  const y1 = Math.max(...boxes.map((b) => b[1] + b[3]));
  return [x0, y0, x1 - x0, y1 - y0];
}

function median(xs: number[]): number {
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.floor(s.length / 2)];
}

/** 2 範囲の間隔（重複時は 0）。 */
const gap = (a0: number, a1: number, b0: number, b1: number) => Math.max(0, Math.max(a0, b0) - Math.min(a1, b1));

export type Band =
  /** 整形先（観測範囲内の物理ピクセル）。 */
  | { kind: 'fit'; rect: Box }
  /** 文字の出現範囲が観測範囲とほぼ同一：整形不要。 */
  | { kind: 'fits' }
  /** 文字を未検出。 */
  | { kind: 'none' };

/**
 * 標本群から字幕領域を算出。`area` は観測範囲の寸法（物理ピクセル）。
 */
export function subtitleBand(samples: Sample[], area: { w: number; h: number }): Band {
  const all = samples.flat();
  if (!all.length) return { kind: 'none' };
  const line = median(all.map((b) => b.lineHeight));

  // 固定表示：全標本の同一位置に同一内容が存在（標本 2 件以上の場合のみ判定可能）
  const fixed = (b: FitBlock) => samples.length >= 2 && samples.every((s) => s.some((o) => same(o, b)));
  const moving: FitBlock[] = [];
  const still: FitBlock[] = [];
  for (const b of all) {
    const list = fixed(b) ? still : moving;
    if (!list.some((o) => same(o, b))) list.push(b);
  }

  let chosen: FitBlock[];
  if (!moving.length) {
    chosen = still;
  } else {
    // 縦方向の間隔が 1 行以内のブロックを同一集団へ統合
    const sorted = [...moving].sort((a, b) => a.bbox[1] - b.bbox[1]);
    const groups: { bottom: number; blocks: FitBlock[] }[] = [];
    for (const b of sorted) {
      const g = groups[groups.length - 1];
      if (g && b.bbox[1] - g.bottom <= line) {
        g.blocks.push(b);
        g.bottom = Math.max(g.bottom, b.bbox[1] + b.bbox[3]);
      } else {
        groups.push({ bottom: b.bbox[1] + b.bbox[3], blocks: [b] });
      }
    }
    // 評価値：標本 1 件当りの文字数（集団の縦範囲内の変動ブロック）。異文の累計では
    // 更新頻度の高い計数表示（残余時間等）が字幕を上回るため、標本数で正規化。
    const chars = (b: FitBlock) => [...b.text].length;
    const score = (g: { blocks: FitBlock[] }) => {
      const top = Math.min(...g.blocks.map((b) => b.bbox[1]));
      const bottom = Math.max(...g.blocks.map((b) => b.bbox[1] + b.bbox[3]));
      const inside = (b: FitBlock) => {
        const mid = b.bbox[1] + b.bbox[3] / 2;
        return mid >= top && mid <= bottom && !still.some((o) => same(o, b));
      };
      return samples.reduce((n, sample) => n + sample.filter(inside).reduce((m, b) => m + chars(b), 0), 0);
    };
    chosen = groups.reduce((best, g) => (score(g) > score(best) ? g : best)).blocks;
    // 隣接する固定表示（話者名の札等）を算入
    const [x, y, w, h] = union(chosen.map((b) => b.bbox));
    chosen = [
      ...chosen,
      ...still.filter(
        ({ bbox: s }) =>
          gap(y, y + h, s[1], s[1] + s[3]) <= line && gap(x, x + w, s[0], s[0] + s[2]) <= line * 2
      )
    ];
  }

  // 余白：未観測の長文・複数行への備え（横は広範、縦は半行）
  const [x, y, w, h] = union(chosen.map((b) => b.bbox));
  const padX = Math.max(line * 2, w * 0.15);
  const padY = line * 0.5;
  const x0 = Math.max(0, Math.floor(x - padX));
  const y0 = Math.max(0, Math.floor(y - padY));
  const x1 = Math.min(area.w, Math.ceil(x + w + padX));
  const y1 = Math.min(area.h, Math.ceil(y + h + padY));
  if (x1 <= x0 || y1 <= y0) return { kind: 'none' };
  if ((x1 - x0) * (y1 - y0) >= area.w * area.h * WORTH_FITTING) return { kind: 'fits' };
  return { kind: 'fit', rect: [x0, y0, x1 - x0, y1 - y0] };
}
