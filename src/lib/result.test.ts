import { describe, expect, it } from 'vitest';
import type { TaskResult } from './ipc';
import { extractText, providerLabel, summarizeFallback } from './result';

function mk(result: Partial<TaskResult> & Pick<TaskResult, 'output'>): TaskResult {
  return {
    task: 'translate',
    used_provider: 'ollama.gemma4',
    via_cloud: false,
    latency_ms: 0,
    fallback_reasons: [],
    ...result
  } as TaskResult;
}

describe('extractText', () => {
  it('returns the string inside a Text variant', () => {
    expect(extractText(mk({ output: { Text: 'hola' } }))).toBe('hola');
  });

  it('joins OCR lines with newline characters', () => {
    const r = mk({
      output: {
        Ocr: {
          lines: [
            { text: 'one', bbox: [0, 0, 0, 0], confidence: 1 },
            { text: 'two', bbox: [0, 0, 0, 0], confidence: 1 }
          ]
        }
      }
    });
    expect(extractText(r)).toBe('one\ntwo');
  });

  it('returns an empty string for an OCR result with zero lines', () => {
    expect(extractText(mk({ output: { Ocr: { lines: [] } } }))).toBe('');
  });

  it('returns the URL for OpenedExternalUrl', () => {
    expect(extractText(mk({ output: { OpenedExternalUrl: 'https://x/' } }))).toBe(
      'https://x/'
    );
  });

  it('labels intents in parentheses', () => {
    expect(extractText(mk({ output: { Intent: 'qr' } }))).toBe('(qr detected)');
  });

  it('pulls the delta out of a streaming chunk', () => {
    expect(
      extractText(mk({ output: { TextChunk: { delta: 'chunk', done: false } } }))
    ).toBe('chunk');
  });

  it('returns empty string for the unit Empty output', () => {
    // Backend serializes the unit variant as a bare "Empty" string. Our
    // TS type treats it as literal 'Empty'.
    expect(extractText(mk({ output: 'Empty' as unknown as TaskResult['output'] }))).toBe(
      ''
    );
  });

  it('never throws on an unknown output shape', () => {
    // Forward-compat: a newer backend may add new variants. The UI must
    // degrade to an empty string instead of crashing the window.
    const r = mk({
      output: { NewlyAdded: { foo: 1 } } as unknown as TaskResult['output']
    });
    expect(extractText(r)).toBe('');
  });
});

describe('providerLabel', () => {
  it('marks local providers as local', () => {
    expect(
      providerLabel(
        mk({ output: { Text: '' }, used_provider: 'ocrs', via_cloud: false })
      )
    ).toBe('ocrs · local');
  });

  it('marks cloud providers as cloud', () => {
    expect(
      providerLabel(mk({ output: { Text: '' }, used_provider: 'deepl', via_cloud: true }))
    ).toBe('deepl · cloud');
  });
});

describe('summarizeFallback', () => {
  it('is empty when nothing fell back', () => {
    expect(summarizeFallback([])).toBe('');
  });

  it('uses a single-reason phrasing', () => {
    expect(summarizeFallback(['ollama: offline'])).toBe('フォールバック: ollama: offline');
  });

  it('counts stages when multiple reasons are present', () => {
    const out = summarizeFallback(['a: timeout', 'b: offline']);
    expect(out).toContain('2 段');
    expect(out).toContain('a: timeout');
    expect(out).toContain('b: offline');
    expect(out).toContain(' → ');
  });
});
