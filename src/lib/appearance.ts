/**
 * Apply the `appearance` settings to the document: theme, density, accent
 * colour and reduced motion. Called on start-up and whenever settings are
 * saved, so changes show without a restart.
 */

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

const HEX = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;

export function applyAppearance(settings: any, root: HTMLElement = document.documentElement) {
  const a = settings?.appearance ?? {};
  root.dataset.theme = a.theme ?? 'system';
  root.dataset.density = a.density ?? 'normal';
  if (typeof a.accent === 'string' && HEX.test(a.accent.trim())) {
    root.style.setProperty('--ml-accent', a.accent.trim());
  } else {
    root.style.removeProperty('--ml-accent');
  }
  root.style.setProperty('--ml-body-zoom', String(bodyZoom(a.text_size)));
  if (a.reduced_motion) root.dataset.reducedMotion = 'true';
  else delete root.dataset.reducedMotion;
}
