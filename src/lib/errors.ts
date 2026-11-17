/**
 * Backend commands return errors as "<code>|<message>" (see
 * src-tauri/src/user_error.rs). The code tells the UI which fix to offer
 * next to the message.
 */

export type ErrorCode =
  | 'model_missing'
  | 'asr_unavailable'
  | 'cpu_unsupported'
  | 'timeout'
  | 'network'
  | 'disk_full'
  | 'integrity'
  | 'error';

export interface UiError {
  code: ErrorCode;
  message: string;
  /** In-app page that fixes the problem, if any. */
  fix: { label: string; route: string } | null;
}

const CODES: ErrorCode[] = [
  'model_missing',
  'asr_unavailable',
  'cpu_unsupported',
  'timeout',
  'network',
  'disk_full',
  'integrity',
  'error'
];

export const MODELS_ROUTE = '/settings/#section-models';

export function parseError(e: unknown): UiError {
  const raw = String(e ?? '').replace(/^Error:\s*/, '');
  const bar = raw.indexOf('|');
  const head = bar > 0 ? raw.slice(0, bar) : '';
  const code = (CODES as string[]).includes(head) ? (head as ErrorCode) : 'error';
  const message = code === head ? raw.slice(bar + 1) : raw;
  const fix = code === 'model_missing' ? { label: 'モデル準備', route: MODELS_ROUTE } : null;
  return { code, message, fix };
}

/** Just the text, for places without room for a button. */
export const errorText = (e: unknown) => parseError(e).message;
