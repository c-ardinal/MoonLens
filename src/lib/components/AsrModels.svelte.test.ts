import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/svelte';

const model = (id: string, extra: Record<string, unknown> = {}) => ({
  id,
  label: `Model ${id}`,
  note: 'note',
  file: `ggml-${id}.bin`,
  url: `https://huggingface.co/x/ggml-${id}.bin`,
  size: 874_188_075,
  sha256: '0'.repeat(64),
  license: 'MIT',
  license_url: 'https://example.com',
  installed: false,
  partial_bytes: 0,
  downloading: false,
  ...extra
});

const ipc = vi.hoisted(() => ({
  listAsrModels: vi.fn(),
  installAsrModel: vi.fn(),
  cancelAsrDownload: vi.fn(),
  deleteAsrModel: vi.fn(),
  selectAsrModel: vi.fn(),
  onAsrProgress: vi.fn(async () => () => {})
}));
vi.mock('$lib/ipc', () => ipc);

import AsrModels from './AsrModels.svelte';

describe('<AsrModels>', () => {
  it('lists models, marks the one in use and offers to switch', async () => {
    ipc.listAsrModels.mockResolvedValue({
      selected: 'a',
      dir: '/data/whisper',
      models: [
        model('a', { installed: true }),
        model('b', { installed: true, size: 190_085_487 }),
        model('c', { partial_bytes: 5 })
      ]
    });
    const { findByText, getByText, getAllByText } = render(AsrModels);
    expect(await findByText('Model a')).toBeInTheDocument();
    expect(getByText('使用中')).toBeInTheDocument();
    expect(getAllByText('使用')).toHaveLength(1);
    // Sizes under a gigabyte read in MB.
    expect(getByText(/181 MB/)).toBeInTheDocument();
    expect(getByText('再開')).toBeInTheDocument();
    expect(getByText('/data/whisper')).toBeInTheDocument();
  });

  it('puts the first downloaded model to work', async () => {
    ipc.listAsrModels.mockResolvedValue({ selected: 'a', dir: '', models: [model('b')] });
    ipc.installAsrModel.mockResolvedValue('/data/whisper/ggml-b.bin');
    ipc.selectAsrModel.mockResolvedValue(undefined);
    const { findByText } = render(AsrModels);
    await fireEvent.click(await findByText('ダウンロード'));
    await waitFor(() => expect(ipc.installAsrModel).toHaveBeenCalledWith('b'));
    await waitFor(() => expect(ipc.selectAsrModel).toHaveBeenCalledWith('b'));
  });

  it('asks once more before deleting the model in use', async () => {
    ipc.listAsrModels.mockResolvedValue({
      selected: 'a',
      dir: '',
      models: [model('a', { installed: true })]
    });
    ipc.deleteAsrModel.mockResolvedValue(undefined);
    const { findByLabelText, findByText } = render(AsrModels);
    await fireEvent.click(await findByLabelText('Model a を削除'));
    expect(ipc.deleteAsrModel).not.toHaveBeenCalled();
    await fireEvent.click(await findByText('使用中。削除確定'));
    await waitFor(() => expect(ipc.deleteAsrModel).toHaveBeenCalledWith('a'));
  });

  it('shows coded backend errors as plain text', async () => {
    ipc.listAsrModels.mockResolvedValue({ selected: 'x', dir: '', models: [model('x')] });
    ipc.installAsrModel.mockRejectedValue('disk_full|空き容量不足：必要 1.4 GB／空き 1.0 GB');
    const { findByText, getByText } = render(AsrModels);
    await fireEvent.click(await findByText('ダウンロード'));
    await waitFor(() =>
      expect(getByText('空き容量不足：必要 1.4 GB／空き 1.0 GB')).toBeInTheDocument()
    );
  });
});
