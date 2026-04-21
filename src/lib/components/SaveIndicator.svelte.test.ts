import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import SaveIndicator from './SaveIndicator.svelte';

/**
 * Visual status only — the four states each produce a distinct label
 * so the user sees immediately whether their last edit has been
 * persisted.
 */

describe('<SaveIndicator>', () => {
  it.each([
    ['saved', '保存済み'],
    ['dirty', '未保存…'],
    ['saving', '保存中…'],
    ['error', '保存失敗']
  ] as const)('renders the %s state with label "%s"', (state, label) => {
    const { getByText } = render(SaveIndicator, { props: { state } });
    expect(getByText(label)).toBeInTheDocument();
  });

  it('surfaces the error message via title when in error state', () => {
    const { container } = render(SaveIndicator, {
      props: { state: 'error', errorMessage: 'disk full' }
    });
    const host = container.firstElementChild as HTMLElement;
    expect(host.getAttribute('title')).toBe('disk full');
  });
});
