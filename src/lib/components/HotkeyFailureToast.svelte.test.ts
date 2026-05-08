import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import HotkeyFailureToast from './HotkeyFailureToast.svelte';
import type { HotkeyRegistrationReport } from '$lib/ipc';

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

  it('renders a toast with friendly labels and accelerators when failures arrive', async () => {
    const { findByRole, findByText } = render(HotkeyFailureToast);
    await Promise.resolve();
    expect(lastHandler).not.toBeNull();

    lastHandler?.({
      failed: [
        { id: 'capture_region', accelerator: 'Ctrl+Shift+Space', error: 'taken' },
        { id: 'toggle_subtitle', accelerator: 'Ctrl+Shift+S', error: 'conflict' }
      ]
    });

    const alert = await findByRole('alert');
    expect(alert).toBeInTheDocument();
    // The new toast renders the localised label so the user knows
    // *which* hotkey is broken without consulting docs.
    expect(await findByText(/範囲キャプチャ/)).toBeInTheDocument();
    expect(await findByText(/音声字幕の開始/)).toBeInTheDocument();
    expect(await findByText(/Ctrl\+Shift\+Space/)).toBeInTheDocument();
  });

  it('dismisses the toast when the close button is clicked', async () => {
    const { findByRole, queryByRole, findAllByLabelText } = render(HotkeyFailureToast);
    await Promise.resolve();

    lastHandler?.({
      failed: [{ id: 'capture_region', accelerator: 'Ctrl+Shift+Space', error: 'taken' }]
    });

    await findByRole('alert');
    // Two elements share the "閉じる" label: a top-right X button and
    // an inline "閉じる" text button. Click any of them.
    const closeBtns = await findAllByLabelText('閉じる');
    await fireEvent.click(closeBtns[0]);

    expect(queryByRole('alert')).toBeNull();
  });

  it('navigates to the settings page when the rebind button is clicked', async () => {
    const { findByRole } = render(HotkeyFailureToast);
    await Promise.resolve();

    lastHandler?.({
      failed: [{ id: 'capture_region', accelerator: 'Ctrl+Shift+Space', error: 'taken' }]
    });

    const rebindBtn = await findByRole('button', { name: /設定で変更/ });
    await fireEvent.click(rebindBtn);

    expect(gotoSpy).toHaveBeenCalledWith('/settings/');
  });

  it('ignores reports with no failed hotkeys', async () => {
    const { container } = render(HotkeyFailureToast);
    await Promise.resolve();

    lastHandler?.({ failed: [] });

    expect(container.querySelector('[role="alert"]')).toBeNull();
  });

  it('filters out empty / whitespace-only accelerators from the displayed list', async () => {
    const { container, queryByText } = render(HotkeyFailureToast);
    await Promise.resolve();

    // The whole report contains nothing meaningful — every entry has
    // an empty accelerator. The toast must not pop up.
    lastHandler?.({
      failed: [
        { id: 'capture_region', accelerator: '', error: 'cleared' },
        { id: 'open_history', accelerator: '   ', error: 'cleared' }
      ]
    });

    expect(container.querySelector('[role="alert"]')).toBeNull();
    expect(queryByText(/範囲キャプチャ/)).toBeNull();
  });

  it('an empty report after a non-empty one auto-dismisses the toast', async () => {
    const { container, findByRole, queryByRole } = render(HotkeyFailureToast);
    await Promise.resolve();

    lastHandler?.({
      failed: [{ id: 'capture_region', accelerator: 'Ctrl+Shift+Space', error: 'taken' }]
    });
    // findByRole waits for Svelte to flush the reactive update.
    await findByRole('alert');

    // Backend reports "all clear" — the toast should disappear without
    // requiring the user to dismiss manually.
    lastHandler?.({ failed: [] });
    // Allow the next microtask so the `visible = false` flush propagates.
    await Promise.resolve();
    await Promise.resolve();
    expect(queryByRole('alert')).toBeNull();
    void container; // eslint quiet
  });
});
