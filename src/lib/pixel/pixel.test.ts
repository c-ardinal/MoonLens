import { describe, expect, it } from 'vitest';
import { slantPolygon } from './slant';
import { ROTORS, SELECTED_ANGLE, SIDE_WIDTH, dialGeometry, hitTest, labelPlacement, moonPart, moonRadius, rotorSize } from './dial';
import { homeDot, titleOffsets } from './home';
import { inkBounds, slabCenterOrigin } from './pix';

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

describe('中心の月', () => {
  it('左側が明部の三日月、右上は影、円外は対象外', () => {
    const r = moonRadius(158);
    expect(moonPart(-r + 1, 0, r)).toBe('lit');
    expect(moonPart(0, r - 1, r)).toBe('lit');
    expect(moonPart(Math.round(r / 2), -Math.round(r / 2), r)).toBe('shade');
    expect(moonPart(0, 0, r)).toBe('shade');
    expect(moonPart(r + 2, 0, r)).toBeNull();
  });
  it('枠環を含め、回転体（正方形）の頂点より内側へ収まる', () => {
    for (const r of [120, 158, 171]) {
      const corner = (rotorSize('core-b', r) - 1) / 2 - 12;
      expect(moonRadius(r) + 3).toBeLessThan(corner);
    }
  });
});

describe('帯内の字面配置', () => {
  /** 幅 w・高さ h の画素列（指定範囲のみ不透明）。 */
  const bitmap = (w: number, h: number, x0: number, y0: number, x1: number, y1: number) => {
    const d = new Uint8ClampedArray(w * h * 4);
    for (let j = y0; j <= y1; j++) for (let i = x0; i <= x1; i++) d[(j * w + i) * 4 + 3] = 255;
    return d;
  };
  it('打点範囲：斜体は上の行ほど右へ拡大、打点なしは null', () => {
    expect(inkBounds(bitmap(10, 10, 2, 3, 5, 6), 10, 10)).toEqual({ x0: 2, y0: 3, x1: 5, y1: 6 });
    // k=0.5：行 3 は +3、行 6 は +1
    expect(inkBounds(bitmap(10, 10, 2, 3, 5, 6), 10, 10, 0.5)).toEqual({ x0: 3, y0: 3, x1: 8, y1: 6 });
    expect(inkBounds(new Uint8ClampedArray(400), 10, 10)).toBeNull();
  });
  it('字面の上下・左右の余白は均等（差は 1 ドット以内）', () => {
    const ink = { x0: 3, y0: 5, x1: 40, y1: 20 };
    const slab = { x: 0, y: 0, w: 60, h: 32, k: 0 };
    const o = slabCenterOrigin(ink, slab);
    const top = o.y + ink.y0 - slab.y,
      bottom = slab.y + slab.h - 1 - (o.y + ink.y1);
    const left = o.x + ink.x0 - slab.x,
      right = slab.x + slab.w - 1 - (o.x + ink.x1);
    expect(Math.abs(top - bottom)).toBeLessThanOrEqual(1);
    expect(Math.abs(left - right)).toBeLessThanOrEqual(1);
  });
});

describe('titleOffsets', () => {
  it('切替前の機能名は回転方向へ退場、切替後は反対側から同時に入場', () => {
    const start = titleOffsets(0, 1, 0);
    expect(start.leave).toBe(0);
    expect(start.enter).toBeGreaterThanOrEqual(62);
    const mid = titleOffsets(0.5, 1, 0);
    expect(mid.leave).toBeLessThan(0);
    expect(mid.enter).toBeGreaterThan(0);
    // 両者の間隔は一定：重複なしで連続移動
    expect(mid.enter - mid.leave!).toBe(start.enter - start.leave!);
    expect(titleOffsets(1, 1, 0)).toEqual({ enter: 0, leave: -62 });
  });
  it('逆回転は上下反転', () => {
    expect(titleOffsets(0.5, -1, 0)).toEqual({ enter: -titleOffsets(0.5, 1, 0).enter, leave: -titleOffsets(0.5, 1, 0).leave! });
  });
  it('演出途中の再切替：退場側は途中位置から開始', () => {
    expect(titleOffsets(0, 1, 20).leave).toBe(20);
    expect(titleOffsets(1, 1, 20).leave).toBe(-62);
  });
  it('退場側なし（同一項目への復帰）は入場側のみ', () => {
    expect(titleOffsets(1, 1)).toEqual({ enter: 0, leave: null });
  });
});
