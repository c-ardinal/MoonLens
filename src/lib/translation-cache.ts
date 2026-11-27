/** 訳文キャッシュの表示用整形（設定画面）。 */

import { t } from './i18n';

/** 容量表記：1 MB 未満は KB、以上は MB。 */
export function cacheSize(bytes: number): string {
  if (bytes <= 0) return '0 KB';
  if (bytes < 1_048_576) return `${Math.max(1, Math.round(bytes / 1024)).toLocaleString()} KB`;
  return `${(bytes / 1_048_576).toFixed(1)} MB`;
}

/** 内訳の名称。空文字：追従先アプリ未指定の共通分。 */
export function scopeLabel(scope: string): string {
  return scope ? t('アプリ「{scope}」', { scope }) : t('共通（追従先アプリ未指定）');
}
