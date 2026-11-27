import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/svelte';

const ipc = vi.hoisted(() => ({
  ocrScriptsStatus: vi.fn(),
  installOcrScript: vi.fn(),
  deleteOcrScript: vi.fn(),
  ocrScriptFile: (id: string) => `ppocrv5-rec-${id}.onnx`,
  onModelProgress: vi.fn(async () => () => {})
}));
vi.mock('$lib/ipc', () => ipc);

import OcrScripts from './OcrScripts.svelte';

const scripts = (korean: boolean) => [
  { id: 'korean', size: 13_418_787, installed: korean },
  { id: 'thai', size: 7_891_015, installed: false }
];

describe('<OcrScripts>', () => {
  it('fetches the model of a script on request', async () => {
    ipc.ocrScriptsStatus.mockResolvedValueOnce(scripts(false)).mockResolvedValue(scripts(true));
    ipc.installOcrScript.mockResolvedValue(undefined);
    const r = render(OcrScripts, { ready: true });
    await fireEvent.click(await r.findByRole('button', { name: '韓国語 を取得' }));
    await waitFor(() => expect(ipc.installOcrScript).toHaveBeenCalledWith('korean'));
    // 取得後：削除ボタンへ切替、他の文字体系は未取得の状態を維持。
    expect(await r.findByRole('button', { name: '韓国語 を削除' })).toBeInTheDocument();
    expect(r.getByRole('button', { name: 'タイ語 を取得' })).toBeInTheDocument();
    expect(r.getByText('13 MB')).toBeInTheDocument();
  });

  it('deletes a model only after a second press', async () => {
    ipc.ocrScriptsStatus.mockResolvedValueOnce(scripts(true)).mockResolvedValue(scripts(false));
    ipc.deleteOcrScript.mockResolvedValue(undefined);
    const r = render(OcrScripts, { ready: true });
    await fireEvent.click(await r.findByRole('button', { name: '韓国語 を削除' }));
    expect(ipc.deleteOcrScript).not.toHaveBeenCalled();
    await fireEvent.click(r.getByRole('button', { name: '削除確定' }));
    await waitFor(() => expect(ipc.deleteOcrScript).toHaveBeenCalledWith('korean'));
    expect(await r.findByRole('button', { name: '韓国語 を取得' })).toBeInTheDocument();
  });

  it('offers nothing to fetch while the standard models are missing', async () => {
    ipc.ocrScriptsStatus.mockResolvedValue(scripts(false));
    const r = render(OcrScripts, { ready: false });
    expect(await r.findByRole('button', { name: '韓国語 を取得' })).toBeDisabled();
  });

  it('shows why a download failed', async () => {
    ipc.ocrScriptsStatus.mockResolvedValue(scripts(false));
    ipc.installOcrScript.mockRejectedValue('network|接続失敗');
    const r = render(OcrScripts, { ready: true });
    await fireEvent.click(await r.findByRole('button', { name: 'タイ語 を取得' }));
    expect(await r.findByText(/接続失敗/)).toBeInTheDocument();
    expect(r.getByRole('button', { name: 'タイ語 を取得' })).toBeInTheDocument();
  });
});
