import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/svelte';

const model = (id: string, extra: Record<string, unknown> = {}) => ({
  id,
  label: `Model ${id}`,
  note: 'note',
  file: `${id}.gguf`,
  url: `https://huggingface.co/x/${id}.gguf`,
  size: 2 * 1_073_741_824,
  sha256: '0'.repeat(64),
  license: 'Apache-2.0',
  license_url: 'https://example.com',
  style: 'chat_template',
  installed: false,
  partial_bytes: 0,
  downloading: false,
  ...extra
});

const ipc = vi.hoisted(() => ({
  listLlmModels: vi.fn(),
  installLlmModel: vi.fn(),
  cancelLlmDownload: vi.fn(),
  deleteLlmModel: vi.fn(),
  selectLlmModel: vi.fn(),
  onLlmProgress: vi.fn(async () => () => {}),
  listOrphanLlmFiles: vi.fn(async () => [] as { name: string; size: number }[]),
  deleteOrphanLlmFile: vi.fn()
}));
vi.mock('$lib/ipc', () => ipc);

import LocalModels from './LocalModels.svelte';

describe('<LocalModels>', () => {
  it('lists models and marks which role each one serves', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      assistant: 'c',
      dir: '/data/llm',
      models: [
        model('a', { installed: true, style: 'pfml3' }),
        model('b', { partial_bytes: 5 }),
        model('c', { installed: true })
      ]
    });
    const { findByText, getByText, queryByText } = render(LocalModels);
    expect(await findByText('Model a')).toBeInTheDocument();
    expect(getByText('翻訳')).toBeInTheDocument();
    expect(getByText('説明・要約')).toBeInTheDocument();
    // The chat model can also take over translation; PLaMo cannot assist.
    expect(getByText('翻訳用に設定')).toBeInTheDocument();
    expect(queryByText('説明用に設定')).not.toBeInTheDocument();
    // Interrupted download offers to resume.
    expect(getByText('再開')).toBeInTheDocument();
    expect(getByText('/data/llm')).toBeInTheDocument();
  });

  it('puts a downloaded translation model to work when none is installed', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'x',
      assistant: 'y',
      dir: '',
      models: [model('a', { style: 'pfml3' })]
    });
    ipc.installLlmModel.mockResolvedValue('/data/llm/a.gguf');
    ipc.selectLlmModel.mockResolvedValue(undefined);
    const { findByText } = render(LocalModels);
    await fireEvent.click(await findByText('ダウンロード'));
    await waitFor(() => expect(ipc.installLlmModel).toHaveBeenCalledWith('a'));
    await waitFor(() => expect(ipc.selectLlmModel).toHaveBeenCalledWith('a', 'translate'));
  });
});

describe('<LocalModels> housekeeping', () => {
  it('asks once more before deleting a model', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      assistant: 'c',
      dir: '/data/llm',
      models: [model('a', { installed: true, style: 'pfml3' })]
    });
    ipc.deleteLlmModel.mockResolvedValue(undefined);
    const { findByLabelText, findByText } = render(LocalModels);
    await fireEvent.click(await findByLabelText('Model a を削除'));
    expect(ipc.deleteLlmModel).not.toHaveBeenCalled();
    // The model is in use, and the confirmation says so.
    await fireEvent.click(await findByText('使用中。削除確定'));
    await waitFor(() => expect(ipc.deleteLlmModel).toHaveBeenCalledWith('a'));
  });

  it('lists files left by older versions and deletes them', async () => {
    ipc.listLlmModels.mockResolvedValue({ selected: 'a', assistant: 'c', dir: '/d', models: [] });
    ipc.listOrphanLlmFiles.mockResolvedValue([{ name: 'old-Q4.gguf', size: 1_073_741_824 }]);
    ipc.deleteOrphanLlmFile.mockResolvedValue(undefined);
    const { findByLabelText } = render(LocalModels);
    await fireEvent.click(await findByLabelText('old-Q4.gguf を削除'));
    await waitFor(() => expect(ipc.deleteOrphanLlmFile).toHaveBeenCalledWith('old-Q4.gguf'));
  });

  it('shows coded backend errors as plain text', async () => {
    ipc.listOrphanLlmFiles.mockResolvedValue([]);
    ipc.listLlmModels.mockResolvedValue({
      selected: 'x',
      assistant: 'y',
      dir: '/d',
      models: [model('x', { style: 'pfml3' })]
    });
    ipc.installLlmModel.mockRejectedValue('disk_full|空き容量不足：必要 2.3 GB／空き 1.0 GB');
    const { findByText, getByText } = render(LocalModels);
    await fireEvent.click(await findByText('ダウンロード'));
    await waitFor(() => expect(getByText('空き容量不足：必要 2.3 GB／空き 1.0 GB')).toBeInTheDocument());
  });
});
