import { describe, expect, it } from 'vitest';
import { FLOOR_DB, LevelScope, levelRatio } from './subtitle-level';

const GATE = -50.3;
const ev = (...bins: number[]) => ({ bins, gate_db: GATE });

describe('入力レベル表示', () => {
  it('dBFS を表示範囲（-80〜0）の振幅比へ変換', () => {
    expect(levelRatio(0)).toBe(1);
    expect(levelRatio(-80)).toBe(0);
    expect(levelRatio(FLOOR_DB)).toBe(0);
    expect(levelRatio(-40)).toBeCloseTo(0.5);
    expect(levelRatio(GATE)).toBeGreaterThan(0.3);
  });

  it('入力状態：未検出・閾値未満・音声検出', () => {
    const s = new LevelScope();
    expect(s.state()).toBe('none');
    s.receive(ev(-70, -68), 0);
    expect(s.state()).toBe('low');
    s.receive(ev(-30, -28), 50);
    expect(s.state()).toBe('ok');
    // 発話中の短い間（0.6 秒未満）は音声検出を維持
    for (let t = 100; t < 500; t += 50) s.receive(ev(-70, -70), t);
    expect(s.state()).toBe('ok');
    for (let t = 500; t < 1200; t += 50) s.receive(ev(-70, -70), t);
    expect(s.state()).toBe('low');
    // 音声入力の途絶
    s.receive(ev(), 1250);
    expect(s.state()).toBe('none');
  });

  it('無信号（デジタル無音）のフレーム継続は入力未検出', () => {
    const s = new LevelScope();
    s.receive(ev(FLOOR_DB, FLOOR_DB), 0);
    expect(s.state()).toBe('none');
  });

  it('波形は右端が最新、一定速度で進行', () => {
    const s = new LevelScope();
    s.receive(ev(-40, -30), 0);
    s.advance(1000);
    expect(s.frame(3).levels).toEqual([-30, -40, FLOOR_DB]);
    expect(s.current()).toBe(-30);
    // 次のレベルは 25 ms 単位で表示位置へ到達
    s.receive(ev(-20, -10), 1000);
    s.advance(1025);
    expect(s.frame(2).levels).toEqual([-20, -30]);
    s.advance(1050);
    expect(s.frame(2).levels).toEqual([-10, -20]);
  });

  it('受信遅延後は最新レベルへ追従（遅延は 4 レベル以内）', () => {
    const s = new LevelScope();
    s.receive(ev(...Array.from({ length: 40 }, (_, i) => -i)), 0);
    s.advance(1);
    expect(s.frame(1).levels[0]).toBeLessThanOrEqual(-35);
  });

  it('入力途絶中は過去の波形を流去し、消去後は再描画不要', () => {
    const s = new LevelScope();
    s.receive(ev(-30, -30), 0);
    s.advance(100);
    s.receive(ev(), 150);
    expect(s.settled(8)).toBe(false);
    s.advance(250);
    expect(s.frame(8).levels.filter((l) => l > FLOOR_DB)).toHaveLength(2);
    expect(s.frame(8).levels[0]).toBe(FLOOR_DB);
    // 描画フレーム単位（16 ms）の進行でも流去
    for (let t = 266; t <= 600; t += 16) s.advance(t);
    expect(s.settled(8)).toBe(true);
    // 再入力で描画再開
    s.receive(ev(-30), 1050);
    expect(s.settled(8)).toBe(false);
  });
});
