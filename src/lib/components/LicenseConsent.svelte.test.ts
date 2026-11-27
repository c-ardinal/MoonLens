import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/svelte';

const ipc = vi.hoisted(() => ({ acceptLicense: vi.fn() }));
vi.mock('$lib/ipc', () => ipc);

import LicenseConsent from './LicenseConsent.svelte';

describe('<LicenseConsent>', () => {
  it('keeps agree disabled until the box is checked, then accepts and continues', async () => {
    ipc.acceptLicense.mockResolvedValue(undefined);
    const onaccepted = vi.fn();
    const r = render(LicenseConsent, { license: 'plamo-community', onaccepted, oncancel: vi.fn() });
    const agree = r.getByRole('button', { name: '同意・取得' });
    expect(agree).toBeDisabled();
    await fireEvent.click(r.getByRole('checkbox'));
    expect(agree).toBeEnabled();
    expect(ipc.acceptLicense).not.toHaveBeenCalled();
    await fireEvent.click(agree);
    await waitFor(() => expect(onaccepted).toHaveBeenCalledOnce());
    expect(ipc.acceptLicense).toHaveBeenCalledWith('plamo-community');
  });

  it('labels the points as a summary and links the full text', () => {
    const r = render(LicenseConsent, { license: 'plamo-community', onaccepted: vi.fn(), oncancel: vi.fn() });
    expect(r.getByText('要点（要約）')).toBeInTheDocument();
    expect(r.getByRole('link', { name: '契約全文' })).toHaveAttribute(
      'href',
      'https://plamo.preferredai.jp/info/plamo-community-license-ja'
    );
  });

  it('does not continue when saving the acceptance fails', async () => {
    ipc.acceptLicense.mockRejectedValue('error|保存失敗');
    const onaccepted = vi.fn();
    const r = render(LicenseConsent, { license: 'plamo-community', onaccepted, oncancel: vi.fn() });
    await fireEvent.click(r.getByRole('checkbox'));
    await fireEvent.click(r.getByRole('button', { name: '同意・取得' }));
    expect(await r.findByRole('alert')).toHaveTextContent('保存失敗');
    expect(onaccepted).not.toHaveBeenCalled();
  });

  it('cancels without accepting', async () => {
    ipc.acceptLicense.mockClear();
    const oncancel = vi.fn();
    const r = render(LicenseConsent, { license: 'plamo-community', onaccepted: vi.fn(), oncancel });
    await fireEvent.click(r.getByRole('button', { name: '中止' }));
    expect(oncancel).toHaveBeenCalledOnce();
    expect(ipc.acceptLicense).not.toHaveBeenCalled();
  });
});
