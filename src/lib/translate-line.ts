/**
 * Line-by-line translation helpers, extracted from the Translate
 * page so the regex + dispatch logic can be unit-tested independently
 * of Svelte's runtime and the Tauri IPC layer.
 *
 * The Translate page wires `runTranslate` (Tauri IPC) into
 * `translateLines` via the `translateLine` callback so the IPC
 * dependency stays at the route layer where the language pair
 * state lives, while the splitting / bypass / re-assembly
 * concerns live here where they can be tested in isolation.
 */

/** A line shaped like a Rust tracing / generic ISO 8601 log
 *  entry — timestamp followed by a log level. These get bypassed
 *  because feeding them to the LLM either returns the line
 *  unchanged or invents a "translation" of the timestamp /
 *  identifier portion. */
export const LOG_LINE_RE =
  /^\s*\d{4}-\d{2}-\d{2}T\S+\s+(INFO|WARN|ERROR|DEBUG|TRACE)\b/;

/** A line composed entirely of code punctuation (closing braces,
 *  separators). `}`, `};`, `  )  ` etc. Sending these to the LLM
 *  is pure waste — there is no semantic content to translate. */
export const PUNCT_ONLY_RE = /^[\s})\]([{;,.]*$/;

/** Decide whether a single line should bypass the LLM entirely
 *  in line-by-line mode. The line is then emitted verbatim in
 *  the output. */
export function shouldBypassLine(line: string): boolean {
  if (!line.trim()) return true;
  if (LOG_LINE_RE.test(line)) return true;
  if (PUNCT_ONLY_RE.test(line)) return true;
  return false;
}

/** Split a line into `(leading whitespace, content)` so the caller
 *  can strip indent before invoking the LLM and re-prepend it on
 *  the result. Keeps per-line indentation intact across the
 *  translation round trip. */
export function splitLeadingWs(line: string): { leading: string; content: string } {
  const m = line.match(/^(\s*)([\s\S]*)$/);
  return {
    leading: m ? m[1] : '',
    content: m ? m[2] : line,
  };
}

/** Per-line translation outcome the caller hands back to
 *  `translateLines`. Modelled after the relevant TaskResult fields
 *  so the route layer can extract whichever envelope shape the IPC
 *  returns without leaking it here. */
export interface LineTranslation {
  /** Translated text (already stripped of leading/trailing
   *  whitespace and any embedded newlines collapsed). */
  text: string;
  /** Provider id that handled this line, if available. */
  provider: string | null;
  /** Whether this line went through a cloud provider. */
  viaCloud: boolean;
}

/** Aggregate result of `translateLines`. */
export interface LinesResult {
  /** Final reassembled string with original newline count
   *  preserved. */
  output: string;
  /** Provider that handled the first non-bypassed line. Surfaced
   *  in the UI as the "engine that did the work" — using the
   *  first non-bypassed line is intentional: the first line is
   *  the most likely to be representative prose. */
  firstProvider: string | null;
  /** Cloud flag from the first non-bypassed line, matching
   *  `firstProvider`. */
  firstViaCloud: boolean;
  /** Number of lines that were actually sent to the translator
   *  (after bypass filtering). Caller can use this for analytics
   *  or to decide whether to record history. */
  translatedCount: number;
}

/** Translate an input string line-by-line.
 *
 * - Blank lines: emitted verbatim, no IPC call
 * - Lines matching `shouldBypassLine`: emitted verbatim, no IPC
 *   call
 * - Other lines: leading whitespace stripped, content sent through
 *   `translateLine`, result has the original leading whitespace
 *   re-prepended
 *
 * Per-line `translateLine` failures fall back to the original
 * line content (NOT the trimmed content) so the user sees the
 * source untranslated rather than a hole in the output.
 *
 * @param text          input string with `\n` separators
 * @param translateLine async callback per line; the route layer
 *                      wires this to Tauri's `runTranslate`
 * @param onProgress    optional progress callback fired after each
 *                      translated line; `done` excludes bypassed
 *                      lines so the count matches `total`
 */
export async function translateLines(
  text: string,
  translateLine: (line: string) => Promise<LineTranslation>,
  onProgress?: (done: number, total: number) => void,
): Promise<LinesResult> {
  // Split on the literal `\n` (not splitlines) so the rejoin
  // below faithfully reproduces the original separator count
  // — including a possible trailing empty entry from a final
  // newline in the input.
  const lines = text.split('\n');
  const out: string[] = new Array(lines.length);
  const total = lines.filter((l) => !shouldBypassLine(l)).length;
  let done = 0;
  let firstProvider: string | null = null;
  let firstViaCloud = false;
  let firstSet = false;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (shouldBypassLine(line)) {
      out[i] = line;
      continue;
    }
    const { leading, content } = splitLeadingWs(line);
    try {
      const translation = await translateLine(content);
      out[i] = `${leading}${translation.text}`;
      if (!firstSet) {
        firstProvider = translation.provider;
        firstViaCloud = translation.viaCloud;
        firstSet = true;
      }
    } catch {
      out[i] = line;
    }
    done += 1;
    onProgress?.(done, total);
  }

  return {
    output: out.join('\n'),
    firstProvider,
    firstViaCloud,
    translatedCount: done,
  };
}
