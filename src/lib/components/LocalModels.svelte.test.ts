import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor, within } from '@testing-library/svelte';

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
  deleteOrphanLlmFile: vi.fn(),
  acceptLicense: vi.fn(),
  getSystemProfile: vi.fn(async () => ({
    profile: {},
    accelerator: null,
    recommended: {
      translate: { model: 'b', reason: '' },
      assistant: { model: 'd', reason: '' },
      ocr: { model: 'ppocr', reason: '' },
      asr: { model: 'w', reason: '' }
    }
  }))
}));
vi.mock('$lib/ipc', () => ipc);

import LocalModels from './LocalModels.svelte';

type Found = { findByRole: (role: string, opts: { name: RegExp }) => Promise<HTMLElement> };
const radio = (r: Found, name: string) =>
  r.findByRole('radio', { name: new RegExp(`^Model ${name}`) });

describe('<LocalModels>', () => {
  it('shows one selected radio per role, the recommended mark and missing models', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      assistant: 'c',
      dir: '/data/llm',
      models: [
        model('a', { installed: true, style: 'pfml3' }),
        model('b', { partial_bytes: 5, style: 'pfml3' }),
        model('c', { installed: true })
      ]
    });
    const r = render(LocalModels, { role: 'translate' });
    expect(await radio(r, 'a')).toHaveAttribute('aria-checked', 'true');
    expect(r.getAllByRole('radio').filter((x) => x.getAttribute('aria-checked') === 'true')).toHaveLength(1);
    // Recommended for this PC (system profile): model b.
    await waitFor(() => expect(within(r.getByRole('radio', { name: /^Model b/ })).getByText('推奨')).toBeInTheDocument());
    expect(r.getAllByText('未取得')).toHaveLength(1);
  });

  it('switches to an installed model at once, explain / summarize offers chat models only', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      assistant: 'c',
      dir: '/data/llm',
      models: [model('a', { installed: true, style: 'pfml3' }), model('c', { installed: true }), model('d', { installed: true })]
    });
    ipc.selectLlmModel.mockResolvedValue(undefined);
    const r = render(LocalModels, { role: 'assistant' });
    await fireEvent.click(await radio(r, 'd'));
    // PLaMo cannot assist.
    expect(r.queryByRole('radio', { name: /^Model a/ })).not.toBeInTheDocument();
    await waitFor(() => expect(ipc.selectLlmModel).toHaveBeenCalledWith('d', 'assistant'));
  });

  it('asks before downloading a missing model, then switches to it', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      assistant: 'y',
      dir: '',
      models: [model('a', { installed: true, style: 'pfml3' }), model('b', { style: 'pfml3' })]
    });
    ipc.installLlmModel.mockResolvedValue('/data/llm/b.gguf');
    ipc.selectLlmModel.mockResolvedValue(undefined);
    ipc.installLlmModel.mockClear();
    ipc.selectLlmModel.mockClear();
    const r = render(LocalModels);
    await fireEvent.click(await radio(r, 'b'));
    expect(ipc.installLlmModel).not.toHaveBeenCalled();
    expect(ipc.selectLlmModel).not.toHaveBeenCalled();
    await fireEvent.click(await r.findByText('取得・切替'));
    await waitFor(() => expect(ipc.installLlmModel).toHaveBeenCalledWith('b'));
    await waitFor(() => expect(ipc.selectLlmModel).toHaveBeenCalledWith('b', 'translate'));
  });

  it('prompts to download the selected model when it is missing', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'x',
      assistant: 'y',
      dir: '',
      models: [model('x', { style: 'pfml3' })]
    });
    ipc.installLlmModel.mockResolvedValue('/data/llm/x.gguf');
    ipc.installLlmModel.mockClear();
    const r = render(LocalModels);
    expect(await r.findByText('選択中モデル未取得。翻訳に必要')).toBeInTheDocument();
    await fireEvent.click(r.getByRole('button', { name: '取得' }));
    await waitFor(() => expect(ipc.installLlmModel).toHaveBeenCalledWith('x'));
  });
});

