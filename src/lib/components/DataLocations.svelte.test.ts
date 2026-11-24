import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/svelte';

const ipc = vi.hoisted(() => ({
  dataLocations: vi.fn(),
  listOrphanLlmFiles: vi.fn(),
  deleteOrphanLlmFile: vi.fn()
}));
vi.mock('$lib/ipc', () => ipc);

import DataLocations from './DataLocations.svelte';

const info = {
  version: '0.1.0 (abc)',
  config_dir: '/c',
  data_dir: '/d',
  models_dir: '/m',
  log_dir: '/l',
  models_bytes: 2 * 1_073_741_824,
  data_bytes: 1_048_576
};

describe('<DataLocations>', () => {
  it('lists each location with its size', async () => {
    ipc.dataLocations.mockResolvedValue(info);
    ipc.listOrphanLlmFiles.mockResolvedValue([]);
    const { findByText, getByText } = render(DataLocations);
    expect(await findByText('0.1.0 (abc)')).toBeInTheDocument();
    expect(getByText('2.0 GB')).toBeInTheDocument();
    expect(getByText('1.0 MB')).toBeInTheDocument();
  });

  it('lists files left by older versions and deletes them', async () => {
    ipc.dataLocations.mockResolvedValue(info);
    ipc.listOrphanLlmFiles.mockResolvedValue([{ name: 'old-Q4.gguf', size: 1_073_741_824 }]);
    ipc.deleteOrphanLlmFile.mockResolvedValue(undefined);
    const { findByLabelText } = render(DataLocations);
    await fireEvent.click(await findByLabelText('old-Q4.gguf を削除'));
    await waitFor(() => expect(ipc.deleteOrphanLlmFile).toHaveBeenCalledWith('old-Q4.gguf'));
  });
});
