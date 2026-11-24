/**
 * メインウィンドウ専用のドット絵外観。
 * Lens・オーバーレイ・字幕など訳文表示のウィンドウは対象外
 * （原文のレイアウト・書体・配色を遵守）。
 */
import { stripeTile } from './pix';

let tileUrl: string | null = null;

/** 背景の斜め縞タイルを 1 回だけ生成し、以後は静止画として再利用。 */
function backgroundTile(): string | null {
  if (tileUrl) return tileUrl;
  try {
    tileUrl = stripeTile().toDataURL('image/png');
  } catch {
    tileUrl = null; // Canvas 非対応環境（テスト等）は単色背景
  }
  return tileUrl;
}

export function applyPixelSkin(on: boolean, root: HTMLElement = document.documentElement) {
  if (!on) {
    delete root.dataset.skin;
    root.style.removeProperty('--ml-px-bg');
    return;
  }
  root.dataset.skin = 'pixel';
  const url = backgroundTile();
  if (url) root.style.setProperty('--ml-px-bg', `url(${url})`);
}
