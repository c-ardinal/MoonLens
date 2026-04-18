import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import ActionRibbon from './ActionRibbon.svelte';

describe('<ActionRibbon>', () => {
  it('renders the five canonical actions in order', () => {
    const { container } = render(ActionRibbon);
    const ids = Array.from(container.querySelectorAll<HTMLElement>('[data-action]')).map(
      (el) => el.dataset.action
    );
    expect(ids).toEqual(['translate', 'copy', 'search', 'explain', 'pin']);
  });

  it('invokes onAction with the clicked action id', async () => {
    const onAction = vi.fn();
    const { container } = render(ActionRibbon, { props: { onAction } });
    const copy = container.querySelector<HTMLElement>('[data-action="copy"]');
    expect(copy).not.toBeNull();
    await fireEvent.click(copy!);
    expect(onAction).toHaveBeenCalledWith('copy');
  });

  it('does not throw when no onAction handler is provided', async () => {
    const { container } = render(ActionRibbon);
    const first = container.querySelector<HTMLElement>('[data-action]');
    await fireEvent.click(first!);
    // Passing through without error is the assertion; if the component
    // blew up the test would fail at the click above.
    expect(first).toBeInTheDocument();
  });
});
