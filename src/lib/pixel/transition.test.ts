import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { EFFECTS, play, replay, type Effect } from './transition';

const dataset: Record<string, string> = {};

function fakeNode() {
  const calls: { frames: Keyframe[]; opts: KeyframeAnimationOptions }[] = [];
  const node = {
    animate: vi.fn((frames: Keyframe[], opts: KeyframeAnimationOptions) => {
      calls.push({ frames, opts });
      return { id: '', cancel: vi.fn() };
    }),
    getAnimations: () => []
  } as unknown as HTMLElement;
  return { node, calls };
}

beforeEach(() => {
  vi.stubGlobal('document', { documentElement: { dataset } });
  dataset.skin = 'pixel';
  dataset.motion = 'on';
});
afterEach(() => vi.unstubAllGlobals());

describe('play', () => {
  it('段階変化（steps）の単発演出として再生', () => {
    const { node, calls } = fakeNode();
    expect(play(node, 'type')).not.toBeNull();
    expect(calls[0].opts.easing).toBe(`steps(${EFFECTS.type.steps}, end)`);
    expect(calls[0].opts.duration).toBeLessThanOrEqual(200);
  });

  it('推論中・動き削減時（data-motion が on 以外）は無処理', () => {
    dataset.motion = 'off';
    const { node, calls } = fakeNode();
    expect(play(node, 'open')).toBeNull();
    expect(calls).toHaveLength(0);
  });

  it('ドット絵外観以外（訳文表示の窓）は無処理', () => {
    delete dataset.skin;
    const { node, calls } = fakeNode();
    play(node, 'open');
    expect(calls).toHaveLength(0);
  });
});

describe('replay', () => {
  it('初回表示・同一 key では再生なし、key 変化時のみ方向付きで再生', () => {
    const { node, calls } = fakeNode();
    const effect = (a: number, b: number): Effect => (b > a ? 'slide-left' : 'slide-right');
    const action = replay(node, { key: 1, effect });
    expect(calls).toHaveLength(0);
    action.update({ key: 1, effect });
    expect(calls).toHaveLength(0);
    action.update({ key: 2, effect });
    expect(calls[0].frames).toBe(EFFECTS['slide-left'].frames);
    action.update({ key: 0, effect });
    expect(calls[1].frames).toBe(EFFECTS['slide-right'].frames);
  });

  it('方向関数が null の場合は再生なし', () => {
    const { node, calls } = fakeNode();
    const action = replay(node, { key: 'a', effect: () => null });
    action.update({ key: 'b', effect: () => null });
    expect(calls).toHaveLength(0);
  });
});
