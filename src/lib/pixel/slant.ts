/**
 * 斜めの帯・名札：要素の外形を階段状の平行四辺形へ切抜き。
 * clip-path の斜線は補間されるため、ドット単位の階段で多角形を構成。
 */

/** ドット 1 個の CSS 寸法（論理解像度の 2 倍表示）。 */
export const DOT = 2;

/**
 * 幅 w・高さ h の要素を、上の行ほど右へ k 倍ずらす平行四辺形の多角形。
 * 各頂点はドット格子（DOT の倍数）に整列。
 */
export function slantPolygon(w: number, h: number, k = 0.3, dot = DOT): string {
  const rows = Math.max(1, Math.round(h / dot));
  const offsetOf = (row: number) => Math.floor((rows - 1 - row) * k) * dot; // row 0 = 最上段
  const top = offsetOf(0);
  const right = (o: number) => Math.max(o + dot, w - top + o);
  const left: [number, number][] = [];
  const rightEdge: [number, number][] = [];
  for (let row = 0; row < rows; row++) {
    const o = offsetOf(row);
    const y0 = row * dot,
      y1 = Math.min(h, (row + 1) * dot);
    const prev = row > 0 ? offsetOf(row - 1) : null;
    if (prev !== o) {
      left.push([o, y0]);
      rightEdge.push([right(o), y0]);
    }
    const next = row < rows - 1 ? offsetOf(row + 1) : null;
    if (next !== o) {
      left.push([o, y1]);
      rightEdge.push([right(o), y1]);
    }
  }
  const pts = [...rightEdge, ...left.reverse()];
  return `polygon(${pts.map(([x, y]) => `${x}px ${y}px`).join(', ')})`;
}

/** 要素寸法の変化に追従して階段状に切抜く Svelte アクション。 */
export function slant(node: HTMLElement, k = 0.3) {
  let factor = k;
  const apply = () => {
    const { width, height } = node.getBoundingClientRect();
    if (width > 0 && height > 0) node.style.clipPath = slantPolygon(Math.round(width), Math.round(height), factor);
  };
  const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(apply);
  ro?.observe(node);
  apply();
  return {
    update(next: number) {
      factor = next;
      apply();
    },
    destroy() {
      ro?.disconnect();
      node.style.clipPath = '';
    }
  };
}
