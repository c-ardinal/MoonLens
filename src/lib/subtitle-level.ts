/**
 * 音声翻訳ウィンドウの入力レベル表示（波形・閾値・入力状態）。
 * DOM・Tauri 非依存：時刻は引数で受領。
 */

/** バックエンドのイベント（`moonlens://subtitle-level`）。 */
export type LevelEvent = {
  /** 25 ms 単位の RMS レベル（dBFS、時系列順）。空配列＝音声入力の途絶。 */
  bins: number[];
  /** 認識閾値（dBFS）。未満の音声は無音扱い。 */
  gate_db: number;
};

/** 入力状態：入力未検出・認識閾値未満・認識対象の音声を検出。 */
export type InputState = 'none' | 'low' | 'ok';

export const BINS_PER_SECOND = 40;
export const FLOOR_DB = -96;
/** 表示範囲の下限（dBFS）。上限は 0 dBFS。 */
export const SCALE_MIN_DB = -80;
/** 入力状態の判定区間：直近 0.6 秒。発話中の短い間では状態を維持。 */
const HOLD_BINS = 24;
/** 無信号扱いの上限（dBFS）。 */
const NO_SIGNAL_DB = -90;
/** 保持上限：幅 3840px 相当。 */
const CAPACITY = 1024;

/** dBFS → 表示上の振幅比（0〜1）。 */
export function levelRatio(db: number): number {
  return Math.min(1, Math.max(0, (db - SCALE_MIN_DB) / -SCALE_MIN_DB));
}

/** 波形の 1 フレーム分：右端基準のレベル列と描画位置の端数。 */
export type ScopeFrame = {
  /** 新→旧の順（先頭が右端）。 */
  levels: number[];
  /** 右端レベルの進行度（0〜1）。スクロールの補間用。 */
  shift: number;
};

/**
 * 入力レベルの履歴と再生位置。イベントは 50 ms 間隔で到着するため、
 * 表示位置を一定速度で進行させ、スクロールを平滑化。
 */
export class LevelScope {
  private ring = new Int8Array(CAPACITY).fill(FLOOR_DB);
  /** 受信済みレベルの総数。 */
  private total = 0;
  /** 表示済み位置（レベル単位、小数）。 */
  private shown = 0;
  private last = 0;
  private idle = true;
  gateDb = -50;

  receive(ev: LevelEvent, now: number): void {
    this.gateDb = ev.gate_db;
    this.advance(now);
    this.idle = ev.bins.length === 0;
    for (const db of ev.bins) this.append(db);
  }

  private append(db: number): void {
    this.ring[this.total % CAPACITY] = Math.max(FLOOR_DB, Math.min(0, Math.round(db)));
    this.total += 1;
  }

  /** 時刻を進行。入力途絶中は無信号レベルを補充し、過去の波形を流去。 */
  advance(now: number): void {
    const dt = Math.max(0, now - this.last) / 1000;
    this.last = now;
    const next = this.shown + dt * BINS_PER_SECOND;
    if (this.idle) {
      // 表示位置を時刻通りに進行し、不足分を無信号で補充。
      this.shown = Math.max(next, this.total - 4);
      const fill = Math.min(CAPACITY, Math.ceil(this.shown) - this.total);
      for (let i = 0; i < fill; i++) this.append(FLOOR_DB);
      return;
    }
    // 受信遅延の許容：2 イベント分（4 レベル）以内で追従。
    this.shown = Math.min(this.total, Math.max(next, this.total - 4));
  }

  private at(index: number): number {
    return index < 0 || index < this.total - CAPACITY ? FLOOR_DB : this.ring[index % CAPACITY];
  }

  frame(count: number): ScopeFrame {
    const head = Math.ceil(this.shown) - 1;
    const levels = Array.from({ length: Math.min(count, CAPACITY) }, (_, i) => this.at(head - i));
    return { levels, shift: this.shown - Math.floor(this.shown) };
  }

  /** 直近レベル（タイトルバーのメーター用）。 */
  current(): number {
    return this.at(Math.ceil(this.shown) - 1);
  }

  state(): InputState {
    if (this.idle) return 'none';
    let max = FLOOR_DB;
    for (let i = 1; i <= HOLD_BINS; i++) max = Math.max(max, this.at(this.total - i));
    if (max <= NO_SIGNAL_DB) return 'none';
    return max >= this.gateDb ? 'ok' : 'low';
  }

  /** 表示範囲が全て無信号かつ入力途絶中：再描画不要。 */
  settled(count: number): boolean {
    if (!this.idle) return false;
    for (let i = 1; i <= Math.min(count, CAPACITY); i++) {
      if (this.at(this.total - i) > FLOOR_DB) return false;
    }
    return true;
  }
}
