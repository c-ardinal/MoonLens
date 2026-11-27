/**
 * 配置セット：表示中のウィンドウ翻訳一式の保存・復元（バックエンド：`layout_sets.rs`）。
 */

import type { AttachDock } from './attach';
import { t } from './i18n';
import type { TextPlacement } from './lens-text';

/** アプリの識別（再起動後も不変、`app_identity.rs`）。 */
export interface AppIdentity {
  key: string;
  name: string;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** 保存時の吸着先。 */
export interface AttachLayout {
  app: AppIdentity;
  title: string;
  dock: AttachDock;
  anchor: Rect;
}

export interface WindowLayout {
  rect: Rect;
  target_language: string | null;
  read_mode: 'subtitle' | 'ui';
  view: unknown;
  attach: AttachLayout | null;
}

export interface LayoutSet {
  id: string;
  name: string;
  /** 所属アプリ（吸着先の最多アプリ）。null：手動復元のみ。 */
  app: AppIdentity | null;
  /** 所属アプリの起動時に自動復元。 */
  auto_restore: boolean;
  windows: WindowLayout[];
  saved_at: number;
  /** 当該セットのウィンドウを表示中。 */
  active: boolean;
}

/** バックエンド → 全ウィンドウ：保存済みセットの変化。 */
export const LAYOUT_SETS_EVENT = 'moonlens://layout-sets';

/** 一覧の副表示：個数と所属アプリ。 */
export function setSummary(set: LayoutSet): string {
  const n = set.windows.length;
  return set.app
    ? t('layout-sets:summary_app', { n, name: set.app.name })
    : t('layout-sets:summary_no_app', { n });
}

/** 説明欄の文面。 */
export function setHelp(set: LayoutSet): string {
  const text = !set.app
    ? t('layout-sets:help_no_app')
    : set.auto_restore
      ? t('layout-sets:help_auto', { name: set.app.name })
      : t('layout-sets:help_manual', { name: set.app.name });
  return set.active ? t('layout-sets:help_active', { text }) : text;
}

/**
 * ウィンドウ翻訳 1 個の表示状態。配置セットへ保存し、復元時の初期状態に使用。
 * 項目の追加時は {@link parseView} の既定値で旧データを補完。
 */
export interface LensView {
  /** 常時読取。 */
  continuous: boolean;
  /** テキスト表示。 */
  text: boolean;
  /** テキスト表示の配置。 */
  placement: TextPlacement;
  /** 訳文の読上。 */
  speak: boolean;
}

const PLACEMENTS: readonly TextPlacement[] = ['inside', 'top', 'bottom', 'left', 'right', 'free'];

/** 保存値から表示状態を復元。欠落・不正な項目は `fallback` を使用。 */
export function parseView(raw: unknown, fallback: LensView): LensView {
  const v = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  const flag = (x: unknown, d: boolean) => (typeof x === 'boolean' ? x : d);
  return {
    continuous: flag(v.continuous, fallback.continuous),
    text: flag(v.text, fallback.text),
    placement: PLACEMENTS.includes(v.placement as TextPlacement)
      ? (v.placement as TextPlacement)
      : fallback.placement,
    speak: flag(v.speak, fallback.speak)
  };
}
