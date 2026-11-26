/**
 * ウィンドウ翻訳のタイトルバー：幅へ収納可能な操作項目数の算出（DOM 非依存）。
 */

/**
 * 先頭から表示可能な項目数。全項目が収納可能な場合は全数、不足時は
 * 縦メニューのボタン幅（`more`）を確保した上で収納可能な数。
 */
export function fitCount(widths: number[], gap: number, available: number, more: number): number {
  const total = widths.reduce((a, w) => a + w, 0) + gap * Math.max(0, widths.length - 1);
  if (total <= available) return widths.length;
  const room = available - more;
  let used = 0;
  let n = 0;
  for (const w of widths) {
    // 表示項目の後方へ必ず縦メニューのボタンが続くため、項目単位で間隔を加算
    const next = used + w + gap;
    if (next > room) break;
    used = next;
    n++;
  }
  return n;
}
