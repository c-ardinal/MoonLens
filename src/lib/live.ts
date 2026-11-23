/**
 * Pure helpers for the live translation window (`/live`). The window
 * covers the pinned region exactly, so block boxes (capture pixels,
 * relative to the region) map to CSS px by the device pixel ratio.
 */

import type { LensBlock } from "./ipc";
import { blockCssRect, growRoom, type CssRect } from "./lens-overlay";

export type LiveEvent =
  | { kind: "blocks"; blocks: LensBlock[]; translating: boolean }
  | { kind: "stale"; indices: number[] }
  | { kind: "clear" };

export interface LiveBox {
  block: LensBlock;
  rect: CssRect & { key: string };
  /** Translation still on its way: the box masks the source meanwhile. */
  pending: boolean;
  /** Room (CSS px) a longer translation may take to the right. */
  grow: number;
}

/**
 * Boxes to paint: blocks with an area, not already in the target
 * language and not hidden as stale.
 */
export function liveBoxes(
  blocks: LensBlock[],
  hidden: Set<number>,
  dpr: number,
  /** Width of the region in capture pixels. */
  regionWidth = Infinity,
): LiveBox[] {
  const origin = { x: 0, y: 0, w: 0, h: 0 };
  const out: LiveBox[] = [];
  blocks.forEach((b, i) => {
    if (hidden.has(i) || b.same_language || b.bbox[2] <= 0 || b.bbox[3] <= 0)
      return;
    const r = blockCssRect(b, origin, dpr);
    out.push({
      block: b,
      rect: { ...r, key: `${i}:${b.source}` },
      pending: b.translation == null,
      grow: growRoom(blocks, i, regionWidth) / dpr,
    });
  });
  return out;
}
