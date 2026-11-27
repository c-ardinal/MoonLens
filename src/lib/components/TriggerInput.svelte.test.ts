import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/svelte';
import { sourceText } from '$lib/i18n';

/** The capture the backend is waiting on, resolved by each test. */
let finish: (trigger: string | null) => void = () => {};
const ipc = vi.hoisted(() => ({
  triggerSupported: vi.fn(async () => true),
  captureTrigger: vi.fn(),
  cancelTriggerCapture: vi.fn(async () => {})
}));
vi.mock('$lib/ipc', () => ipc);

import TriggerInput from './TriggerInput.svelte';

const WAITING = sourceText('trigger-input:waiting_for_input_esc_cancel_backspace_clear');

async function mount(value = '') {
  const onChange = vi.fn();
  const view = render(TriggerInput, { props: { value, label: 'L', onChange } });
  const button = view.getByRole('button') as HTMLButtonElement;
  await waitFor(() => expect(button.disabled).toBe(false));
  return { ...view, button, onChange };
}

describe('<TriggerInput>', () => {
  beforeEach(() => {
    ipc.captureTrigger.mockReset();
    ipc.captureTrigger.mockImplementation(() => new Promise((resolve) => (finish = resolve)));
    ipc.cancelTriggerCapture.mockClear();
  });

  it('records the trigger the backend reports', async () => {
    const { button, onChange, getByText, queryByText } = await mount();
    await fireEvent.click(button);
    expect(getByText(WAITING)).toBeInTheDocument();
    finish('Pad:Back+Y');
    await waitFor(() => expect(onChange).toHaveBeenCalledWith('Pad:Back+Y'));
    expect(queryByText(WAITING)).not.toBeInTheDocument();
  });

  it('shows the stored trigger by name', async () => {
    const { button } = await mount('Double:Control');
    expect(button.textContent).toContain(sourceText('trigger:double_tap').replace('{key}', 'Ctrl'));
  });

  it('cancels on Esc and drops a result that arrives afterwards', async () => {
    const { button, onChange, queryByText } = await mount('Mouse4');
    await fireEvent.click(button);
    await fireEvent.keyDown(button, { key: 'Escape' });
    expect(ipc.cancelTriggerCapture).toHaveBeenCalledTimes(1);
    expect(queryByText(WAITING)).not.toBeInTheDocument();
    finish('Mouse5');
    await Promise.resolve();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('clears the trigger on Backspace', async () => {
    const { button, onChange } = await mount('Mouse4');
    await fireEvent.click(button);
    await fireEvent.keyDown(button, { key: 'Backspace' });
    expect(onChange).toHaveBeenCalledWith('');
    expect(ipc.cancelTriggerCapture).toHaveBeenCalledTimes(1);
  });

  it('keeps the value when nothing was performed in time', async () => {
    const { button, onChange, queryByText } = await mount('Mouse4');
    await fireEvent.click(button);
    finish(null);
    await waitFor(() => expect(queryByText(WAITING)).not.toBeInTheDocument());
    expect(onChange).not.toHaveBeenCalled();
  });

  it('keeps side buttons from navigating while recording', async () => {
    const { button } = await mount();
    const side = () => new MouseEvent('mouseup', { button: 3, cancelable: true });
    const idle = side();
    window.dispatchEvent(idle);
    expect(idle.defaultPrevented).toBe(false);
    await fireEvent.click(button);
    const recording = side();
    window.dispatchEvent(recording);
    expect(recording.defaultPrevented).toBe(true);
  });
});
