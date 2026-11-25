/**
 * 同一要素の内容差替え時の切替演出（設定の分類切替・説明窓・履歴の詳細等）。
 * 新規挿入の要素は CSS（design/pixel-motion.css）で処理。本モジュールは
 * 要素を再生成せずに内容のみ変わる箇所向け（モデル管理等の状態保持が必要な領域）。
 * Web Animations API で transform・opacity・clip-path を段階変化。推論中・動き削減時は無処理。
 */

export type Effect = 'slide-left' | 'slide-right' | 'slide-up' | 'slide-down' | 'type' | 'open';

/** 各演出のキーフレーム・所要時間・段数（steps）。 */
export const EFFECTS: Record<Effect, { frames: Keyframe[]; ms: number; steps: number }> = {
  // 小分類：左右方向（次の項目は右から、前の項目は左から）
  'slide-left': { frames: [{ transform: 'translateX(24px)', opacity: 0 }, { transform: 'none', opacity: 1 }], ms: 120, steps: 3 },
  'slide-right': { frames: [{ transform: 'translateX(-24px)', opacity: 0 }, { transform: 'none', opacity: 1 }], ms: 120, steps: 3 },
  // 大分類：上下方向（分類一覧の並び順に一致）
  'slide-up': { frames: [{ transform: 'translateY(24px)', opacity: 0 }, { transform: 'none', opacity: 1 }], ms: 120, steps: 3 },
  'slide-down': { frames: [{ transform: 'translateY(-24px)', opacity: 0 }, { transform: 'none', opacity: 1 }], ms: 120, steps: 3 },
  // 説明窓：左から右への文字送り（会話窓の表示処理）
  type: { frames: [{ clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0 0 0)' }], ms: 120, steps: 6 },
  // 詳細窓：中央の横線から上下へ展開
  open: { frames: [{ clipPath: 'inset(50% 0 50% 0)' }, { clipPath: 'inset(0 0 0 0)' }], ms: 120, steps: 4 }
};

/** 演出の実行可否（メインウィンドウのドット絵外観かつ演出有効時のみ）。 */
export function motionEnabled(root: HTMLElement = document.documentElement): boolean {
  return root.dataset.skin === 'pixel' && root.dataset.motion === 'on';
}

/** 要素へ演出を 1 回再生。実行中の同種演出は破棄し最初から再生。 */
export function play(node: HTMLElement, effect: Effect): Animation | null {
  if (typeof node.animate !== 'function' || !motionEnabled()) return null;
  const { frames, ms, steps } = EFFECTS[effect];
  for (const a of node.getAnimations?.() ?? []) if ((a as Animation & { id: string }).id === 'px-replay') a.cancel();
  const a = node.animate(frames, { duration: ms, easing: `steps(${steps}, end)`, fill: 'backwards' });
  a.id = 'px-replay';
  return a;
}

/**
 * key の変化時に演出を再生する Svelte アクション（初回表示時は無処理）。
 * effect は関数指定も可能：変化前後の key から方向を決定。
 */
export function replay<K>(
  node: HTMLElement,
  param: { key: K; effect: Effect | ((prev: K, next: K) => Effect | null) }
) {
  let prev = param.key;
  return {
    update(next: { key: K; effect: Effect | ((prev: K, next: K) => Effect | null) }) {
      if (Object.is(next.key, prev)) return;
      const e = typeof next.effect === 'function' ? next.effect(prev, next.key) : next.effect;
      prev = next.key;
      if (e) play(node, e);
    }
  };
}
