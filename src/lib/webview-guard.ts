/**
 * 組込ブラウザ由来の既定動作（リンク・画像のドラッグ、標準コンテキストメニュー）の
 * 代替規則。判定のみ担当し、表示は `ContextMenu.svelte` が担当。
 */
import { invoke } from '@tauri-apps/api/core';

export type EditAction = 'cut' | 'copy' | 'paste' | 'selectAll';

export interface MenuItem {
  action: EditAction;
  label: string;
  enabled: boolean;
}

type TextField = HTMLInputElement | HTMLTextAreaElement;

/** 文字入力が可能な input 種別（チェックボックス・スライダー等は対象外）。 */
const TEXT_INPUT_TYPES = new Set(['text', 'search', 'url', 'tel', 'email', 'password', 'number']);

function elementOf(target: EventTarget | null): Element | null {
  if (target instanceof Element) return target;
  // 選択文字列のドラッグ等：イベント発生元がテキストノード。
  return target instanceof Node ? target.parentElement : null;
}

function isTextField(el: Element): el is TextField {
  if (el instanceof HTMLTextAreaElement) return true;
  return el instanceof HTMLInputElement && TEXT_INPUT_TYPES.has(el.type);
}

/** 操作対象を含む編集欄（入力欄・contenteditable）。該当なしは null。 */
export function editableOf(target: EventTarget | null): HTMLElement | null {
  const el = elementOf(target)?.closest<HTMLElement>(
    'input, textarea, [contenteditable]:not([contenteditable="false"])'
  );
  if (!el) return null;
  if (el instanceof HTMLInputElement && !isTextField(el)) return null;
  return el;
}

/**
 * ドラッグ開始の可否。編集欄内の選択文字列の移動のみ許可し、リンク・画像・
 * 表示文字列のドラッグ（デスクトップへのショートカット作成等）は抑止。
 */
export function allowsDrag(target: EventTarget | null): boolean {
  return editableOf(target) !== null;
}

function fieldHasSelection(field: TextField): boolean {
  try {
    const { selectionStart, selectionEnd } = field;
    // email・number は選択範囲の取得が不可（null）：選択有と仮定。
    if (selectionStart === null || selectionEnd === null) return field.value.length > 0;
    return selectionStart !== selectionEnd;
  } catch {
    return field.value.length > 0;
  }
}

/**
 * 右クリック位置に応じたメニュー項目。空配列はメニュー非表示
 * （編集欄でも選択文字列でもない箇所）。
 */
export function menuItemsFor(target: EventTarget | null, selectedText: string): MenuItem[] {
  const editable = editableOf(target);
  if (!editable) {
    return selectedText.trim() ? [{ action: 'copy', label: 'コピー', enabled: true }] : [];
  }
  let hasSelection: boolean;
  let hasContent: boolean;
  let writable: boolean;
  if (isTextField(editable)) {
    if (editable.disabled) return [];
    hasSelection = fieldHasSelection(editable);
    hasContent = editable.value.length > 0;
    writable = !editable.readOnly;
  } else {
    hasSelection = selectedText.length > 0;
    hasContent = (editable.textContent ?? '').length > 0;
    writable = true;
  }
  return [
    { action: 'cut', label: '切取', enabled: writable && hasSelection },
    { action: 'copy', label: 'コピー', enabled: hasSelection },
    { action: 'paste', label: '貼付', enabled: writable },
    { action: 'selectAll', label: '全選択', enabled: hasContent }
  ];
}

async function readClipboardText(): Promise<string> {
  try {
    // WebView 標準の読取は許可確認が発生するため、アプリ側の機能を優先。
    return (await invoke<string>('plugin:clipboard-manager|read_text')) ?? '';
  } catch {
    try {
      return await navigator.clipboard.readText();
    } catch {
      return '';
    }
  }
}

/** メニュー項目の実行。`editable` は右クリック時点の編集欄（無ければ null）。 */
export async function runEditAction(action: EditAction, editable: HTMLElement | null) {
  if (action === 'paste') {
    const text = await readClipboardText();
    if (!text || !editable) return;
    editable.focus();
    // insertText：入力イベントが発生し、取消履歴（Ctrl+Z）も維持。
    document.execCommand('insertText', false, text);
    return;
  }
  if (action === 'selectAll') {
    if (editable && isTextField(editable)) editable.select();
    else document.execCommand('selectAll');
    return;
  }
  document.execCommand(action);
}
