/**
 * Lay a translation over the rows of its source block, the way an
 * in-place (Lens-style) translator does: the translated text runs along
 * the original line positions, keeps their indents and spacing, and uses
 * the largest font that still fits. Pure; the caller supplies text
 * measurement so this runs (and is tested) without a DOM.
 */

/** A source row in CSS px, relative to the block's box. */
export interface Row {
  left: number;
  top: number;
  width: number;
  height: number;
  /**
   * Right edge (from the box's left) the text of this row may reach,
   * when something that is not text (a picture) stands beside the row.
   */
  limit?: number;
}

/** One laid-out line of the translation, relative to the block's box. */
export interface FlowLine {
  text: string;
  left: number;
  top: number;
  width: number;
  height: number;
  /** Width of the text itself at the chosen font size. */
  textWidth: number;
}

export interface FlowResult {
  fontSize: number;
  lines: FlowLine[];
  /** The text did not fit the source rows and was spread over the box. */
  overflow: boolean;
}

export interface FlowOptions {
  /** Box (mask) size in CSS px. */
  width: number;
  height: number;
  /** Source rows inside the box, top to bottom. Empty: one row = box. */
  rows: Row[];
  align: 'left' | 'center' | 'right';
  maxFont: number;
  minFont?: number;
  /** Width of `text` at a font size of 1 px. */
  measure: (text: string) => number;
  /** Line height (× font size) used when lines are spread over the box. */
  lineHeight?: number;
  /**
   * Extra width (CSS px) the text may take to the right of the box before
   * the font shrinks. Lets a short label whose translation is longer keep
   * its size, like an in-place translator widens the mask.
   */
  grow?: number;
}

/** Characters that must not start a line (Japanese kinsoku + Latin). */
// i18n-ignore（禁則処理の文字集合）
const NO_START = new Set(
  Array.from(
    // i18n-ignore
    '、。，．,.:;!?！？：；・…‥ー〜～）)]｝}」』】〕〉》〙〗’”ゝゞヽヾぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶㇰㇱㇲㇳㇴㇵㇶㇷㇸㇹㇺㇻㇼㇽㇾㇿ々〻%％'
  )
);
/** Characters that must not end a line. */
// i18n-ignore
const NO_END = new Set(Array.from('（([｛{「『【〔〈《〘〖‘“'));

const isSpace = (s: string) => /^\s+$/.test(s);
const isCjk = (c: string) =>
  /[　-ヿ㐀-䶿一-鿿豈-﫿＀-￯가-힯]/.test(c);

/**
 * Break opportunities: words for spaced scripts, single characters for
 * CJK (phrases when Intl.Segmenter is available), with kinsoku applied
 * by gluing forbidden characters to their neighbour.
 */
