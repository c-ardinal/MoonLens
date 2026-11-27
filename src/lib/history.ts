/**
 * History page helpers: kind labels and the kind filter.
 *
 * `kind` values come from `pipeline::history_kind` in the backend. Rows
 * written before those existed use `translate` for every translation, so
 * they count as window translations. One-off (`capture`) and continuous
 * (`lens`, and `live` from before the two were merged) window
 * translations share one tab. `subtitle` (a timed record) and the older
 * `transcript` / `asr` rows share the audio tab.
 */
import type { HistoryEntry } from './ipc';

export type HistoryFilter = 'all' | 'window' | 'image' | 'text' | 'qr' | 'audio';

export const HISTORY_FILTERS: { value: HistoryFilter; label: string }[] = [
  { value: 'all', label: '全件' },
  { value: 'window', label: 'ウィンドウ' },
  { value: 'image', label: '画像' },
  { value: 'text', label: 'テキスト' },
  { value: 'qr', label: 'QR' },
  { value: 'audio', label: '音声' }
];

const KIND_LABELS: Record<string, string> = {
  capture: 'ウィンドウ翻訳',
  translate: 'ウィンドウ翻訳',
  lens: 'ウィンドウ翻訳（常時）',
  live: 'ウィンドウ翻訳（常時）',
  image: '画像翻訳',
  text: 'テキスト翻訳',
  qr: 'QR コード',
  ocr: '文字認識',
  explain: '説明',
  summarize: '要約',
  subtitle: '音声翻訳',
  transcript: '音声翻訳',
  asr: '音声翻訳'
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
    case 'lens':
    case 'live':
      return 'window';
    case 'subtitle':
    case 'transcript':
    case 'asr':
      return 'audio';
    case 'image':
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
    window: 0,
    image: 0,
    text: 0,
    qr: 0,
    audio: 0
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

/** 詳細の日時表示（例：2026/11/27 06:02）。`created` は UNIX 秒。 */
export function dateLabel(created: number): string {
  return new Date(created * 1000).toLocaleString('ja-JP', {
    year: 'numeric',
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  });
}
