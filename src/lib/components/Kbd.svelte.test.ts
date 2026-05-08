import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import Kbd from './Kbd.svelte';

describe('<Kbd>', () => {
  it('renders the label inside a span with the kbd class', () => {
    const { container } = render(Kbd, { props: { label: '⌘' } });
    const el = container.querySelector('span.kbd');
    expect(el).not.toBeNull();
    expect(el!.textContent).toBe('⌘');
  });
});
