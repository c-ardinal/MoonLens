import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import HotkeyFailureToast from './HotkeyFailureToast.svelte';
import type { HotkeyRegistrationReport } from '$lib/ipc';

// Hold the last handler passed to onHotkeyFailure so tests can fire
// synthetic events without going through Tauri.
let lastHandler: ((report: HotkeyRegistrationReport) => void) | null = null;

vi.mock('$lib/ipc', () => ({
  onHotkeyFailure: (handler: (report: HotkeyRegistrationReport) => void) => {
    lastHandler = handler;
    return Promise.resolve(() => {
      lastHandler = null;
    });
  }
}));

const gotoSpy = vi.fn();
vi.mock('$lib/navigation', () => ({
  goto: (path: string) => gotoSpy(path)
}));

describe('<HotkeyFailureToast>', () => {
  beforeEach(() => {
    lastHandler = null;
    gotoSpy.mockReset();
  });

  it('renders nothing when no failures have been reported', () => {
    const { container } = render(HotkeyFailureToast);
    expect(container.querySelector('[role="alert"]')).toBeNull();
  });

  it('renders a toast listing each failed shortcut when the backend reports failures', async () => {
    const { findByRole, findByText } = render(HotkeyFailureToast);

    // Wait a tick so the onMount-registered listener is in place.
    await Promise.resolve();
    expect(lastHandler).not.toBeNull();

    lastHandler?.({
      failed: [
        { id: 'capture_region', accelerator: 'Ctrl+Shift+Space', error: 'already taken' },
        { id: 'toggle_subtitle', accelerator: 'Ctrl+Shift+S', error: 'conflict' }
      ]
    });

    const alert = await findByRole('alert');
    expect(alert).toBeInTheDocument();
    expect(await findByText(/capture_region/)).toBeInTheDocument();
    expect(await findByText(/Ctrl\+Shift\+Space/)).toBeInTheDocument();
    expect(await findByText(/toggle_subtitle/)).toBeInTheDocument();
  });

  it('dismisses the toast when the close button is clicked', async () => {
    const { findByRole, queryByRole, findByLabelText } = render(HotkeyFailureToast);
    await Promise.resolve();

    lastHandler?.({
      failed: [{ id: 'capture_region', accelerator: 'Ctrl+Shift+Space', error: 'taken' }]
    });

    await findByRole('alert');
    const closeBtn = await findByLabelText('閉じる');
    await fireEvent.click(closeBtn);

    expect(queryByRole('alert')).toBeNull();
  });

  it('navigates to the settings page when the rebind button is clicked', async () => {
    const { findByRole } = render(HotkeyFailureToast);
    await Promise.resolve();

    lastHandler?.({
      failed: [{ id: 'capture_region', accelerator: 'Ctrl+Shift+Space', error: 'taken' }]
    });

    const rebindBtn = await findByRole('button', { name: /設定/ });
    await fireEvent.click(rebindBtn);

    expect(gotoSpy).toHaveBeenCalledWith('/settings/');
  });

  it('ignores empty failure reports', async () => {
    const { container } = render(HotkeyFailureToast);
    await Promise.resolve();

    lastHandler?.({ failed: [] });

    expect(container.querySelector('[role="alert"]')).toBeNull();
  });
});
