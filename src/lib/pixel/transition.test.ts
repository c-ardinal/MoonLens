import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { EFFECTS, play, replay, wheelStep, type Effect } from './transition';

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
    expect(calls[0].opts.easing).toBe(`steps(${EFFECTS.type.steps}, start)`);
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

describe('wheelStep', () => {
  const wheel = (deltaY: number) => ({ deltaY, deltaX: 0, ctrlKey: false, preventDefault: vi.fn() }) as unknown as WheelEvent;
  function fakeTarget() {
    let handler: (e: WheelEvent) => void = () => {};
    const node = {
      addEventListener: (_: string, h: (e: WheelEvent) => void) => (handler = h),
      removeEventListener: vi.fn()
    } as unknown as HTMLElement;
    return { node, fire: (e: WheelEvent) => handler(e) };
  }

  it('1 ノッチで 1 項目、方向は符号に一致、連続入力は間隔を確保', () => {
    let now = 1000;
    vi.spyOn(performance, 'now').mockImplementation(() => now);
    const { node, fire } = fakeTarget();
    const steps: number[] = [];
    wheelStep(node, (d) => steps.push(d));
    fire(wheel(100));
    expect(steps).toEqual([1]);
    now += 50; // 間隔未満：無視
    fire(wheel(100));
    expect(steps).toEqual([1]);
    now += 200;
    fire(wheel(-100));
    // 累積（+100-100=0）は閾値未満
    expect(steps).toEqual([1]);
    fire(wheel(-100));
    expect(steps).toEqual([1, -1]);
  });

  it('微小な入力（トラックパッド）は累積で 1 項目', () => {
    let now = 5000;
    vi.spyOn(performance, 'now').mockImplementation(() => now);
    const { node, fire } = fakeTarget();
    const steps: number[] = [];
    wheelStep(node, (d) => steps.push(d));
    for (let i = 0; i < 5; i++) fire(wheel(10));
    expect(steps).toEqual([]);
    fire(wheel(10));
    expect(steps).toEqual([1]);
  });
});
