import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MAX_TOASTS, TOAST_MS, close, copyText, hold, release, show, toasts, undoable } from './toast.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  for (const t of [...toasts.items]) close(t.id, 'dismiss');
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

  it('hold 中は終了せず、全要因の release 後に計測を再開', () => {
    const id = show({ text: 'a' });
    hold(id, 'pointer');
    hold(id, 'focus');
    release(id, 'pointer');
    vi.advanceTimersByTime(TOAST_MS.info * 4);
    expect(texts()).toEqual(['a']);
    release(id, 'focus');
    vi.advanceTimersByTime(TOAST_MS.info - 1);
    expect(texts()).toEqual(['a']);
    vi.advanceTimersByTime(1);
    expect(texts()).toEqual([]);
  });
});

describe('undoable', () => {
  it('取消：undo を 1 回実行', () => {
    const undo = vi.fn(async () => {});
    const id = undoable({ text: '削除完了', failure: '復元失敗', undo });
    expect(toasts.items.at(-1)).toMatchObject({ text: '削除完了', action: '取消' });
    close(id, 'action');
    close(id, 'action');
    expect(undo).toHaveBeenCalledOnce();
  });

  it('満了・閉鎖：undo なし', () => {
    for (const end of [() => vi.advanceTimersByTime(TOAST_MS.action), (id: number) => close(id, 'dismiss')]) {
      const undo = vi.fn(async () => {});
      end(undoable({ text: '削除完了', failure: '復元失敗', undo }));
      expect(undo).not.toHaveBeenCalled();
    }
  });

  it('取消失敗：失敗を通知', async () => {
    const id = undoable({ text: '削除完了', failure: '復元失敗', undo: async () => Promise.reject(new Error('db')) });
    close(id, 'action');
    await vi.waitFor(() => expect(toasts.items.at(-1)).toMatchObject({ kind: 'error', text: expect.stringMatching(/^復元失敗：.*db/) }));
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
    expect(toasts.items.at(-1)).toMatchObject({ kind: 'error', text: expect.stringMatching(/^コピー失敗：.*denied/) });
  });
});
