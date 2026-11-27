/**
 * 表示言語の切替を Svelte の再描画へ接続。
 * `t()`／`locale()` を描画中に呼出した箇所は、言語変更時に再評価。
 * 全ウィンドウ共通のルートレイアウトが読込。
 */
import { onLocaleChange, setTracker } from './index';

let version = $state(0);
// 値を返却：`void version` 形式は本番ビルドの最適化が読出しを除去し、再描画が停止。
setTracker(() => version);
onLocaleChange(() => version++);