export function tokenize(text: string): string[] {
  const raw: string[] = [];
  const Seg = (Intl as unknown as { Segmenter?: typeof Intl.Segmenter }).Segmenter;
  if (Seg) {
    const seg = new Seg('ja', { granularity: 'word' });
    for (const s of seg.segment(text)) raw.push(s.segment);
  } else {
    for (const m of text.matchAll(/\s+|[A-Za-z0-9À-ɏ'’\-]+|./gsu)) raw.push(m[0]);
  }
  // Split long CJK words so they can wrap like running text would.
  const split: string[] = [];
  for (const t of raw) {
    const chars = Array.from(t);
    if (chars.length > 1 && chars.every(isCjk)) split.push(...chars);
    else split.push(t);
  }
  const out: string[] = [];
  for (const t of split) {
    const first = Array.from(t)[0] ?? '';
    const prev = out[out.length - 1];
    const prevLast = prev ? Array.from(prev).pop() ?? '' : '';
    if (prev != null && !isSpace(prev) && !isSpace(t) && (NO_START.has(first) || NO_END.has(prevLast))) {
      out[out.length - 1] = prev + t;
    } else {
      out.push(t);
    }
  }
  return out;
}

/**
 * Greedy line breaking: `widthOf(i)` is the room on line i (CSS px)
 * and `w(token)` a token's width at the current font size. Returns the
 * lines' text. A token wider than a line is split by character.
 */
export function breakLines(
  tokens: string[],
  widthOf: (line: number) => number,
  w: (token: string) => number
): string[] {
  const lines: string[] = [];
  let cur = '';
  let curW = 0;
  // Width of trailing spaces on `cur`; they do not count toward fit.
  let trail = 0;
  const flush = () => {
    lines.push(cur.trimEnd());
    cur = '';
    curW = 0;
    trail = 0;
  };
  const push = (t: string) => {
    const tw = w(t);
    if (isSpace(t)) {
      if (cur) {
        cur += t;
        curW += tw;
        trail += tw;
      }
      return;
    }
    const room = widthOf(lines.length);
    if (!cur || curW + tw <= room + 0.01) {
      if (!cur && tw > room + 0.01 && Array.from(t).length > 1) {
        for (const c of Array.from(t)) push(c);
        return;
      }
      cur += t;
      curW += tw;
      trail = 0;
      return;
    }
    curW -= trail;
    flush();
    push(t);
  };
  for (const t of tokens) push(t);
  if (cur.trim()) flush();
  return lines;
}

/**
 * Lay out `text` over the source rows. Tries the largest font first:
 * the translation fits when it needs no more lines than the source has
 * rows. Otherwise the lines are spread evenly over the whole box.
 */
export function flowLayout(text: string, o: FlowOptions): FlowResult {
  const minFont = o.minFont ?? 7;
  const lh = o.lineHeight ?? 1.15;
  const rows: Row[] = o.rows.length > 0 ? o.rows : [{ left: 0, top: 0, width: o.width, height: o.height }];
  const tokens = tokenize(text.replace(/\s*\n\s*/g, ' ').trim());
  const unit = new Map<string, number>();
  const unitW = (t: string) => {
    let v = unit.get(t);
    if (v == null) {
      v = o.measure(t);
      unit.set(t, v);
    }
    return v;
  };
  // Room on each source row: left-aligned rows keep their indent and run
  // to the block's right edge; centred / right-aligned text gets the
  // full width.
  const grow = o.align === 'left' ? Math.max(0, o.grow ?? 0) : 0;
  const right = Math.max(...rows.map((r) => r.left + r.width), o.width) + grow;
  const rowLeft = (r: Row) => (o.align === 'left' ? r.left : 0);
  // A row beside a picture ends at the picture.
  const rowRight = (r: Row) => (r.limit == null ? right : Math.min(right, Math.max(r.limit, r.left + r.width)));
  const rowRoom = (i: number) => (i < rows.length ? rowRight(rows[i]) - rowLeft(rows[i]) : o.width);
  // Lines spread over the box stay clear of every picture beside it.
  const spreadWidth = Math.min(o.width, ...rows.map((r) => (r.limit == null ? o.width : rowRight(r))));
  const textWidth = (t: string, fs: number) => o.measure(t) * fs;

  const place = (lines: string[], fs: number, overflow: boolean): FlowResult => {
    if (!overflow) {
      return {
        fontSize: fs,
        overflow,
        lines: lines.map((t, i) => {
          const r = rows[i];
          const left = rowLeft(r);
          const width = Math.min(rowRight(r), o.width) - left;
          return { text: t, left, top: r.top, width, height: r.height, textWidth: textWidth(t, fs) };
        })
      };
    }
    // Never tighter than a readable line height: if even the smallest
    // font does not fit, the last lines run past the box (the renderer
    // clips them) instead of overlapping.
    const pitch = Math.max(o.height / Math.max(1, lines.length), fs * lh);
    return {
      fontSize: fs,
      overflow,
      lines: lines.map((t, i) => ({
        text: t,
        left: 0,
        top: i * pitch,
        width: spreadWidth,
        height: pitch,
        textWidth: textWidth(t, fs)
      }))
    };
  };

  const start = Math.max(minFont, Math.floor(o.maxFont * 2) / 2);
  const along = (fs: number) => breakLines(tokens, rowRoom, (t) => unitW(t) * fs);
  const spread = (fs: number) => breakLines(tokens, () => spreadWidth, (t) => unitW(t) * fs);
  let rowsFs: number | null = null;
  for (let fs = start; fs >= minFont; fs -= 0.5) {
    if (along(fs).length <= rows.length) {
      rowsFs = fs;
      break;
    }
  }
  if (rowsFs === start) return place(along(start), start, false);
  let spreadFs: number | null = null;
  for (let fs = start; fs > (rowsFs ?? minFont - 0.5); fs -= 0.5) {
    if (fs * lh * spread(fs).length <= o.height + 0.5) {
      spreadFs = fs;
      break;
    }
  }
  // Keep the source rows unless that costs more than a fifth of the
  // font size compared with spreading the lines over the whole box.
  if (rowsFs != null && (spreadFs == null || rowsFs >= spreadFs * 0.8)) {
    return place(along(rowsFs), rowsFs, false);
  }
  if (spreadFs != null) return place(spread(spreadFs), spreadFs, true);
  const full = breakLines(tokens, () => spreadWidth, (t) => unitW(t) * minFont);
  return place(full, minFont, true);
}
