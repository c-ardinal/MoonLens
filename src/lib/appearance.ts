/**
 * Apply the `appearance` settings to the document: theme, density, accent
 * colour and reduced motion. Called on start-up and whenever settings are
 * saved, so changes show without a restart.
 */

import { applyPalette, normalizeAccent } from './pixel/palette.svelte';
import { textScale } from './pixel/text-scale.svelte';

/** 本文画面の文字寸法（CSS px）。ドット書体 16 ドットの整数 px 段階。 */
export const TEXT_SIZE_PX: Record<string, number> = {
  xsmall: 16,
  small: 20,
  medium: 24,
  large: 28,
  xlarge: 32
};
/** ドット絵外観の部品寸法の基準（32px 文字前提で設計）。 */
const DESIGN_TEXT_PX = 32;

/** 本文画面の拡縮率：文字寸法 ÷ 設計基準。未知の値は標準。 */
export function bodyZoom(textSize: unknown): number {
  const px = TEXT_SIZE_PX[String(textSize)] ?? TEXT_SIZE_PX.medium;
  return px / DESIGN_TEXT_PX;
}

/** 「システム設定」選択時の OS 配色監視の解除関数。 */
let stopSystemWatch: (() => void) | null = null;

export function applyAppearance(settings: any, root: HTMLElement = document.documentElement) {
  const a = settings?.appearance ?? {};
  const theme = a.theme ?? 'dark';
  root.dataset.theme = theme;
  root.dataset.density = a.density ?? 'normal';
  // 訳文表示のウィンドウ（ドット絵外観の対象外）用の主色
  root.style.setProperty('--ml-accent', normalizeAccent(a.accent));

  // ドット絵外観の配色：テーマ（システム設定は OS の配色へ追従）とアクセントカラーから算出
  stopSystemWatch?.();
  stopSystemWatch = null;
  const mq = typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: light)') : null;
  const sync = () =>
    applyPalette(theme === 'light' || (theme === 'system' && !!mq?.matches) ? 'light' : 'dark', a.accent, root);
  sync();
  if (theme === 'system' && mq) {
    mq.addEventListener('change', sync);
    stopSystemWatch = () => mq.removeEventListener('change', sync);
  }

  const zoom = bodyZoom(a.text_size);
  root.style.setProperty('--ml-body-zoom', String(zoom));
  if (root === document.documentElement) textScale.zoom = zoom;
  if (a.reduced_motion) root.dataset.reducedMotion = 'true';
  else delete root.dataset.reducedMotion;
}
