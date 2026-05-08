/**
 * Global capture state store.
 *
 * The Capture page lives inside a routed layout: navigating away
 * unmounts it and a fresh mount loses any in-flight overlay event
 * subscription. The user observed this as "switching tabs cancels
 * the capture queue" — backend kept processing but the GUI lost its
 * tracking.
 *
 * To fix that we subscribe to the overlay event stream once at the
 * app shell level and keep the latest stage / blocks / error in this
 * store. Pages read from it on mount, so switching back shows the
 * exact state the backend last reported.
 */

import { writable, type Writable } from 'svelte/store';
import {
  onOverlay,
  type CaptureFlowRect,
  type Intent,
  type OverlayEvent,
  type TaskResult
} from '$lib/ipc';

export type CaptureStage =
  | 'idle'
  | 'capturing'
  | 'processing'
  | 'result'
  | 'error';

export interface ResultBlock {
  id: number;
  src: string;
  dst: string;
  confidence: number | null;
}

export interface CaptureState {
  stage: CaptureStage;
  statusMessage: string;
  blocks: ResultBlock[];
  lastError: string | null;
  lastProvider: string | null;
  lastLatencyMs: number | null;
  lastIntent: Intent | null;
  lastSourceRect: CaptureFlowRect | null;
  /** Accumulated streamed translation, when the provider streams chunks. */
  streamingText: string;
  /** Wall-clock ms when the latest event arrived. Pages use it to age out
   *  the "processing" indicator if no further events arrive. */
  updatedAt: number;
}

const initial: CaptureState = {
  stage: 'idle',
  statusMessage: '',
  blocks: [],
  lastError: null,
  lastProvider: null,
  lastLatencyMs: null,
  lastIntent: null,
  lastSourceRect: null,
  streamingText: '',
  updatedAt: Date.now()
};

export const captureState: Writable<CaptureState> = writable(initial);

let unlisten: (() => void) | null = null;
let initialized = false;

export function blocksFromResult(
  result: TaskResult,
  sourceText: string | null
): ResultBlock[] {
  const out = result.output as
    | { Ocr?: { lines?: { text: string; confidence: number }[] } }
    | { Text?: string }
    | string;
  if (typeof out === 'object' && out !== null && 'Ocr' in out) {
    const lines = (out as { Ocr?: { lines?: { text: string; confidence: number }[] } })
      .Ocr?.lines;
    if (lines && lines.length > 0) {
      return lines.map((line, i) => ({
        id: i,
        src: line.text,
        dst: line.text,
        confidence: line.confidence
      }));
    }
  }
  if (sourceText) {
    const translated =
      typeof out === 'object' && out !== null && 'Text' in out
        ? (out as { Text?: string }).Text ?? sourceText
        : sourceText;
    return [
      {
        id: 0,
        src: sourceText,
        dst: translated,
        confidence: null
      }
    ];
  }
  return [];
}

/**
 * Pure reducer applied to the in-memory capture state for each
 * incoming overlay event. Extracted so unit tests can exercise the
 * full event matrix without going through Tauri IPC.
 */
export function reduceCaptureEvent(
  state: CaptureState,
  event: OverlayEvent,
  now: number = Date.now()
): CaptureState {
  if (event.kind === 'processing') {
    return {
      ...state,
      stage: 'processing',
      statusMessage: event.message,
      lastError: null,
      streamingText: '',
      updatedAt: now
    };
  }
  if (event.kind === 'chunk') {
    return {
      ...state,
      stage: 'processing',
      streamingText: state.streamingText + event.delta,
      updatedAt: now
    };
  }
  if (event.kind === 'result') {
    return {
      ...state,
      stage: 'result',
      statusMessage: '',
      blocks: blocksFromResult(event.result, event.source_text),
      lastError: null,
      lastProvider: event.result.used_provider,
      lastLatencyMs: event.result.latency_ms,
      lastIntent: event.intent,
      lastSourceRect: event.source_rect,
      streamingText: '',
      updatedAt: now
    };
  }
  if (event.kind === 'error') {
    return {
      ...state,
      stage: 'error',
      lastError: event.message,
      streamingText: '',
      updatedAt: now
    };
  }
  // 'hidden' is informational for the floating overlay only.
  return state;
}

/** Wire the overlay event stream into the store. Idempotent — calling
 *  it twice is a no-op so layout components can call it freely. */
export async function ensureCaptureSubscription(): Promise<void> {
  if (initialized) return;
  initialized = true;
  try {
    unlisten = await onOverlay((event: OverlayEvent) => {
      captureState.update((s) => reduceCaptureEvent(s, event));
    });
  } catch {
    initialized = false;
  }
}

export function setCaptureStage(
  stage: CaptureStage,
  patch: Partial<CaptureState> = {}
) {
  captureState.update((s) => ({
    ...s,
    ...patch,
    stage,
    updatedAt: Date.now()
  }));
}

export function clearCaptureResult() {
  captureState.set({
    ...initial,
    updatedAt: Date.now()
  });
}

export function teardownCaptureSubscription() {
  unlisten?.();
  unlisten = null;
  initialized = false;
}
