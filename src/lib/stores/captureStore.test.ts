import { describe, expect, it } from 'vitest';
import { reduceCaptureEvent, type CaptureState } from './captureStore';
import type { OverlayEvent, TaskResult } from '$lib/ipc';

const baseState: CaptureState = {
  stage: 'idle',
  statusMessage: '',
  blocks: [],
  lastError: null,
  lastProvider: null,
  lastLatencyMs: null,
  lastIntent: null,
  lastSourceRect: null,
  streamingText: '',
  updatedAt: 0
};

const okResult: TaskResult = {
  task: 'capture',
  output: { Text: 'translated text' },
  used_provider: 'ollama.gemma4',
  via_cloud: false,
  latency_ms: 421,
  fallback_reasons: []
};

describe('reduceCaptureEvent', () => {
  it('moves to processing on a processing event', () => {
    const event: OverlayEvent = { kind: 'processing', message: 'OCR中…' };
    const next = reduceCaptureEvent(baseState, event, 100);
    expect(next.stage).toBe('processing');
    expect(next.statusMessage).toBe('OCR中…');
    expect(next.lastError).toBeNull();
    expect(next.updatedAt).toBe(100);
  });

  it('appends streaming chunks while keeping previous result intact', () => {
    const start = reduceCaptureEvent(
      baseState,
      { kind: 'processing', message: '' },
      1
    );
    const after = [
      { kind: 'chunk', delta: 'Hel', done: false },
      { kind: 'chunk', delta: 'lo, ', done: false },
      { kind: 'chunk', delta: 'world!', done: true }
    ].reduce<CaptureState>(
      (s, e, i) => reduceCaptureEvent(s, e as OverlayEvent, 2 + i),
      start
    );
    expect(after.streamingText).toBe('Hello, world!');
    expect(after.stage).toBe('processing');
  });

  it('lands in result with parsed blocks, provider and latency', () => {
    const event: OverlayEvent = {
      kind: 'result',
      source_text: 'hello',
      intent: 'text',
      result: okResult,
      source_rect: null
    };
    const next = reduceCaptureEvent(baseState, event, 200);
    expect(next.stage).toBe('result');
    expect(next.lastProvider).toBe('ollama.gemma4');
    expect(next.lastLatencyMs).toBe(421);
    expect(next.blocks).toHaveLength(1);
    expect(next.blocks[0].dst).toBe('translated text');
    expect(next.blocks[0].src).toBe('hello');
    // The streamingText buffer is wiped: the result is authoritative.
    expect(next.streamingText).toBe('');
  });

  it('records the error message on an error event', () => {
    const event: OverlayEvent = {
      kind: 'error',
      message: 'プロバイダがオフラインです'
    };
    const after = reduceCaptureEvent(
      { ...baseState, stage: 'processing' },
      event,
      300
    );
    expect(after.stage).toBe('error');
    expect(after.lastError).toBe('プロバイダがオフラインです');
  });

  it('ignores hidden events so the result panel survives overlay dismissal', () => {
    const result = reduceCaptureEvent(
      baseState,
      {
        kind: 'result',
        source_text: 'x',
        intent: 'text',
        result: okResult,
        source_rect: null
      },
      400
    );
    const after = reduceCaptureEvent(result, { kind: 'hidden' }, 401);
    expect(after).toBe(result);
  });
});
