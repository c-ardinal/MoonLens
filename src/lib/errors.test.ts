import { describe, expect, it } from 'vitest';
import { MODELS_ROUTE, errorText, parseError } from './errors';

describe('parseError', () => {
  it('splits a coded backend error and offers the fix', () => {
    const e = parseError('model_missing|翻訳モデル未導入：PLaMo 3 Translate 2B');
    expect(e.code).toBe('model_missing');
    expect(e.message).toBe('翻訳モデル未導入：PLaMo 3 Translate 2B');
    expect(e.fix?.route).toBe(MODELS_ROUTE);
  });

  it('keeps uncoded text whole, including bars inside it', () => {
    expect(parseError(new Error('a|b')).message).toBe('a|b');
    expect(parseError('plain failure')).toEqual({ code: 'error', message: 'plain failure', fix: null });
  });

  it('has a text-only helper', () => {
    expect(errorText('network|ネットワーク接続失敗')).toBe('ネットワーク接続失敗');
  });
});