describe('<LocalModels> shared state', () => {
  it('shows a model downloaded in one panel as installed in the other', async () => {
    const before = { selected: 'a', assistant: 'x', dir: '', models: [model('a', { installed: true, style: 'pfml3' }), model('c')] };
    const after = { ...before, assistant: 'c', models: [before.models[0], model('c', { installed: true })] };
    ipc.listLlmModels.mockResolvedValue(before);
    ipc.installLlmModel.mockImplementation(async () => {
      ipc.listLlmModels.mockResolvedValue(after);
      return '/d/c.gguf';
    });
    ipc.selectLlmModel.mockResolvedValue(undefined);
    const tr = render(LocalModels, { role: 'translate' });
    const as = render(LocalModels, { role: 'assistant' });
    const trPanel = within(tr.container);
    await fireEvent.click(await trPanel.findByRole('radio', { name: /^Model c/ }));
    await fireEvent.click(await trPanel.findByText('取得・切替'));
    const asPanel = within(as.container);
    await waitFor(() => expect(asPanel.getByRole('radio', { name: /^Model c/ })).toHaveAttribute('aria-checked', 'true'));
    expect(asPanel.queryByText('未取得')).not.toBeInTheDocument();
  });
});

describe('<LocalModels> housekeeping', () => {
  it('asks once more before deleting a model not in use', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      assistant: 'c',
      dir: '/data/llm',
      models: [model('a', { installed: true, style: 'pfml3' }), model('b', { installed: true, style: 'pfml3' })]
    });
    ipc.deleteLlmModel.mockResolvedValue(undefined);
    ipc.deleteLlmModel.mockClear();
    const r = render(LocalModels);
    await fireEvent.click(await r.findByLabelText('Model b を削除'));
    expect(ipc.deleteLlmModel).not.toHaveBeenCalled();
    await fireEvent.click(await r.findByText('削除確定'));
    await waitFor(() => expect(ipc.deleteLlmModel).toHaveBeenCalledWith('b'));
  });

  it('asks for a replacement before deleting the model in use', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      assistant: 'c',
      dir: '/data/llm',
      models: [model('a', { installed: true, style: 'pfml3' }), model('b', { style: 'pfml3' })]
    });
    ipc.installLlmModel.mockImplementation(async () => {
      ipc.listLlmModels.mockResolvedValue({
        selected: 'b',
        assistant: 'c',
        dir: '/data/llm',
        models: [model('a', { installed: true, style: 'pfml3' }), model('b', { installed: true, style: 'pfml3' })]
      });
      return '/data/llm/b.gguf';
    });
    ipc.selectLlmModel.mockResolvedValue(undefined);
    ipc.deleteLlmModel.mockResolvedValue(undefined);
    ipc.deleteLlmModel.mockClear();
    const r = render(LocalModels);
    await fireEvent.click(await r.findByLabelText('Model a を削除'));
    expect(await r.findByText('使用中モデル。削除前に代替モデル選択')).toBeInTheDocument();
    // The recommended model is offered first.
    const alts = within(r.getByRole('radiogroup', { name: '代替モデル' })).getAllByRole('radio');
    expect(alts[0]).toHaveAttribute('aria-checked', 'true');
    await fireEvent.click(r.getByText('切替・削除'));
    await waitFor(() => expect(ipc.installLlmModel).toHaveBeenCalledWith('b'));
    await waitFor(() => expect(ipc.selectLlmModel).toHaveBeenCalledWith('b', 'translate'));
    await waitFor(() => expect(ipc.deleteLlmModel).toHaveBeenCalledWith('a'));
  });

  it('keeps a model the other role uses', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'a',
      assistant: 'c',
      dir: '/data/llm',
      models: [model('a', { installed: true, style: 'pfml3' }), model('c', { installed: true })]
    });
    ipc.deleteLlmModel.mockClear();
    const r = render(LocalModels, { role: 'translate' });
    await fireEvent.click(await r.findByLabelText('Model c を削除'));
    expect(await r.findByText('説明・要約で使用中。説明・要約の選択変更後に削除可能')).toBeInTheDocument();
    expect(ipc.deleteLlmModel).not.toHaveBeenCalled();
  });

  it('shows coded backend errors as plain text', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'x',
      assistant: 'y',
      dir: '/d',
      models: [model('x', { style: 'pfml3' })]
    });
    ipc.installLlmModel.mockRejectedValue('disk_full|残容量不足：必要 2.3 GB／残容量 1.0 GB');
    const r = render(LocalModels);
    await fireEvent.click(await r.findByRole('button', { name: '取得' }));
    await waitFor(() => expect(r.getByText('残容量不足：必要 2.3 GB／残容量 1.0 GB')).toBeInTheDocument());
  });
});

