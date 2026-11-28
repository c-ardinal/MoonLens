/**
 * Pure helpers for continuous window translation (`/lens`). The body of
 * a lens window covers the observed range exactly, so block boxes
 * (capture pixels, relative to the range) map to CSS px by the device
 * pixel ratio.
 */

import type { LensBlock } from "./ipc";
import { blockCssRect, growRoom, type CssRect } from "./lens-overlay";

export type LiveEvent =
  | {
      kind: "blocks";
      blocks: LensBlock[];
      translating: boolean;
      /** Engine of the new translations, once known. */
      provider?: string;
      /** The new translations came from a cloud provider. */
      via_cloud?: boolean;
      /** Target language the blocks were translated into. */
      language?: string;
    }
  | { kind: "stale"; indices: number[] }
  | { kind: "clear" }
  /** Text recognition of a changed range started / ended. */
  | { kind: "reading"; on: boolean };

/** Events of a continuously reading Lens window (`moonlens://lens-live`). */
export type LensLiveEvent =
  | LiveEvent
  | { kind: "decoded"; text: string }
  | { kind: "error"; message: string };

/**
 * Source and translation of live blocks as plain text (the Lens text
 * view and copy button). Blocks already in the target language show
 * their source; blocks still with the model are left out.
 */
export function liveText(blocks: LensBlock[]): {
  source: string;
  translation: string;
} {
  const join = (xs: string[]) =>
    xs
      .map((x) => x.trim())
      .filter(Boolean)
      .join("\n\n");
  return {
    source: join(blocks.map((b) => b.source)),
    translation: join(
      blocks.map((b) => b.translation ?? (b.same_language ? b.source : "")),
    ),
  };
}

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
