/**
 * 同一要素の内容差替え時の切替演出（設定の分類切替・説明欄・履歴の詳細等）。
 * 新規挿入の要素は CSS（design/pixel-motion.css）で処理。本モジュールは
 * 要素を再生成せずに内容のみ変わる箇所向け（モデル管理等の状態保持が必要な領域）。
 * Web Animations API で transform・opacity・clip-path を段階変化。推論中・動き削減時は無処理。
 */

export type Effect = 'slide-left' | 'slide-right' | 'type' | 'open';

/**
 * 各演出のキーフレーム・所要時間・段数。
 * 補間は steps(n, start)：開始直後に 1 段目を表示し、空白のコマを作らない
 * （空白のコマは描画遅延に見えるため）。
 */
export const EFFECTS: Record<Effect, { frames: Keyframe[]; ms: number; steps: number }> = {
  // 枠内の項目入替：並び順に応じた左右の小移動（枠・タブは固定）
  'slide-left': { frames: [{ transform: 'translateX(16px)', opacity: 0.2 }, { transform: 'none', opacity: 1 }], ms: 100, steps: 4 },
  'slide-right': { frames: [{ transform: 'translateX(-16px)', opacity: 0.2 }, { transform: 'none', opacity: 1 }], ms: 100, steps: 4 },
  // 説明欄：左から右への文字送り（会話ウィンドウの表示処理）
  type: { frames: [{ clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0 0 0)' }], ms: 120, steps: 6 },
  // 詳細欄：中央の横線から上下へ展開
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
  const a = node.animate(frames, { duration: ms, easing: `steps(${steps}, start)`, fill: 'backwards' });
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

/** 画面遷移の帯：幅（px）・所要時間（ms）・傾斜（見出し帯と同じ 0.3）。 */
export const WIPE = { band: 96, ms: 220, k: 0.3 } as const;

/**
 * 画面遷移：斜めの帯が左から右へ通過し、帯の後方から新画面を表示。
 * 帯（可視の先端）を伴わせ、表示の進行を演出として明示（先端のない段階表示は描画遅延に見える）。
 * 帯は transform のみ（合成処理）、新画面は clip-path を同一時間軸で等速変化。
 * @param pageEl 新画面の最上位要素
 * @param bandEl 帯の要素（position: fixed、通常時は画面外）
 */
export function pageWipe(pageEl: HTMLElement, bandEl: HTMLElement): void {
  if (typeof pageEl.animate !== 'function' || !motionEnabled()) return;
  const r = pageEl.getBoundingClientRect();
  if (r.width <= 0 || r.height <= 0) return;
  const off = Math.round(r.height * WIPE.k);
  // zoom 指定の要素では clip-path の px が拡縮対象：要素内の座標系へ換算
  const z = r.width / (pageEl.offsetWidth || r.width);
  const px = (v: number) => `${Math.round(v / z)}px`;
  const clip = (e: number) => `polygon(${px(-off)} 0, ${px(e)} 0, ${px(e - off)} 100%, ${px(-off)} 100%)`;
  const end = r.width + off;
  const timing: KeyframeAnimationOptions = { duration: WIPE.ms, easing: 'linear' };
  const a = pageEl.animate([{ clipPath: clip(0) }, { clipPath: clip(end) }], timing);
  a.id = 'px-page';
  Object.assign(bandEl.style, { left: `${r.left}px`, top: `${r.top}px`, height: `${r.height}px`, width: `${WIPE.band}px` });
  const skew = `skewX(${(-Math.atan(WIPE.k) * 180) / Math.PI}deg)`;
  const b = bandEl.animate(
    [
      { transform: `translateX(0px) ${skew}`, visibility: 'visible' },
      { transform: `translateX(${end}px) ${skew}`, visibility: 'visible' }
    ],
    timing
  );
  b.id = 'px-page';
}

/**
 * ホイール操作で項目を 1 個ずつ切替える Svelte アクション（魔法陣と同じ感度）。
 * 1 ノッチ（累積 60）で 1 項目、連続入力は 140 ms 間隔。
 */
export function wheelStep(node: HTMLElement, onStep: (delta: 1 | -1) => void) {
  let acc = 0;
  let at = 0;
  let cb = onStep;
  const onWheel = (e: WheelEvent) => {
    if (e.ctrlKey) return; // 拡大縮小操作
    e.preventDefault();
    acc += Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
    const now = performance.now();
    if (Math.abs(acc) < 60 || now - at < 140) return;
    at = now;
    cb(acc > 0 ? 1 : -1);
    acc = 0;
  };
  node.addEventListener('wheel', onWheel, { passive: false });
  return {
    update(next: (delta: 1 | -1) => void) {
      cb = next;
    },
    destroy() {
      node.removeEventListener('wheel', onWheel);
    }
  };
}
