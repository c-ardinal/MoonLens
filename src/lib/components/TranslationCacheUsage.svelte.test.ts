import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/svelte';

const ipc = vi.hoisted(() => ({
  translationCacheUsage: vi.fn(),
  clearTranslationCache: vi.fn()
}));
vi.mock('$lib/ipc', () => ipc);

import TranslationCacheUsage from './TranslationCacheUsage.svelte';

const empty = { bytes: 0, entries: 0, scopes: [] };

describe('<TranslationCacheUsage>', () => {
  it('shows the total and clears everything', async () => {
    ipc.translationCacheUsage
      .mockResolvedValueOnce({
        bytes: 2 * 1_048_576,
        entries: 1234,
        scopes: [{ scope: '', entries: 1234, bytes: 900_000 }]
      })
      .mockResolvedValue(empty);
    ipc.clearTranslationCache.mockResolvedValue(1234);
    const { findByText, getByLabelText, queryByText } = render(TranslationCacheUsage);
    expect(await findByText('1,234 件・2.0 MB')).toBeInTheDocument();
    // 共通分のみ：内訳行は非表示
    expect(queryByText(/内訳/)).toBeNull();
    await fireEvent.click(getByLabelText('訳文キャッシュを全消去'));
    await waitFor(() => expect(ipc.clearTranslationCache).toHaveBeenCalledWith(undefined));
    expect(await findByText('0 件・0 KB')).toBeInTheDocument();
    expect(getByLabelText('訳文キャッシュを全消去')).toBeDisabled();
  });

  it('lists apps and clears one of them', async () => {
    ipc.translationCacheUsage.mockResolvedValue({
      bytes: 4096,
      entries: 3,
      scopes: [
        { scope: 'game', entries: 2, bytes: 2048 },
        { scope: '', entries: 1, bytes: 10 }
      ]
    });
    ipc.clearTranslationCache.mockResolvedValue(2);
    const { findByText, getByLabelText } = render(TranslationCacheUsage);
    expect(await findByText('内訳：アプリ「game」')).toBeInTheDocument();
    expect(await findByText('内訳：共通（追従先アプリ未指定）')).toBeInTheDocument();
    await fireEvent.click(getByLabelText('アプリ「game」の訳文キャッシュを消去'));
    await waitFor(() => expect(ipc.clearTranslationCache).toHaveBeenCalledWith('game'));
  });
});
