import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import Toggle from './Toggle.svelte';

describe('<Toggle>', () => {
  it('reflects the checked state via aria-checked', () => {
    const off = render(Toggle, { props: { checked: false, onChange: () => {} } });
    expect(off.getByRole('switch').getAttribute('aria-checked')).toBe('false');
    off.unmount();

    const on = render(Toggle, { props: { checked: true, onChange: () => {} } });
    expect(on.getByRole('switch').getAttribute('aria-checked')).toBe('true');
  });

  it('flips the value through onChange when clicked', async () => {
    const onChange = vi.fn();
    const { getByRole } = render(Toggle, { props: { checked: false, onChange } });
    await fireEvent.click(getByRole('switch'));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('does not call onChange when disabled', async () => {
    const onChange = vi.fn();
    const { getByRole } = render(Toggle, {
      props: { checked: false, onChange, disabled: true }
    });
    await fireEvent.click(getByRole('switch'));
    expect(onChange).not.toHaveBeenCalled();
  });
});
