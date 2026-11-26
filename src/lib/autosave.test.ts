import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createAutosave } from './autosave';

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => (resolve = r));
  return { promise, resolve };
}

describe('createAutosave', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('連続入力は最後の 1 回へ集約', async () => {
    const save = vi.fn(async () => {});
    const a = createAutosave(save);
    a.schedule(300);
    await vi.advanceTimersByTimeAsync(200);
    a.schedule(300);
    await vi.advanceTimersByTimeAsync(299);
    expect(save).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it('保存中の変更は完了後に 1 回再保存（並行実行なし）', async () => {
    const gates = [deferred(), deferred()];
    let active = 0;
    let peak = 0;
    let calls = 0;
    const a = createAutosave(async () => {
      active++;
      peak = Math.max(peak, active);
      await gates[calls++].promise;
      active--;
    });
    a.schedule();
    await vi.advanceTimersByTimeAsync(0);
    a.schedule();
    a.schedule();
    await vi.advanceTimersByTimeAsync(0);
    expect(calls).toBe(1);
    gates[0].resolve();
    await vi.advanceTimersByTimeAsync(0);
    expect(calls).toBe(2);
    gates[1].resolve();
    await vi.advanceTimersByTimeAsync(0);
    expect(peak).toBe(1);
  });

  it('flush：待機中の保存を即時実行', async () => {
    const save = vi.fn(async () => {});
    const a = createAutosave(save);
    a.schedule(600);
    await a.flush();
    expect(save).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it('flush：待機なしの場合は保存なし', async () => {
    const save = vi.fn(async () => {});
    await createAutosave(save).flush();
    expect(save).not.toHaveBeenCalled();
  });

  it('保存失敗後も次回の変更で再実行', async () => {
    const save = vi.fn().mockRejectedValueOnce(new Error('x')).mockResolvedValue(undefined);
    const a = createAutosave(save);
    a.schedule();
    await vi.advanceTimersByTimeAsync(0);
    a.schedule();
    await vi.advanceTimersByTimeAsync(0);
    expect(save).toHaveBeenCalledTimes(2);
  });
});
