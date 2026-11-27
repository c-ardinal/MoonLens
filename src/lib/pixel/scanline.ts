/**
 * 走査線の除外領域。対象要素の表示矩形を CSS 変数（--ml-hole-*）へ反映し、
 * 走査線オーバーレイ（pixel.css の body::after）を当該矩形のみ切抜き。
 * 用途：画像翻訳の画像（原文準拠の表示、走査線は可読性を損なうため）。
 */
const PROPS = ['x', 'y', 'w', 'h'] as const;

export function scanlineHole(node: HTMLElement) {
  const root = document.documentElement;
  const sync = () => {
    const r = node.getBoundingClientRect();
    // 走査線の周期（2px）へ整列：切抜きの境界で縞の欠けを防止
    const y = Math.floor(r.top / 2) * 2;
    const values = [Math.floor(r.left), y, Math.ceil(r.right) - Math.floor(r.left), Math.ceil((r.bottom - y) / 2) * 2];
    PROPS.forEach((p, i) => root.style.setProperty(`--ml-hole-${p}`, `${values[i]}px`));
  };
  // 位置の変化（ウィンドウ寸法・見出し行の折返し）は親要素の寸法変化を伴うため、双方を監視
  const ro = new ResizeObserver(sync);
  ro.observe(node);
  if (node.parentElement) ro.observe(node.parentElement);
  sync();
  return {
    destroy() {
      ro.disconnect();
      PROPS.forEach((p) => root.style.removeProperty(`--ml-hole-${p}`));
    }
  };
}
