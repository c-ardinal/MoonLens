import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/svelte';

const ipc = vi.hoisted(() => ({
  listPlugins: vi.fn(),
  setPluginEnabled: vi.fn(),
  pickAndInstallPlugin: vi.fn(),
  checkPluginUpdates: vi.fn()
}));
vi.mock('$lib/ipc', () => ipc);

import Plugins from './Plugins.svelte';

const plugin = {
  id: 'sample',
  name: 'Sample',
  version: '1.0.0',
  author: 'a',
  license: 'MIT',
  kind: 'wasm',
  enabled: false,
  provides: ['asr'],
  permissions_summary: [],
  path: '/p'
};

describe('<Plugins>', () => {
  it('shows the empty state and hides the update check', async () => {
    ipc.listPlugins.mockResolvedValue([]);
    const { findByText, queryByText } = render(Plugins);
    expect(await findByText('拡張機能なし')).toBeInTheDocument();
    expect(queryByText('更新確認')).toBeNull();
  });

  it('enables a plugin', async () => {
    ipc.listPlugins.mockResolvedValue([plugin]);
    ipc.setPluginEnabled.mockResolvedValue(undefined);
    const { findByLabelText } = render(Plugins);
    await fireEvent.click(await findByLabelText('Sample：有効化'));
    await waitFor(() => expect(ipc.setPluginEnabled).toHaveBeenCalledWith('sample', true));
  });

  it('marks plugins that have an update', async () => {
    ipc.listPlugins.mockResolvedValue([plugin]);
    ipc.checkPluginUpdates.mockResolvedValue([
      { plugin_id: 'sample', installed_version: '1.0.0', latest_version: '1.1.0', download_url: null, notes: null }
    ]);
    const { findByText } = render(Plugins);
    await fireEvent.click(await findByText('更新確認'));
    expect(await findByText('更新 v1.1.0')).toBeInTheDocument();
  });
});
