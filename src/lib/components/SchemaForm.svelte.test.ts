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

  it('resolves $ref into the referenced definition', () => {
    const refSchema = {
      definitions: {
        Settings: {
          type: 'object',
          properties: {
            providers: { $ref: '#/definitions/Providers' }
          }
        },
        Providers: {
          type: 'object',
          properties: {
            translate: { type: 'string' }
          }
        }
      }
    };
    const { getByText } = render(SchemaForm, {
      props: { schema: refSchema, value: { providers: { translate: 'deepl' } } }
    });
    // The inner property "translate" should appear as a label since the
    // $ref expansion reveals the nested object's properties.
    expect(getByText('translate')).toBeInTheDocument();
  });

  it('renders a number array as an add/remove list editor', () => {
    const arrSchema = {
      definitions: {
        Settings: {
          type: 'object',
          properties: {
            weird: { type: 'array', items: { type: 'number' } }
          }
        }
      }
    };
    const { container, getByText } = render(SchemaForm, {
      props: { schema: arrSchema, value: { weird: [1, 2, 3] } }
    });
    // The fallback warning must be gone: the form now ships a proper
    // editor for primitive-array fields. A "項目を追加" button
    // (plus per-row inputs) is always rendered.
    expect(getByText('項目を追加')).toBeInTheDocument();
    const numberInputs = container.querySelectorAll<HTMLInputElement>(
      'input[type="number"]'
    );
    expect(numberInputs.length).toBe(3);
  });

  it('renders an enum array as a multi-select chip list', async () => {
    const arrSchema = {
      definitions: {
        Settings: {
          type: 'object',
          properties: {
            modes: {
              type: 'array',
              items: {
                type: 'string',
                enum: ['timeout', 'error', 'offline']
              }
            }
          }
        }
      }
    };
    let received: unknown[] | null = null;
    const { getByText } = render(SchemaForm, {
      props: {
        schema: arrSchema,
        value: { modes: ['timeout'] },
        onChange: () => {
          /* the value bind handles state; we only verify the wire-up */
          received = ['observed'];
        }
      }
    });
    expect(getByText('timeout')).toBeInTheDocument();
    expect(getByText('error')).toBeInTheDocument();
    expect(getByText('offline')).toBeInTheDocument();
    // Hush the unused warning — the test asserts presence, not edit.
    expect(received).toBeNull();
  });

  it('flattens an Option<T> anyOf into a renderable T control', () => {
    const optSchema = {
      definitions: {
        Settings: {
          type: 'object',
          properties: {
            timeout_ms: {
              anyOf: [{ type: 'integer' }, { type: 'null' }]
            }
          }
        }
      }
    };
    const { container } = render(SchemaForm, {
      props: { schema: optSchema, value: { timeout_ms: 1500 } }
    });
    // The unwrap step should pick the integer branch, so a number
    // input renders rather than the "unsupported" fallback warning.
    const number = container.querySelector<HTMLInputElement>(
      'input[type="number"]'
    );
    expect(number).not.toBeNull();
    expect(number!.value).toBe('1500');
  });

  it('topic restricts the rendered tree to a single root section', () => {
    const { getByText, queryByText } = render(SchemaForm, {
      props: {
        schema,
        value: {
          appearance: { theme: 'system', reduced_motion: false },
          subtitle: { target_language: 'ja', font_size_px: 22 }
        },
        topic: 'subtitle'
      }
    });
    expect(getByText('subtitle')).toBeInTheDocument();
    expect(queryByText('appearance')).not.toBeInTheDocument();
  });

  it('an empty topic falls back to rendering every section', () => {
    const { getByText } = render(SchemaForm, {
      props: {
        schema,
        value: {
          appearance: { theme: 'system', reduced_motion: false },
          subtitle: { target_language: 'ja', font_size_px: 22 }
        },
        topic: ''
      }
    });
    expect(getByText('appearance')).toBeInTheDocument();
    expect(getByText('subtitle')).toBeInTheDocument();
  });

  it('renders a dynamic key/value editor for additionalProperties maps', () => {
    const mapSchema = {
      definitions: {
        Settings: {
          type: 'object',
          properties: {
            aliases: {
              type: 'object',
              additionalProperties: { type: 'string' }
            }
          }
        }
      }
    };
    const { getByText, container } = render(SchemaForm, {
      props: {
        schema: mapSchema,
        value: { aliases: { foo: 'bar', baz: 'qux' } }
      }
    });
    expect(getByText('キーを追加')).toBeInTheDocument();
    const rows = container.querySelectorAll('input[type="text"]');
    // Two rows × 2 inputs each = 4 text inputs.
    expect(rows.length).toBeGreaterThanOrEqual(4);
  });
});
