import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import ActionRibbon from './ActionRibbon.svelte';

describe('<ActionRibbon>', () => {
  it('renders the five canonical text-mode actions in order by default', () => {
    const { container } = render(ActionRibbon);
    const ids = Array.from(container.querySelectorAll<HTMLElement>('[data-action]')).map(
      (el) => el.dataset.action
    );
    expect(ids).toEqual(['translate', 'copy', 'search', 'explain', 'pin']);
  });

  it('promotes open_url first when intent is qr', () => {
    const { container } = render(ActionRibbon, { props: { intent: 'qr' } });
    const ids = Array.from(container.querySelectorAll<HTMLElement>('[data-action]')).map(
      (el) => el.dataset.action
    );
    // For a QR payload the most likely action is opening the URL the
    // decoder returned; translate is irrelevant and is omitted.
    expect(ids[0]).toBe('open_url');
    expect(ids).not.toContain('translate');
  });

  it('promotes search first when intent is photo', () => {
    const { container } = render(ActionRibbon, { props: { intent: 'photo' } });
    const ids = Array.from(container.querySelectorAll<HTMLElement>('[data-action]')).map(
      (el) => el.dataset.action
    );
    // Photo content should send the user to reverse image search first;
    // translate stays in the ribbon because OCR may have found text.
    expect(ids[0]).toBe('search');
    expect(ids).toContain('translate');
  });

  it('treats mixed intent as text-with-search-bumped-up', () => {
    const { container } = render(ActionRibbon, { props: { intent: 'mixed' } });
    const ids = Array.from(container.querySelectorAll<HTMLElement>('[data-action]')).map(
      (el) => el.dataset.action
    );
    expect(ids[0]).toBe('translate');
    expect(ids[1]).toBe('search');
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
