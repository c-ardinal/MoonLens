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
  onLlmProgress: vi.fn(async () => () => {})
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
    expect(getByText('翻訳に使う')).toBeInTheDocument();
    expect(queryByText('説明・要約に使う')).not.toBeInTheDocument();
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
