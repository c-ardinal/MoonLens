import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import MoonLensMark from './MoonLensMark.svelte';

describe('<MoonLensMark>', () => {
  it('renders an svg sized to the requested pixel value', () => {
    const { getByTestId } = render(MoonLensMark, { props: { size: 32 } });
    const svg = getByTestId('moonlens-mark');
    expect(svg.getAttribute('width')).toBe('32');
    expect(svg.getAttribute('height')).toBe('32');
  });

  it('omits the glow ring by default and adds it when glow is true', () => {
    const off = render(MoonLensMark, { props: {} });
    expect(off.container.querySelector('animate')).toBeNull();
    off.unmount();

    const on = render(MoonLensMark, { props: { glow: true } });
    expect(on.container.querySelector('animate')).not.toBeNull();
  });
});
