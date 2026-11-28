/**
 * 訳文の読上（OS の音声合成、バックエンド：`speech/mod.rs`）。読上の有無はウィンドウ単位。
 * 依頼は呼出元ウィンドウの所有となり、経過は同ウィンドウ宛のイベントで受信。
 */

import { t } from './i18n';
import type { LensBlock } from './ipc';
import { languageLabel } from './lens-overlay';

/** バックエンド → 依頼元ウィンドウ：依頼の経過。 */
export const SPEECH_EVENT = 'moonlens://speech';

/** 読上の見送り理由：PC 全体の音声翻訳中／対象言語の音声が未導入／エンジンの失敗。 */
export type SpeechSkip = 'blocked' | 'no_voice' | 'failed';

export type SpeechEvent =
  | { kind: 'started' }
  /** 完了、または停止。 */
  | { kind: 'ended' }
  | { kind: 'skipped'; reason: SpeechSkip; lang: string }
  /** 案内なしの破棄（新規の依頼による待機分の破棄、発音対象のない文面）。 */
  | { kind: 'dropped' };

/** 導入済みの音声。 */
export interface Voice {
  id: string;
  name: string;
  /** BCP 47（例：ja-JP）。 */
  lang: string;
}

/** 見送り時の案内文。 */
export function skipNote(ev: { reason: SpeechSkip; lang: string }): string {
  if (ev.reason === 'blocked') return t('speech:blocked_by_whole_pc_capture');
  if (ev.reason === 'no_voice') return t('speech:no_voice_for_language', { language: t(languageLabel(ev.lang)) });
  return t('speech:failed');
}

/** 音声の選択肢の表記。 */
export const voiceLabel = (v: Voice): string => t('speech:voice_with_language', { name: v.name, lang: v.lang });

/**
 * 試聴の文面と言語。表示言語と音声の言語が一致する場合は案内文、不一致の場合は音声名
 * （当該言語の文面を保持しないため）。音声未選択（自動）は訳文の言語で試聴。
 */
export function sampleFor(voice: Voice | undefined, language: string): { text: string; lang: string } {
  const lang = voice?.lang ?? language;
  const primary = (tag: string) => tag.split(/[-_]/)[0].toLowerCase();
  const text = primary(lang) === primary(t('speech:sample_language')) ? t('speech:sample') : (voice?.name ?? 'MoonLens');
  return { text, lang };
}

/** 読上済みの訳文（照合用の文字列 → 画面からの消失時刻。表示中は null）。 */
export type Spoken = Map<string, number | null>;

/**
 * 画面から消失した訳文の記憶期間。期間内の再表示（読取の揺らぎ、スクロールの往復）は
 * 再読上せず、期間経過後の再表示は新規の文として読上。表示の継続中は期間に無関係で再読上なし。
 */
export const FORGET_MS = 10_000;

/** 表記差（大小文字・空白・約物）を除去した照合用の文字列。 */
const key = (text: string): string =>
  text.normalize('NFKC').toLowerCase().replace(/[^\p{L}\p{N}]/gu, '');

/**
 * ウィンドウ翻訳：今回の読取結果のうち未読上の訳文（表示順）。`spoken` を更新。
 * 翻訳待ち・翻訳不要（対象言語と同一）のブロックは対象外。
 * 消失の検出は呼出し時のため、文字なしの読取結果（空配列）でも呼出しが必要。
 */
export function unspoken(spoken: Spoken, blocks: LensBlock[], now: number): string[] {
  const fresh: string[] = [];
  const shown = new Set<string>();
  for (const b of blocks) {
    const text = b.same_language ? '' : (b.translation ?? '').trim();
    const k = key(text);
    if (!k || shown.has(k)) continue;
    shown.add(k);
    const gone = spoken.get(k);
    const remembered = spoken.has(k) && (gone == null || now - gone <= FORGET_MS);
    if (!remembered) fresh.push(text);
    spoken.set(k, null);
  }
  for (const [k, gone] of spoken) {
    if (shown.has(k)) continue;
    if (gone == null) spoken.set(k, now);
    else if (now - gone > FORGET_MS) spoken.delete(k);
  }
  return fresh;
}
