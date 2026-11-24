/**
 * メインウィンドウの演出制御。推論中（字幕・ライブ翻訳・翻訳実行）と動き削減設定時は停止。
 * 停止状態は `<html data-motion>` へ反映し、CSS 演出も同期。
 */
import { untrack } from 'svelte';
import { onPinState, onSubtitle, subtitleState } from '$lib/ipc';

const state = $state({ holds: 0, subtitle: false, pinned: false, reduced: false });

export const motion = {
  /** 推論の実行中。 */
  get busy() {
    return state.holds > 0 || state.subtitle || state.pinned;
  },
  /** 演出の実行可否。 */
  get on() {
    return !this.busy && !state.reduced;
  }
};

/**
 * 推論区間の宣言。戻り値の関数で解除（重複呼出しは無視）。
 * 計数の読み書きは追跡外：呼出し元の $effect が計数へ依存し再実行を繰返す事態を防止。
 */
export function holdMotion(): () => void {
  untrack(() => state.holds++);
  let done = false;
  return () => {
    if (done) return;
    done = true;
    untrack(() => state.holds--);
  };
}

/** 動き削減設定・字幕・ライブ翻訳の状態監視を開始。戻り値で終了。 */
export function watchMotion(root: HTMLElement = document.documentElement): () => void {
  const mq = matchMedia('(prefers-reduced-motion: reduce)');
  const sync = () => (state.reduced = mq.matches || root.dataset.reducedMotion === 'true');
  sync();
  mq.addEventListener('change', sync);
  const mo = new MutationObserver(sync);
  mo.observe(root, { attributes: true, attributeFilter: ['data-reduced-motion'] });

  const stops = [
    Promise.resolve().then(() =>
      onSubtitle((e) => {
        if (e.kind === 'started') state.subtitle = true;
        else if (e.kind === 'stopped') state.subtitle = false;
      })
    ),
    Promise.resolve().then(() => onPinState((pinned) => (state.pinned = pinned)))
  ];
  stops.forEach((s) => s.catch(() => {}));
  subtitleState()
    .then((on) => (state.subtitle = !!on))
    .catch(() => {});

  return () => {
    mq.removeEventListener('change', sync);
    mo.disconnect();
    stops.forEach((s) => s.then((u) => u()).catch(() => {}));
  };
}
