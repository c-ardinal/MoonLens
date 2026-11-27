import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor, within } from '@testing-library/svelte';

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
  onAsrProgress: vi.fn(async () => () => {}),
  getSystemProfile: vi.fn(async () => ({
    profile: {},
    accelerator: null,
    recommended: {
      translate: { model: 't', reason: '' },
      assistant: { model: 'g', reason: '' },
      ocr: { model: 'ppocr', reason: '' },
      asr: { model: 'b', reason: '' }
    }
  }))
}));
vi.mock('$lib/ipc', () => ipc);

import AsrModels from './AsrModels.svelte';

type Found = { findByRole: (role: string, opts: { name: RegExp }) => Promise<HTMLElement> };
const radio = (r: Found, id: string) =>
  r.findByRole('radio', { name: new RegExp(`^Model ${id}`) });

describe('<AsrModels>', () => {
  it('shows the selection as a radio, the recommended mark and missing models', async () => {
    ipc.listAsrModels.mockResolvedValue({
      selected: 'a',
      dir: '/data/whisper',
      models: [model('a', { installed: true }), model('b', { installed: true, size: 190_085_487 }), model('c')]
    });
    ipc.selectAsrModel.mockResolvedValue(undefined);
    const r = render(AsrModels);
    expect(await radio(r, 'a')).toHaveAttribute('aria-checked', 'true');
    await waitFor(() => expect(within(r.getByRole('radio', { name: /^Model b/ })).getByText('推奨')).toBeInTheDocument());
    expect(r.getByText('181 MB')).toBeInTheDocument();
    expect(r.getAllByText('未取得')).toHaveLength(1);
    await fireEvent.click(r.getByRole('radio', { name: /^Model b/ }));
    await waitFor(() => expect(ipc.selectAsrModel).toHaveBeenCalledWith('b'));
  });

  it('downloads a missing model after asking, then puts it to work', async () => {
    ipc.listAsrModels.mockResolvedValue({
      selected: 'a',
      dir: '',
      models: [model('a', { installed: true }), model('b')]
    });
    ipc.installAsrModel.mockResolvedValue('/d/ggml-b.bin');
    ipc.selectAsrModel.mockResolvedValue(undefined);
    ipc.selectAsrModel.mockClear();
    const r = render(AsrModels);
    await fireEvent.click(await radio(r, 'b'));
    expect(ipc.installAsrModel).not.toHaveBeenCalled();
    await fireEvent.click(await r.findByText('取得・切替'));
    await waitFor(() => expect(ipc.installAsrModel).toHaveBeenCalledWith('b'));
    await waitFor(() => expect(ipc.selectAsrModel).toHaveBeenCalledWith('b'));
  });

  it('asks for a replacement before deleting the model in use', async () => {
    ipc.listAsrModels.mockResolvedValue({
      selected: 'a',
      dir: '',
      models: [model('a', { installed: true }), model('b', { installed: true })]
    });
    ipc.selectAsrModel.mockImplementation(async () => {
      ipc.listAsrModels.mockResolvedValue({
        selected: 'b',
        dir: '',
        models: [model('a', { installed: true }), model('b', { installed: true })]
      });
    });
    ipc.deleteAsrModel.mockResolvedValue(undefined);
    const r = render(AsrModels);
    await fireEvent.click(await r.findByLabelText('Model a を削除'));
    expect(ipc.deleteAsrModel).not.toHaveBeenCalled();
    await fireEvent.click(await r.findByText('切替・削除'));
    await waitFor(() => expect(ipc.selectAsrModel).toHaveBeenCalledWith('b'));
    await waitFor(() => expect(ipc.deleteAsrModel).toHaveBeenCalledWith('a'));
  });

  it('shows coded backend errors as plain text', async () => {
    ipc.listAsrModels.mockResolvedValue({ selected: 'a', dir: '', models: [model('a')] });
    ipc.installAsrModel.mockRejectedValue('disk_full|残容量不足：必要 1.0 GB／残容量 0.5 GB');
    const r = render(AsrModels);
    await fireEvent.click(await r.findByRole('button', { name: '取得' }));
    await waitFor(() => expect(r.getByText('残容量不足：必要 1.0 GB／残容量 0.5 GB')).toBeInTheDocument());
  });
});
