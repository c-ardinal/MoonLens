import { describe, expect, it, vi } from 'vitest';
import {
  LOG_LINE_RE,
  PUNCT_ONLY_RE,
  shouldBypassLine,
  splitLeadingWs,
  translateLines,
  type LineHistory,
  type LineTranslation,
} from './translate-line';

// -------------------- shouldBypassLine --------------------

describe('shouldBypassLine', () => {
  it('bypasses an empty line', () => {
    expect(shouldBypassLine('')).toBe(true);
  });

  it('bypasses a whitespace-only line', () => {
    expect(shouldBypassLine('   ')).toBe(true);
    expect(shouldBypassLine('\t\t')).toBe(true);
  });

  it('bypasses a Rust tracing log line (INFO)', () => {
    expect(
      shouldBypassLine(
        '2026-05-18T00:51:11.674022Z  INFO embedded_llm_perf: warmup',
      ),
    ).toBe(true);
  });

  it('bypasses log lines for every supported level', () => {
    for (const lvl of ['INFO', 'WARN', 'ERROR', 'DEBUG', 'TRACE']) {
      expect(
        shouldBypassLine(`2026-05-18T00:51:11Z ${lvl} mod: msg`),
      ).toBe(true);
    }
  });

  it('bypasses a leading-whitespace log line', () => {
    expect(
      shouldBypassLine(
        '   2026-05-18T00:51:11Z INFO mod: indented log',
      ),
    ).toBe(true);
  });

  it('bypasses pure punctuation lines', () => {
    expect(shouldBypassLine('}')).toBe(true);
    expect(shouldBypassLine('};')).toBe(true);
    expect(shouldBypassLine('  )  ')).toBe(true);
    expect(shouldBypassLine('};')).toBe(true);
    expect(shouldBypassLine('};,')).toBe(true);
  });

  it('does NOT bypass normal prose', () => {
    expect(shouldBypassLine('Hello, world.')).toBe(false);
    expect(shouldBypassLine('The quick brown fox.')).toBe(false);
  });

  it('does NOT bypass an indented prose line', () => {
    expect(shouldBypassLine('    1. First step description')).toBe(false);
  });

  it('does NOT bypass a line that mentions a date in prose', () => {
    // The LOG_LINE_RE requires `T<time>` AND a log level — a prose
    // line that contains a date but no log structure stays
    // translatable. Regression guard against making the regex
    // too aggressive.
    expect(shouldBypassLine('We met on 2026-05-18 for coffee.')).toBe(false);
  });
});

// -------------------- LOG_LINE_RE / PUNCT_ONLY_RE --------------------

describe('LOG_LINE_RE', () => {
  it('anchors at start, not mid-string', () => {
    expect(LOG_LINE_RE.test('prefix 2026-05-18T00:51:11Z INFO mod: x')).toBe(
      false,
    );
  });

  it('requires log level immediately after timestamp', () => {
    expect(LOG_LINE_RE.test('2026-05-18T00:51:11Z mod: no level')).toBe(false);
  });

  it('matches with multiple spaces between timestamp and level', () => {
    expect(LOG_LINE_RE.test('2026-05-18T00:51:11Z   INFO mod: x')).toBe(true);
  });
});

describe('PUNCT_ONLY_RE', () => {
  it('matches braces, brackets, parens, separators', () => {
    expect(PUNCT_ONLY_RE.test('}')).toBe(true);
    expect(PUNCT_ONLY_RE.test('};')).toBe(true);
    expect(PUNCT_ONLY_RE.test('])')).toBe(true);
    expect(PUNCT_ONLY_RE.test('   ,;.   ')).toBe(true);
  });

  it('does not match if any letter is present', () => {
    expect(PUNCT_ONLY_RE.test('}a')).toBe(false);
    expect(PUNCT_ONLY_RE.test('a;')).toBe(false);
  });
});

// -------------------- splitLeadingWs --------------------

