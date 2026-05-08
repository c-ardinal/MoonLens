import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import NumberSlider from './NumberSlider.svelte';

describe('<NumberSlider>', () => {
  it('renders the current value formatted with the suffix', () => {
    const { container } = render(NumberSlider, {
      props: {
        value: 42,
        onChange: () => {},
        min: 0,
        max: 100,
        suffix: ' px'
      }
    });
    const output = container.querySelector('output');
    expect(output?.textContent?.replace(/\s+/g, ' ').trim()).toContain('42');
    expect(output?.textContent).toContain('px');
  });

  it('calls onChange with the new value when the slider moves', async () => {
    const onChange = vi.fn();
    const { container } = render(NumberSlider, {
      props: {
        value: 10,
        onChange,
        min: 0,
        max: 100
      }
    });
    const slider = container.querySelector<HTMLInputElement>('input[type="range"]');
    expect(slider).not.toBeNull();
    await fireEvent.input(slider!, { target: { value: '57' } });
    expect(onChange).toHaveBeenCalledWith(57);
  });

  it('clamps out-of-range slider values to the configured bounds', async () => {
    const onChange = vi.fn();
    const { container } = render(NumberSlider, {
      props: {
        value: 5,
        onChange,
        min: 1,
        max: 10
      }
    });
    const slider = container.querySelector<HTMLInputElement>('input[type="range"]');
    expect(slider).not.toBeNull();
    // The clamp() helper guards against synthetic out-of-range
    // values that callers might inject (e.g. via paste or scripted
    // events). Even though the native slider already clips, we
    // assert the wrapper does too.
    await fireEvent.input(slider!, { target: { value: '-5' } });
    expect(onChange).toHaveBeenLastCalledWith(1);
    await fireEvent.input(slider!, { target: { value: '999' } });
    expect(onChange).toHaveBeenLastCalledWith(10);
  });

  it('uses the formatter override when provided', () => {
    const { container } = render(NumberSlider, {
      props: {
        value: 0.65,
        onChange: () => {},
        min: 0,
        max: 1,
        step: 0.01,
        formatter: (v: number) => `${Math.round(v * 100)}`,
        suffix: '%'
      }
    });
    const output = container.querySelector('output');
    expect(output?.textContent).toContain('65');
    expect(output?.textContent).toContain('%');
  });
});
