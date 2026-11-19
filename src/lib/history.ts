/**
 * History page helpers: kind labels and the kind filter.
 *
 * `kind` values come from `pipeline::history_kind` in the backend. Rows
 * written before those existed use `translate` for every translation, so
 * they count as screen translations.
 */
import type { HistoryEntry } from './ipc';

export type HistoryFilter = 'all' | 'screen' | 'lens' | 'live' | 'text' | 'qr';

export const HISTORY_FILTERS: { value: HistoryFilter; label: string }[] = [
  { value: 'all', label: '全件' },
  { value: 'screen', label: '画面' },
  { value: 'lens', label: 'Lens' },
  { value: 'live', label: 'ライブ' },
  { value: 'text', label: 'テキスト' },
  { value: 'qr', label: 'QR' }
];

const KIND_LABELS: Record<string, string> = {
  capture: '画面翻訳',
  translate: '画面翻訳',
  lens: 'Lens',
  live: 'ライブ翻訳',
  text: 'テキスト翻訳',
  qr: 'QR コード',
  ocr: '文字認識',
  explain: '説明',
  summarize: '要約',
  transcript: '字幕',
  asr: '字幕'
};

/** Display label for a history row's kind (unknown kinds as is). */
export function kindLabel(kind: string): string {
  return KIND_LABELS[kind] ?? kind;
}

/** Filter a kind belongs to; `null` for kinds no tab covers. */
export function filterOf(kind: string): Exclude<HistoryFilter, 'all'> | null {
  switch (kind) {
    case 'capture':
    case 'translate':
      return 'screen';
    case 'lens':
    case 'live':
    case 'text':
    case 'qr':
      return kind;
    default:
      return null;
  }
}

export function applyFilter(entries: HistoryEntry[], filter: HistoryFilter): HistoryEntry[] {
  if (filter === 'all') return entries;
  return entries.filter((e) => filterOf(e.kind) === filter);
}

/** Row count per filter, for the tab badges. */
export function countByFilter(entries: HistoryEntry[]): Record<HistoryFilter, number> {
  const counts: Record<HistoryFilter, number> = {
    all: entries.length,
    screen: 0,
    lens: 0,
    live: 0,
    text: 0,
    qr: 0
  };
  for (const e of entries) {
    const f = filterOf(e.kind);
    if (f) counts[f] += 1;
  }
  return counts;
}

/** Keep the selection when it is still visible, else select the first row. */
export function nextSelection(visible: HistoryEntry[], selected: number | null): number | null {
  if (selected !== null && visible.some((e) => e.id === selected)) return selected;
  return visible[0]?.id ?? null;
}
