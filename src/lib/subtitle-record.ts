/**
 * 音声翻訳の記録：時刻表示・文単位への集約・文脈付き再翻訳。
 * 純粋関数のみ（Svelte・IPC へ非依存）。翻訳呼出しは引数で注入。
 */
import type { SubtitleLine } from './ipc';

/** 文の区切りと見なす間隔（ms）。 */
export const GAP_MS = 1500;
/** 1 単位の費用の上限（ASCII 1、他 3。`translate_plan.rs` の cost と同一）。 */
export const MAX_COST = 600;

const SENTENCE_END = /[。．.!?！？…]['"”’」』）)\]】]*$/;
const isAscii = (ch: string | undefined) => ch !== undefined && ch.charCodeAt(0) < 128;

/** 経過時間の表示。1 時間未満は M:SS、以上は H:MM:SS。 */
export function timeLabel(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
}

/** 記録全体の長さ（最終行の終了時刻）。 */
export function durationLabel(lines: readonly SubtitleLine[]): string {
  return timeLabel(lines.reduce((max, l) => Math.max(max, l.end_ms), 0));
}

/** 文字列の費用：ASCII 1、他 3。 */
export function cost(text: string): number {
  let n = 0;
  for (const ch of text) n += isAscii(ch) ? 1 : 3;
  return n;
}

/** 分かち書きなしの文字（かな・漢字・全角記号）。 */
const UNSPACED = /[\u3000-\u30ff\u3400-\u9fff\uff00-\uffef]/;

/**
 * 連結：語間は空白（欧文・ハングル・キリル文字等）。接合点の一方が日本語・中国語の文字、
 * または空白の場合は無空白。
 */
export function joinText(left: string, right: string): string {
  if (!left) return right;
  if (!right) return left;
  const [a, b] = [[...left].at(-1)!, [...right][0]];
  const tight = [a, b].some((ch) => UNSPACED.test(ch) || /\s/.test(ch));
  return tight ? left + right : `${left} ${right}`;
}

/** 翻訳の単位：連続する行と、その原文の連結。 */
export interface Unit {
  lines: SubtitleLine[];
  text: string;
}

/** 連続する行を翻訳の単位へ集約（文末記号・無音の間隔・費用の上限で区切り）。 */
export function groupSentences(lines: readonly SubtitleLine[]): Unit[] {
  const units: Unit[] = [];
  let cur: Unit = { lines: [], text: '' };
  lines.forEach((line, i) => {
    cur.lines.push(line);
    cur.text = joinText(cur.text, line.original.trim());
    const next = lines[i + 1];
    const closes =
      SENTENCE_END.test(cur.text) ||
      (next !== undefined && next.start_ms - line.end_ms > GAP_MS) ||
      cost(cur.text) > MAX_COST;
    if (closes || !next) {
      units.push(cur);
      cur = { lines: [], text: '' };
    }
  });
  return units;
}

export interface RetranslateOptions {
  signal?: AbortSignal;
  onProgress?: (done: number, total: number) => void;
}

export interface RetranslateResult {
  lines: SubtitleLine[];
  /** 翻訳に失敗（例外・空の結果）し、旧訳を維持した単位の数。 */
  failed: number;
}

function abortError(signal: AbortSignal): unknown {
  return signal.reason ?? new DOMException('中止', 'AbortError');
}

/**
 * 単位ごとに順次翻訳し、1 単位 1 行の新しい行を返却。
 * `translate(text, history)` の history は、この処理で完了済みの `[原文, 訳文]`。
 * 中止時は部分結果なしで reject。失敗した単位は旧訳の連結を維持（訳文を失わない）。
 */
export async function retranslate(
  lines: readonly SubtitleLine[],
  translate: (text: string, history: [string, string][]) => Promise<string>,
  opts: RetranslateOptions = {}
): Promise<RetranslateResult> {
  const { signal, onProgress } = opts;
  const units = groupSentences(lines);
  const out: SubtitleLine[] = [];
  const history: [string, string][] = [];
  let failed = 0;
  for (const [i, unit] of units.entries()) {
    if (signal?.aborted) throw abortError(signal);
    let translated = '';
    if (unit.text) {
      try {
        translated = (await translate(unit.text, history.map(([s, t]) => [s, t]))).trim();
      } catch {
        translated = '';
      }
      if (signal?.aborted) throw abortError(signal);
    }
    if (!translated) {
      translated = unit.lines.reduce((acc, l) => joinText(acc, l.translated.trim()), '');
      if (unit.text) failed += 1;
    }
    if (unit.text && translated) history.push([unit.text, translated]);
    out.push({
      seq: i,
      start_ms: unit.lines[0].start_ms,
      end_ms: unit.lines[unit.lines.length - 1].end_ms,
      original: unit.text,
      translated
    });
    onProgress?.(i + 1, units.length);
  }
  return { lines: out, failed };
}
