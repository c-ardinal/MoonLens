import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import PrivacyBadge from './PrivacyBadge.svelte';

describe('<PrivacyBadge>', () => {
  it('labels a local provider as Local', () => {
    const { getByText } = render(PrivacyBadge, {
      props: { viaCloud: false, provider: 'ocrs' }
    });
    expect(getByText('Local')).toBeInTheDocument();
  });

  it('labels a cloud provider as Cloud', () => {
    const { getByText, queryByText } = render(PrivacyBadge, {
      props: { viaCloud: true, provider: 'deepl' }
    });
    expect(getByText('Cloud')).toBeInTheDocument();
    expect(queryByText('Local')).not.toBeInTheDocument();
  });

  it('exposes the provider name via the title attribute for tooltip', () => {
    const { container } = render(PrivacyBadge, {
      props: { viaCloud: true, provider: 'gemini' }
    });
    const span = container.querySelector('span[title]');
    expect(span?.getAttribute('title')).toBe('gemini');
  });
});
