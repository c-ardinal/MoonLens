/**
 * Apply the `appearance` settings to the document: theme, density, accent
 * colour and reduced motion. Called on start-up and whenever settings are
 * saved, so changes show without a restart.
 */

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
  if (a.reduced_motion) root.dataset.reducedMotion = 'true';
  else delete root.dataset.reducedMotion;
}
