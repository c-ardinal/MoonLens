import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MAX_TOASTS, TOAST_MS, close, copyText, flush, hold, release, show, toasts, undoable } from './toast.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  flush();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

const texts = () => toasts.items.map((t) => t.text);

describe('toast', () => {
  it('表示時間の満了で終了し、理由 timeout を通知', () => {
    const onClose = vi.fn();
    show({ text: 'a', onClose });
    expect(texts()).toEqual(['a']);
    vi.advanceTimersByTime(TOAST_MS.info);
    expect(texts()).toEqual([]);
    expect(onClose).toHaveBeenCalledExactlyOnceWith('timeout');
  });

  it('終了理由は 1 回のみ確定', () => {
    const onClose = vi.fn();
    const id = show({ text: 'a', action: '取消', onClose });
    close(id, 'action');
    close(id, 'dismiss');
    vi.advanceTimersByTime(TOAST_MS.action);
    expect(onClose).toHaveBeenCalledExactlyOnceWith('action');
  });

  it('上限超過時は最古の通知を満了扱いで終了', () => {
    const onClose = vi.fn();
    show({ text: '0', onClose });
    for (let i = 1; i <= MAX_TOASTS; i++) show({ text: String(i) });
    expect(texts()).toEqual(['1', '2', '3']);
    expect(onClose).toHaveBeenCalledExactlyOnceWith('timeout');
  });

  it('hold 中は終了せず、release 後に計測を再開', () => {
    const id = show({ text: 'a' });
    hold(id);
    vi.advanceTimersByTime(TOAST_MS.info * 4);
    expect(texts()).toEqual(['a']);
    release(id);
    vi.advanceTimersByTime(TOAST_MS.info - 1);
    expect(texts()).toEqual(['a']);
    vi.advanceTimersByTime(1);
    expect(texts()).toEqual([]);
  });
});

describe('undoable', () => {
  const setup = (commit = vi.fn(async () => {})) => {
    const hidden = new Set<number>();
    const id = undoable({
      text: '削除完了',
      failure: '削除失敗',
      apply: () => hidden.add(1),
      revert: () => hidden.delete(1),
      commit
    });
    return { hidden, id, commit };
  };

  it('取消：復元のみ、確定なし', () => {
    const { hidden, id, commit } = setup();
    expect(hidden.has(1)).toBe(true);
    close(id, 'action');
    expect(hidden.has(1)).toBe(false);
    expect(commit).not.toHaveBeenCalled();
  });

  it('満了・閉鎖・flush：確定', () => {
    for (const end of [() => vi.advanceTimersByTime(TOAST_MS.action), (id: number) => close(id, 'dismiss'), flush]) {
      const { hidden, id, commit } = setup();
      end(id);
      expect(commit).toHaveBeenCalledOnce();
      expect(hidden.has(1)).toBe(true);
    }
  });

  it('確定失敗：復元し、失敗を通知', async () => {
    const { hidden } = setup(vi.fn(async () => Promise.reject(new Error('db'))));
    vi.advanceTimersByTime(TOAST_MS.action);
    await vi.waitFor(() => expect(hidden.has(1)).toBe(false));
    expect(toasts.items.at(-1)).toMatchObject({ kind: 'error', text: '削除失敗：Error: db' });
  });
});

describe('copyText', () => {
  it('成功・失敗を通知', async () => {
    const writeText = vi.fn(async () => {});
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    expect(await copyText('x', '訳文')).toBe(true);
    expect(writeText).toHaveBeenCalledWith('x');
    expect(toasts.items.at(-1)).toMatchObject({ kind: 'info', text: '訳文：コピー完了' });
    writeText.mockRejectedValueOnce('denied');
    expect(await copyText('x', '訳文')).toBe(false);
    expect(toasts.items.at(-1)).toMatchObject({ kind: 'error', text: 'コピー失敗：denied' });
  });
});
