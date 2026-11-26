/**
 * 文字サイズ設定の拡縮率（文字寸法 ÷ 32px）。Canvas 描画（ホーム）用。
 * DOM は CSS 変数 --ml-body-zoom を参照（appearance.ts が同時に更新）。
 */
export const textScale = $state({ zoom: 0.75 });