describe('<LocalModels> licence consent', () => {
  const gated = (accepted: string[]) => ({
    selected: 'a',
    assistant: 'y',
    dir: '',
    accepted_licenses: accepted,
    models: [
      model('a', { installed: true, style: 'pfml3', consent: 'plamo-community' }),
      model('b', { style: 'pfml3', consent: 'plamo-community' })
    ]
  });

  it('shows the consent block first and installs only after agreeing', async () => {
    ipc.listLlmModels.mockResolvedValue(gated([]));
    ipc.installLlmModel.mockResolvedValue('/d/b.gguf');
    ipc.selectLlmModel.mockResolvedValue(undefined);
    ipc.acceptLicense.mockResolvedValue(undefined);
    ipc.installLlmModel.mockClear();
    ipc.acceptLicense.mockClear();
    const r = render(LocalModels);
    await fireEvent.click(await radio(r, 'b'));
    await fireEvent.click(await r.findByText('取得・切替'));
    expect(await r.findByText('要点（要約）')).toBeInTheDocument();
    expect(ipc.installLlmModel).not.toHaveBeenCalled();
    const agree = r.getByRole('button', { name: '同意・取得' });
    expect(agree).toBeDisabled();
    await fireEvent.click(r.getByRole('checkbox'));
    await fireEvent.click(agree);
    await waitFor(() => expect(ipc.installLlmModel).toHaveBeenCalledWith('b'));
    expect(ipc.acceptLicense).toHaveBeenCalledWith('plamo-community');
    await waitFor(() => expect(ipc.selectLlmModel).toHaveBeenCalledWith('b', 'translate'));
  });

  it('installs nothing when the consent is cancelled', async () => {
    ipc.listLlmModels.mockResolvedValue(gated([]));
    ipc.installLlmModel.mockClear();
    const r = render(LocalModels);
    await fireEvent.click(await radio(r, 'b'));
    await fireEvent.click(await r.findByText('取得・切替'));
    await fireEvent.click(await r.findByRole('button', { name: '中止' }));
    expect(r.queryByText('要点（要約）')).not.toBeInTheDocument();
    expect(ipc.installLlmModel).not.toHaveBeenCalled();
  });

  it('does not ask again once the licence is accepted', async () => {
    ipc.listLlmModels.mockResolvedValue(gated(['plamo-community']));
    ipc.installLlmModel.mockResolvedValue('/d/b.gguf');
    ipc.selectLlmModel.mockResolvedValue(undefined);
    ipc.installLlmModel.mockClear();
    const r = render(LocalModels);
    await fireEvent.click(await radio(r, 'b'));
    await fireEvent.click(await r.findByText('取得・切替'));
    await waitFor(() => expect(ipc.installLlmModel).toHaveBeenCalledWith('b'));
    expect(r.queryByText('要点（要約）')).not.toBeInTheDocument();
  });

  it('asks before the replacement download of a delete', async () => {
    ipc.listLlmModels.mockResolvedValue(gated([]));
    ipc.installLlmModel.mockClear();
    ipc.deleteLlmModel.mockClear();
    const r = render(LocalModels);
    await fireEvent.click(await r.findByLabelText('Model a を削除'));
    await fireEvent.click(await r.findByText('切替・削除'));
    expect(await r.findByText('要点（要約）')).toBeInTheDocument();
    expect(ipc.installLlmModel).not.toHaveBeenCalled();
    expect(ipc.deleteLlmModel).not.toHaveBeenCalled();
  });

  it('asks before resuming the selected model that is missing', async () => {
    ipc.listLlmModels.mockResolvedValue({
      selected: 'x',
      assistant: 'y',
      dir: '',
      accepted_licenses: [],
      models: [model('x', { style: 'pfml3', consent: 'plamo-community', partial_bytes: 10 })]
    });
    ipc.installLlmModel.mockClear();
    const r = render(LocalModels);
    await fireEvent.click(await r.findByRole('button', { name: '取得再開' }));
    expect(await r.findByText('要点（要約）')).toBeInTheDocument();
    expect(ipc.installLlmModel).not.toHaveBeenCalled();
  });
});