describe('splitLeadingWs', () => {
  it('extracts plain spaces', () => {
    expect(splitLeadingWs('    hello')).toEqual({
      leading: '    ',
      content: 'hello',
    });
  });

  it('extracts tabs', () => {
    expect(splitLeadingWs('\t\thello')).toEqual({
      leading: '\t\t',
      content: 'hello',
    });
  });

  it('extracts mixed tab + space', () => {
    expect(splitLeadingWs('\t  hello')).toEqual({
      leading: '\t  ',
      content: 'hello',
    });
  });

  it('returns empty leading for unindented line', () => {
    expect(splitLeadingWs('hello')).toEqual({
      leading: '',
      content: 'hello',
    });
  });

  it('handles an empty string', () => {
    expect(splitLeadingWs('')).toEqual({ leading: '', content: '' });
  });

  it('handles a whitespace-only line', () => {
    expect(splitLeadingWs('     ')).toEqual({
      leading: '     ',
      content: '',
    });
  });
});

// -------------------- translateLines --------------------

function ok(text: string, provider: string | null = 'mock', viaCloud = false): LineTranslation {
  return { text, provider, viaCloud };
}

describe('translateLines', () => {
  it('translates each line independently and rejoins with newlines', async () => {
    const seen: string[] = [];
    const translate = vi.fn(async (line: string) => {
      seen.push(line);
      return ok(line.toUpperCase());
    });
    const res = await translateLines('hello\nworld', translate);
    expect(seen).toEqual(['hello', 'world']);
    expect(res.output).toBe('HELLO\nWORLD');
    expect(res.translatedCount).toBe(2);
  });

  it('preserves the exact newline count', async () => {
    const translate = vi.fn(async (line: string) => ok(line.toUpperCase()));
    const res = await translateLines('a\nb\nc\nd', translate);
    expect(res.output.split('\n')).toHaveLength(4);
    expect(res.output).toBe('A\nB\nC\nD');
  });

  it('preserves trailing newline as an empty entry', async () => {
    const translate = vi.fn(async (line: string) => ok(line.toUpperCase()));
    const res = await translateLines('a\nb\n', translate);
    expect(res.output).toBe('A\nB\n');
  });

  it('preserves blank lines verbatim without calling the translator', async () => {
    const translate = vi.fn(async (line: string) => ok(`T(${line})`));
    const res = await translateLines('hello\n\nworld', translate);
    expect(res.output).toBe('T(hello)\n\nT(world)');
    expect(translate).toHaveBeenCalledTimes(2);
  });

  it('bypasses log lines without calling the translator', async () => {
    const translate = vi.fn(async (line: string) => ok('TRANSLATED'));
    const logLine = '2026-05-18T00:51:11Z INFO mod: msg';
    const res = await translateLines(`${logLine}\nprose line`, translate);
    expect(translate).toHaveBeenCalledOnce();
    expect(translate.mock.calls[0][0]).toBe('prose line');
    expect(res.output).toBe(`${logLine}\nTRANSLATED`);
    expect(res.translatedCount).toBe(1);
  });

  it('bypasses punct-only lines (closing braces)', async () => {
    const translate = vi.fn(async (line: string) => ok(line.toUpperCase()));
    const res = await translateLines('hello\n}\nworld', translate);
    expect(translate).toHaveBeenCalledTimes(2);
    expect(res.output).toBe('HELLO\n}\nWORLD');
  });

  it('preserves leading indentation across the round trip', async () => {
    const translate = vi.fn(async (line: string) => ok(line.toUpperCase()));
    const res = await translateLines(
      '  indented\n    deeper\nflat',
      translate,
    );
    // The translator should only see the trimmed content; the
    // indent gets restored by the wrapper afterwards.
    expect(translate.mock.calls.map((c) => c[0])).toEqual(['indented', 'deeper', 'flat']);
    expect(res.output).toBe('  INDENTED\n    DEEPER\nFLAT');
  });

  it('captures the provider from the FIRST non-bypassed line', async () => {
    let n = 0;
    const translate = vi.fn(async (line: string) => {
      const provider = `prov-${n}`;
      const viaCloud = n === 0;
      n += 1;
      return ok(line.toUpperCase(), provider, viaCloud);
    });
    // First line is bypassed (log), second is the first
    // non-bypassed line — its provider becomes the headline.
    const res = await translateLines(
      '2026-05-18T00:51:11Z INFO mod: x\nfirst translatable\nsecond translatable',
      translate,
    );
    expect(res.firstProvider).toBe('prov-0');
    expect(res.firstViaCloud).toBe(true);
  });

  it('falls back to the original line on per-line failure', async () => {
    const translate = vi.fn(async (line: string) => {
      if (line === 'fail') throw new Error('boom');
      return ok(line.toUpperCase());
    });
    const res = await translateLines('ok\nfail\nalso ok', translate);
    expect(res.output).toBe('OK\nfail\nALSO OK');
    expect(res.translatedCount).toBe(3); // attempted count, includes the failure
  });

  it('reports progress for each translated line', async () => {
    const translate = vi.fn(async (line: string) => ok(line.toUpperCase()));
    const progress: Array<[number, number]> = [];
    await translateLines(
      'a\nb\nc',
      translate,
      (done, total) => progress.push([done, total]),
    );
    expect(progress).toEqual([
      [1, 3],
      [2, 3],
      [3, 3],
    ]);
  });

  it('progress total excludes bypassed lines', async () => {
    const translate = vi.fn(async (line: string) => ok(line.toUpperCase()));
    const progress: Array<[number, number]> = [];
    await translateLines(
      '2026-05-18T00:51:11Z INFO mod: x\nprose1\n\nprose2',
      translate,
      (done, total) => progress.push([done, total]),
    );
    // Two non-bypassed lines, so total === 2 throughout.
    expect(progress).toEqual([
      [1, 2],
      [2, 2],
    ]);
  });

  it('end-to-end log paste mimics the user-reported repro', async () => {
    // Translator that returns plausible Japanese for each line.
    // The point of the test is the WRAPPER's structure preservation,
    // not the translation quality — log lines must come through
    // verbatim and the prose-y interleaved line must get translated.
    const translate = vi.fn(async (line: string) => {
      if (line.includes('Translate me')) return ok('翻訳して');
      return ok('翻訳結果');
    });
    const input = [
      '2026-05-18T00:51:11.674022Z  INFO embedded_llm_perf: warmup',
      '2026-05-18T00:51:11.674320Z  INFO embedded_llm_perf: ready',
      'Translate me please',
      '2026-05-18T00:52:33.572626Z  INFO embedded_llm_perf: done',
    ].join('\n');

    const res = await translateLines(input, translate);
    const lines = res.output.split('\n');
    expect(lines).toHaveLength(4);
    // All log lines preserved exactly:
    expect(lines[0]).toBe(
      '2026-05-18T00:51:11.674022Z  INFO embedded_llm_perf: warmup',
    );
    expect(lines[1]).toBe(
      '2026-05-18T00:51:11.674320Z  INFO embedded_llm_perf: ready',
    );
    expect(lines[3]).toBe(
      '2026-05-18T00:52:33.572626Z  INFO embedded_llm_perf: done',
    );
    // Only the prose line got translated:
    expect(lines[2]).toBe('翻訳して');
    // Translator was only called once (for the prose line):
    expect(translate).toHaveBeenCalledOnce();
    expect(translate.mock.calls[0][0]).toBe('Translate me please');
  });

  it('passes the lines translated so far as history', async () => {
    const translate = vi.fn(async (line: string, _history: LineHistory) => ok(line.toUpperCase()));
    await translateLines('one\n\n  two\n}\nthree', translate);
    expect(translate.mock.calls.map((c) => c[1])).toEqual([
      [],
      [['one', 'ONE']],
      [
        ['one', 'ONE'],
        ['two', 'TWO'],
      ],
    ]);
  });

  it('leaves failed and empty translations out of the history', async () => {
    const translate = vi.fn(async (line: string, _history: LineHistory) => {
      if (line === 'bad') throw new Error('boom');
      if (line === 'empty') return ok('');
      return ok(line.toUpperCase());
    });
    await translateLines('a\nbad\nempty\nb', translate);
    expect(translate.mock.calls[3][1]).toEqual([['a', 'A']]);
  });

  it('hands each call its own snapshot of the history', async () => {
    const seen: unknown[] = [];
    await translateLines('a\nb', async (line, history) => {
      seen.push(history);
      return ok(line);
    });
    expect(seen[0]).toEqual([]);
    expect(seen[0]).not.toBe(seen[1]);
  });
});
