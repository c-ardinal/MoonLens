import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import HotkeyInput from './HotkeyInput.svelte';

/**
 * HotkeyInput exposes a record mode: after the user clicks the
 * capture button, the next non-modifier keystroke with at least one
 * modifier is converted into a Tauri accelerator string (e.g.
 * "Ctrl+Shift+P") and returned via `onChange`.
 */

describe('<HotkeyInput>', () => {
  it('renders the placeholder when value is empty', () => {
    const { getByText } = render(HotkeyInput, {
      props: { value: '', onChange: () => {} }
    });
    expect(getByText('未割当')).toBeInTheDocument();
  });

  it('displays the current value as formatted symbols', () => {
    const { container } = render(HotkeyInput, {
      props: { value: 'Ctrl+Shift+P', onChange: () => {} }
    });
    const kbd = container.querySelector('kbd');
    expect(kbd).not.toBeNull();
    expect(kbd!.textContent).toMatch(/P/);
    expect(kbd!.textContent).toMatch(/⌃|⇧/);
  });

  it('records a modifier+key into an accelerator string', async () => {
    const onChange = vi.fn();
    const { getByRole } = render(HotkeyInput, {
      props: { value: '', onChange }
    });
    // Click the capture button to enter listening mode.
    const btn = getByRole('button');
    await fireEvent.click(btn);
    // Fire a Ctrl+Shift+P keydown at the window level.
    await fireEvent.keyDown(window, {
      key: 'P',
      ctrlKey: true,
      shiftKey: true
    });
    expect(onChange).toHaveBeenCalled();
    const accel = onChange.mock.calls[0][0];
    expect(accel).toContain('Ctrl');
    expect(accel).toContain('Shift');
    expect(accel).toMatch(/\+P$/);
  });

  it('ignores a bare key without any modifier while listening', async () => {
    const onChange = vi.fn();
    const { getByRole } = render(HotkeyInput, {
      props: { value: '', onChange }
    });
    await fireEvent.click(getByRole('button'));
    await fireEvent.keyDown(window, { key: 'A' });
    expect(onChange).not.toHaveBeenCalled();
  });

  it('clear button resets the value to empty', async () => {
    const onChange = vi.fn();
    const { getByLabelText } = render(HotkeyInput, {
      props: { value: 'Ctrl+Shift+P', onChange }
    });
    await fireEvent.click(getByLabelText('クリア'));
    expect(onChange).toHaveBeenCalledWith('');
  });
});
