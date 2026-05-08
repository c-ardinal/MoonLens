import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import Segmented from './Segmented.svelte';

const options = [
  { value: 'a', label: 'Alpha' },
  { value: 'b', label: 'Beta' },
  { value: 'c', label: 'Gamma' }
];

describe('<Segmented>', () => {
  it('marks only the option matching value as active', () => {
    const { getByText } = render(Segmented, {
      props: { options, value: 'b', onChange: () => {} }
    });
    expect(getByText('Beta').classList.contains('active')).toBe(true);
    expect(getByText('Alpha').classList.contains('active')).toBe(false);
    expect(getByText('Gamma').classList.contains('active')).toBe(false);
  });

  it('emits the value of the clicked option through onChange', async () => {
    const onChange = vi.fn();
    const { getByText } = render(Segmented, {
      props: { options, value: 'a', onChange }
    });
    await fireEvent.click(getByText('Gamma'));
    expect(onChange).toHaveBeenCalledWith('c');
  });

  it('exposes radio semantics via aria-checked', () => {
    const { getAllByRole } = render(Segmented, {
      props: { options, value: 'a', onChange: () => {} }
    });
    const radios = getAllByRole('radio');
    expect(radios).toHaveLength(3);
    expect(radios[0].getAttribute('aria-checked')).toBe('true');
    expect(radios[1].getAttribute('aria-checked')).toBe('false');
  });
});
