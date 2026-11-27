/**
 * Pure helpers for turning a backend `TaskResult` into UI-ready strings.
 * Extracted from the Svelte pages so we can unit test the projection
 * independently of the DOM / Tauri runtime.
 */

import type { TaskOutput, TaskResult } from './ipc';
import { t } from './i18n';

/**
 * Render a `TaskOutput` as a single translation-like string for the
 * overlay body. Unknown shapes collapse to an empty string rather than
 * throwing so a mis-typed backend response never blanks the window.
 */
export function extractText(result: TaskResult): string {
  const o = result.output as TaskOutput;
  if (typeof o === 'string') return '';
  if ('Text' in o) return o.Text;
  if ('Ocr' in o) return o.Ocr.lines.map((l) => l.text).join('\n');
  if ('OpenedExternalUrl' in o) return o.OpenedExternalUrl;
  if ('Intent' in o) return `(${o.Intent} detected)`;
  if ('TextChunk' in o) return o.TextChunk.delta;
  return '';
}

/**
 * Describe the provider in one short UI line, including whether it ran
 * locally or hit the network.
 */
export function providerLabel(result: TaskResult): string {
  const where = result.via_cloud ? 'cloud' : 'local';
  return `${result.used_provider} · ${where}`;
}

/**
 * Convert a provider's `fallback_reasons` trail into a human line. Caller
 * decides when to show it (usually only after a cloud fallback).
 */
export function summarizeFallback(reasons: string[]): string {
  if (reasons.length === 0) return '';
  if (reasons.length === 1) return t('フォールバック: {reason}', { reason: reasons[0] });
  return t('フォールバック {n} 段: {reasons}', { n: reasons.length, reasons: reasons.join(' → ') });
}
