import { describe, expect, it } from 'vitest';
import { slantPolygon } from './slant';
import { ROTORS, SELECTED_ANGLE, SIDE_WIDTH, dialGeometry, hitTest, labelPlacement, rotorSize } from './dial';
import { homeDot } from './home';

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

  it('魔法陣の外周は右側の情報欄と重複しない', () => {
    // 論理 640×368（既定）・560×348（最小）、情報欄 324：外周の外縁（r + 20q）が情報欄の手前
    for (const [w, h] of [[640, 368], [560, 348], [960, 456]]) {
      const g = dialGeometry(w, h, 5, 0);
      expect(g.cx + g.r + (20 * g.r) / 158).toBeLessThanOrEqual(w - SIDE_WIDTH);
      expect(g.cx - g.r - (20 * g.r) / 158).toBeGreaterThanOrEqual(0);
    }
  });
});

describe('hitTest', () => {
  const g = dialGeometry(640, 368, 5, 1);
  it('ノード中心は該当項目', () => {
    g.nodes.forEach((n, i) => expect(hitTest(g, n.x + 3, n.y - 3)).toBe(i));
  });
  it('中心・余白は該当なし（中心の決定は廃止、項目クリックで即起動）', () => {
    expect(hitTest(g, g.cx, g.cy)).toBeNull();
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

describe('homeDot', () => {
  it('既定寸法は 2 倍表示、拡大率 125% は 2 端末画素', () => {
    expect(homeDot(1280, 736, 1)).toBe(2);
    expect(homeDot(1280, 736, 1.25)).toBeCloseTo(1.6);
    expect(homeDot(1280, 736, 2)).toBe(2);
  });
  it('最小寸法（1120×700）は 2 倍表示を維持', () => {
    expect(homeDot(1120, 636, 1)).toBe(2);
  });
  it('高さ不足は端末画素単位で縮小（見切れ防止）', () => {
    // 1920×1080・150% の画面（作業領域 1280×672 相当）
    expect(homeDot(1280, 608, 1.5)).toBeCloseTo(2 / 1.5);
    expect(homeDot(1000, 560, 1)).toBe(1);
  });
});

describe('回転体', () => {
  it('寸法は奇数（中心画素＝回転中心）、外周ほど大きい', () => {
    for (const r of [120, 158, 171]) {
      const sizes = ROTORS.map((o) => rotorSize(o.id, r));
      expect(sizes.every((n) => n % 2 === 1)).toBe(true);
      expect(sizes[0]).toBeGreaterThan(sizes[1]);
      expect(sizes[1]).toBeGreaterThan(sizes[2]);
    }
  });
});
