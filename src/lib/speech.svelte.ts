/**
 * ウィンドウ 1 個分の読上状態（依頼・停止・経過の表示）。
 */

import type { UnlistenFn } from '@tauri-apps/api/event';
import { onSpeech, speak, stopSpeaking } from './ipc';
import { skipNote, type SpeechEvent } from './speech';

/** 見送り理由の案内の表示時間（ms）。 */
export const NOTE_MS = 6000;

export class Speaker {
  /** 当該ウィンドウの依頼分を読上中。 */
  speaking = $state(false);
  /** 見送り理由の案内（一定時間で消去）。 */
  note = $state<string | null>(null);
  #timer: ReturnType<typeof setTimeout> | undefined;

  /** バックエンドのイベントを反映。 */
  receive(ev: SpeechEvent): void {
    if (ev.kind === 'ended') {
      this.speaking = false;
      return;
    }
    clearTimeout(this.#timer);
    if (ev.kind === 'started') {
      this.speaking = true;
      this.note = null;
      return;
    }
    this.note = skipNote(ev);
    this.#timer = setTimeout(() => (this.note = null), NOTE_MS);
  }

  /** イベントの受信を開始。戻り値：受信の解除（依頼分の停止を含む）。 */
  async listen(): Promise<UnlistenFn> {
    const unlisten = await onSpeech((ev) => this.receive(ev));
    return () => {
      unlisten();
      clearTimeout(this.#timer);
      this.stop();
    };
  }

  say(text: string, lang: string): void {
    if (text.trim()) speak(text, lang).catch(() => {});
  }

  stop(): void {
    stopSpeaking().catch(() => {});
  }
}
