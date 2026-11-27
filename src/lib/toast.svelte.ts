/**
 * メインウィンドウ共通の通知（画面下端へ一時表示）。
 * 用途：操作結果（コピー完了・削除完了）、操作の失敗、実行済み操作の取消。
 * 継続する状態（読込失敗・設定の不整合等）は対象外：各画面の常設表示を使用。
 *
 * 各通知は「表示中 → 終了」の一方向。終了理由は 1 回のみ確定し、onClose へ通知。
 * 表示は Toaster.svelte（+layout.svelte へ配置）。
 */

import { errorText } from '$lib/errors';
import { t } from './i18n';

export type ToastKind = 'info' | 'error';
/** 終了理由。action：通知内の操作ボタン、timeout：表示時間の満了、dismiss：利用者による閉鎖。 */
export type CloseReason = 'action' | 'timeout' | 'dismiss';

export interface ToastOptions {
  text: string;
  kind?: ToastKind;
  /** 操作ボタンの表示名（例：取消）。押下時は理由 action で終了。 */
  action?: string;
  /** 表示時間（ms）。省略時は種別・操作ボタンの有無から決定。 */
  ms?: number;
  /** 終了時に 1 回のみ呼出し。 */
  onClose?: (reason: CloseReason) => void;
}

export interface Toast {
  readonly id: number;
  readonly kind: ToastKind;
  readonly text: string;
  readonly action: string | null;
}

/** 同時表示の上限。超過時は最古の通知を満了扱いで終了。 */
export const MAX_TOASTS = 3;
/** 既定の表示時間（ms）：通常・エラー・操作ボタン付き。 */
export const TOAST_MS = { info: 2500, error: 6000, action: 6000 } as const;

/** 表示時間の計測を停止する要因。 */
export type HoldSource = 'pointer' | 'focus';

interface Live {
  toast: Toast;
  ms: number;
  timer: ReturnType<typeof setTimeout> | undefined;
  /** 計測の停止要因（滞在中のポインター・フォーカス）。空の場合のみ計測。 */
  holds: Set<HoldSource>;
  onClose?: (reason: CloseReason) => void;
}

let nextId = 1;
const live = new Map<number, Live>();
const state = $state<{ items: Toast[] }>({ items: [] });

function arm(entry: Live): void {
  clearTimeout(entry.timer);
  entry.timer = setTimeout(() => close(entry.toast.id, 'timeout'), entry.ms);
}

/** 通知を終了。終了済みの id は無視（onClose の重複呼出しなし）。 */
export function close(id: number, reason: CloseReason): void {
  const entry = live.get(id);
  if (!entry) return;
  live.delete(id);
  clearTimeout(entry.timer);
  state.items = state.items.filter((t) => t.id !== id);
  entry.onClose?.(reason);
}

/** 通知を表示。戻り値は通知の id。 */
export function show(options: ToastOptions): number {
  const kind = options.kind ?? 'info';
  const toast: Toast = { id: nextId++, kind, text: options.text, action: options.action ?? null };
  const ms = options.ms ?? (toast.action ? TOAST_MS.action : TOAST_MS[kind]);
  const entry: Live = { toast, ms, timer: undefined, holds: new Set(), onClose: options.onClose };
  live.set(toast.id, entry);
  state.items = [...state.items, toast];
  arm(entry);
  while (state.items.length > MAX_TOASTS) close(state.items[0].id, 'timeout');
  return toast.id;
}

/** 表示時間の計測を停止（ポインター・フォーカスの滞在中）。 */
export function hold(id: number, by: HoldSource): void {
  const entry = live.get(id);
  if (!entry) return;
  entry.holds.add(by);
  clearTimeout(entry.timer);
}

/** 停止要因を解除。全要因の解除後に計測を最初から再開。 */
export function release(id: number, by: HoldSource): void {
  const entry = live.get(id);
  if (!entry) return;
  entry.holds.delete(by);
  if (entry.holds.size === 0) arm(entry);
}

export const toasts = {
  /** 表示中の通知（古い順）。 */
  get items(): readonly Toast[] {
    return state.items;
  }
};

/** 操作結果の通知。 */
export const notify = (text: string): number => show({ text });
/** 操作失敗の通知。 */
export const notifyError = (text: string): number => show({ text, kind: 'error' });

/**
 * クリップボードへ複写し、結果を通知。
 * @param label 対象の名称（例：訳文）。通知文「訳文：コピー完了」に使用。
 */
export async function copyText(text: string, label: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    notify(t('toast:label_copied', { label }));
    return true;
  } catch (e) {
    notifyError(t('toast:copy_failed_error', { error: errorText(e) }));
    return false;
  }
}

/**
 * 実行済みの操作の結果通知（「取消」付き）。「取消」の押下で undo を実行し、失敗時は通知。
 * 操作自体は呼出し前に完了させる：通知の表示中にアプリを終了しても結果を維持。
 */
export function undoable(options: {
  text: string;
  undo: () => Promise<unknown>;
  /** 取消失敗時の通知文の前置き（例：復元失敗）。 */
  failure: string;
}): number {
  return show({
    text: options.text,
    action: t('toast:undo'),
    onClose: (reason) => {
      if (reason !== 'action') return;
      options.undo().catch((e) => notifyError(t('toast:label_detail', { label: options.failure, detail: errorText(e) })));
    }
  });
}
