import { fitFontSize } from "./lens-overlay";

/**
 * Svelte action: shrink the font until the translation fits its box
 * (Lens behaviour). The text lives in an inline-block child so it can be
 * centred vertically; that child is measured against the box.
 */
export function fit(
  node: HTMLElement,
  params: { text: string | null; max: number },
) {
  const apply = (p: { text: string | null; max: number }) => {
    const min = 7;
    const size = fitFontSize(
      (px) => {
        node.style.fontSize = `${px}px`;
        const inner = node.firstElementChild as HTMLElement | null;
        const h = inner ? inner.offsetHeight : node.scrollHeight;
        const w = inner ? inner.scrollWidth : node.scrollWidth;
        return h <= node.clientHeight + 1 && w <= node.clientWidth + 1;
      },
      min,
      Math.max(min, p.max),
    );
    node.style.fontSize = `${size}px`;
  };
  apply(params);
  return { update: apply };
}
