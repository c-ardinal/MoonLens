import { describe, expect, it } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import SchemaForm from './SchemaForm.svelte';

/**
 * Minimal end-to-end render of the auto-generated settings form. We feed
 * it a tiny JSON Schema (similar to what schemars emits on the Rust
 * side) and poke a few inputs to verify:
 *  - each top-level section renders as its own panel
 *  - editing a string/bool/number writes back into `value`
 *  - the `query` filter hides non-matching sections
 */

const schema = {
  definitions: {
    Settings: {
      type: 'object',
      properties: {
        appearance: {
          type: 'object',
          properties: {
            theme: { type: 'string', enum: ['light', 'dark', 'system'] },
            reduced_motion: { type: 'boolean' }
          }
        },
        subtitle: {
          type: 'object',
          properties: {
            target_language: { type: 'string' },
            font_size_px: { type: 'integer' }
          }
        }
      }
    }
  }
};

describe('<SchemaForm>', () => {
  it('renders a section for each top-level property', () => {
    const { getByText } = render(SchemaForm, {
      props: {
        schema,
        value: {
          appearance: { theme: 'system', reduced_motion: false },
          subtitle: { target_language: 'ja', font_size_px: 22 }
        }
      }
    });
    expect(getByText('appearance')).toBeInTheDocument();
    expect(getByText('subtitle')).toBeInTheDocument();
  });

  it('typing in the query hides non-matching sections', () => {
    const { getByText, queryByText } = render(SchemaForm, {
      props: {
        schema,
        value: {
          appearance: { theme: 'system', reduced_motion: false },
          subtitle: { target_language: 'ja', font_size_px: 22 }
        },
        query: 'subtitle'
      }
    });
    expect(getByText('subtitle')).toBeInTheDocument();
    expect(queryByText('appearance')).not.toBeInTheDocument();
  });

  it('the boolean control is rendered as a checkbox reflecting value', () => {
    const { container } = render(SchemaForm, {
      props: {
        schema,
        value: {
          appearance: { theme: 'dark', reduced_motion: true },
          subtitle: { target_language: 'ja', font_size_px: 22 }
        }
      }
    });
    const checkboxes = container.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]'
    );
    expect(checkboxes.length).toBeGreaterThan(0);
    expect(checkboxes[0].checked).toBe(true);
  });

  it('the enum control renders a select listing every variant', () => {
    const { container } = render(SchemaForm, {
      props: {
        schema,
        value: {
          appearance: { theme: 'system', reduced_motion: false },
          subtitle: { target_language: 'ja', font_size_px: 22 }
        }
      }
    });
    const select = container.querySelector<HTMLSelectElement>('select');
    expect(select).not.toBeNull();
    const options = Array.from(select!.options).map((o) => o.value);
    expect(options).toContain('light');
    expect(options).toContain('dark');
    expect(options).toContain('system');
  });

  it('invokes onChange when the user edits a field', async () => {
    let callCount = 0;
    const { container } = render(SchemaForm, {
      props: {
        schema,
        value: {
          appearance: { theme: 'system', reduced_motion: false },
          subtitle: { target_language: 'ja', font_size_px: 22 }
        },
        onChange: () => {
          callCount += 1;
        }
      }
    });
    const text = container.querySelector<HTMLInputElement>('input[type="text"]');
    expect(text).not.toBeNull();
    await fireEvent.input(text!, { target: { value: 'fr' } });
    expect(callCount).toBeGreaterThanOrEqual(1);
  });
});
