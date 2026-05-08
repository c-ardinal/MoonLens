import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import Pill from './Pill.svelte';

/**
 * Children-less rendering is enough to assert the variant classes that
 * drive the visual state. Snippet-content rendering is exercised via
 * higher-level component tests rather than here.
 */
describe('<Pill>', () => {
  it('renders a span with the .pill class', () => {
    const { getByTestId } = render(Pill);
    const el = getByTestId('pill');
    expect(el.tagName).toBe('SPAN');
    expect(el.classList.contains('pill')).toBe(true);
    expect(el.classList.contains('accent')).toBe(false);
    expect(el.classList.contains('dot')).toBe(false);
  });

  it('adds the accent class when accent is true', () => {
    const { getByTestId } = render(Pill, { props: { accent: true } });
    expect(getByTestId('pill').classList.contains('accent')).toBe(true);
  });

  it('adds the dot class when dot is true', () => {
    const { getByTestId } = render(Pill, { props: { dot: true } });
    expect(getByTestId('pill').classList.contains('dot')).toBe(true);
  });
});
