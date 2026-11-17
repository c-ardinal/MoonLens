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
  it('lists models with size and marks the selected one', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      dir: '/data/llm',
      models: [model('a', { installed: true }), model('b', { partial_bytes: 5 })]
    });
    const { findByText, getByText } = render(LocalModels);
    expect(await findByText('Model a')).toBeInTheDocument();
    expect(getByText('使用中')).toBeInTheDocument();
    // Interrupted download offers to resume.
    expect(getByText('再開')).toBeInTheDocument();
    expect(getByText('/data/llm')).toBeInTheDocument();
  });

  it('downloads a model and selects it when nothing usable is selected', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      dir: '',
      models: [model('a')]
    });
    ipc.installLlmModel.mockResolvedValue('/data/llm/a.gguf');
    ipc.selectLlmModel.mockResolvedValue(undefined);
    const { findByText } = render(LocalModels);
    await fireEvent.click(await findByText('ダウンロード'));
    await waitFor(() => expect(ipc.installLlmModel).toHaveBeenCalledWith('a'));
    await waitFor(() => expect(ipc.selectLlmModel).toHaveBeenCalledWith('a'));
  });
});
