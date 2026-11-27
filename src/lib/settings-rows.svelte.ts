/**
 * 設定画面の行選択：選択帯の対象行と、説明欄へ出す説明文の共有状態。
 * 設定項目・モデル・データの各行（SettingRow）が同一の選択を共有。
 */
import { getContext, setContext } from 'svelte';

export interface RowLink {
  href: string;
  label: string;
}

export class RowSelection {
  active = $state<string | null>(null);
  help = $state<string | null>(null);
  link = $state<RowLink | null>(null);

  activate(id: string, help?: string | null, link?: RowLink | null) {
    this.active = id;
    this.help = help ?? null;
    this.link = link ?? null;
  }

  clear() {
    this.active = null;
    this.help = null;
    this.link = null;
  }
}

const KEY = Symbol('settings-rows');

export function provideRowSelection(): RowSelection {
  return setContext(KEY, new RowSelection());
}

/** 設定画面外（単体テスト等）では独立の選択状態。 */
export function useRowSelection(): RowSelection {
  return getContext<RowSelection | undefined>(KEY) ?? new RowSelection();
}
