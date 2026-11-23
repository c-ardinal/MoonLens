import { describe, expect, it } from 'vitest';
import { slantPolygon } from './slant';
import { SELECTED_ANGLE, dialGeometry, hitTest, labelPlacement } from './dial';

const points = (poly: string) =>
  poly
    .replace(/^polygon\(|\)$/g, '')
    .split(', ')
    .map((p) => p.split(' ').map((v) => Number(v.replace('px', ''))));

describe('slantPolygon', () => {
  it('上の行ほど右へずれる階段状の平行四辺形', () => {
    const pts = points(slantPolygon(100, 20, 0.3));
    const xs = pts.map(([x]) => x);
    // 頂点は全てドット格子（2px）上
    expect(pts.every(([x, y]) => x % 2 === 0 && y % 2 === 0)).toBe(true);
    // 最上段の左端 > 最下段の左端（右上がりの傾斜）
    const topLeft = Math.min(...pts.filter(([, y]) => y === 0).map(([x]) => x));
    const bottomLeft = Math.min(...pts.filter(([, y]) => y === 20).map(([x]) => x));
    expect(topLeft).toBeGreaterThan(bottomLeft);
    expect(bottomLeft).toBe(0);
    expect(Math.max(...xs)).toBeLessThanOrEqual(100);
  });

  it('傾斜 0 は矩形', () => {
    expect(points(slantPolygon(40, 10, 0))).toEqual([
      [40, 0],
      [40, 10],
      [0, 10],
      [0, 0]
    ]);
  });
});

describe('dialGeometry', () => {
  it('選択項目は常に右上へ回転', () => {
    for (const sel of [0, 2, 4]) {
      const g = dialGeometry(640, 368, 5, sel);
      expect(g.nodes[sel].angle).toBe(SELECTED_ANGLE);
      expect(g.nodes[sel].x).toBeGreaterThan(g.cx);
      expect(g.nodes[sel].y).toBeLessThan(g.cy);
    }
  });

  it('魔法陣は右側の情報欄と重複しない', () => {
    // モック基準（論理 640×368、情報欄 310）：外周ルーン環は情報欄の手前
    const g = dialGeometry(640, 368, 5, 0);
    expect(g.r).toBe(158);
    expect(g.cx + g.r).toBeLessThanOrEqual(640 - 310 + 10);
  });
});

describe('hitTest', () => {
  const g = dialGeometry(640, 368, 5, 1);
  it('ノード中心は該当項目', () => {
    g.nodes.forEach((n, i) => expect(hitTest(g, n.x + 3, n.y - 3)).toBe(i));
  });
  it('中心は決定、余白は該当なし', () => {
    expect(hitTest(g, g.cx, g.cy)).toBe('center');
    expect(hitTest(g, 2, 2)).toBeNull();
  });
});

describe('labelPlacement', () => {
  it('上部は上、下部は横、側面は下', () => {
    expect(labelPlacement(-90)).toBe('above');
    expect(labelPlacement(120)).toBe('left');
    expect(labelPlacement(60)).toBe('right');
    expect(labelPlacement(18)).toBe('below');
    expect(labelPlacement(162)).toBe('below');
  });
});
