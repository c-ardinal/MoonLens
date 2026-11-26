/**
 * 設定の即時保存：変更の都度 `schedule`、保存処理は常に 1 件のみ実行。
 * 実行中の変更は完了後に 1 回再保存（最終状態を確実に反映）。
 */
export interface Autosave {
  /** `delay` ミリ秒後に保存。連続入力（スライダー等）は最後の 1 回へ集約。 */
  schedule(delay?: number): void;
  /** 待機中の保存を即時実行し、完了まで待機（画面離脱時）。 */
  flush(): Promise<void>;
  /** 待機中の保存を破棄。 */
  cancel(): void;
}

export function createAutosave(save: () => Promise<void>): Autosave {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let running: Promise<void> | null = null;
  let again = false;

  function run(): Promise<void> {
    if (running) {
      again = true;
      return running;
    }
    running = (async () => {
      try {
        do {
          again = false;
          await save();
        } while (again);
      } finally {
        running = null;
      }
    })();
    return running;
  }

  function cancel() {
    clearTimeout(timer);
    timer = undefined;
  }

  return {
    schedule(delay = 0) {
      cancel();
      timer = setTimeout(() => {
        timer = undefined;
        // 失敗の通知は save 側の責務。次回の変更で再実行
        run().catch(() => {});
      }, delay);
    },
    flush() {
      if (timer === undefined) return running ?? Promise.resolve();
      cancel();
      return run();
    },
    cancel
  };
}
