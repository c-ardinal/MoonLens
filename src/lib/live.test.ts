import { describe, expect, it } from "vitest";
import { liveBoxes, liveText } from "./live";
import type { LensBlock } from "./ipc";

const block = (over: Partial<LensBlock> = {}): LensBlock => ({
  bbox: [20, 10, 200, 40],
  line_height: 20,
  source: "Hello",
  translation: "こんにちは",
  colors: null,
  ...over,
});

describe("liveBoxes", () => {
  it("maps capture pixels to CSS px by the device pixel ratio", () => {
    const [b] = liveBoxes([block()], new Set(), 2);
    expect(b.rect.left).toBeCloseTo(10 - 2);
    expect(b.rect.top).toBeCloseTo(5 - 2);
    expect(b.rect.width).toBeCloseTo(100 + 4);
    expect(b.rect.height).toBeCloseTo(20 + 4);
  });

  it("skips stale, same-language and empty blocks", () => {
    const boxes = liveBoxes(
      [
        block({ source: "a" }),
        block({ source: "b", same_language: true }),
        block({ source: "c", bbox: [0, 0, 0, 10] }),
        block({ source: "d" }),
      ],
      new Set([3]),
      1,
    );
    expect(boxes.map((b) => b.block.source)).toEqual(["a"]);
  });

  it("marks blocks without a translation as pending", () => {
    const [b] = liveBoxes([block({ translation: null })], new Set(), 1);
    expect(b.pending).toBe(true);
  });
});

describe("liveText", () => {
  it("joins source and translation, keeps same-language text, skips pending", () => {
    const blocks = [
      block({ source: " Hello ", translation: "こんにちは" }),
      block({ source: "東京", translation: null, same_language: true }),
      block({ source: "World", translation: null }),
    ];
    expect(liveText(blocks)).toEqual({
      source: "Hello\n\n東京\n\nWorld",
      translation: "こんにちは\n\n東京",
    });
  });
});
