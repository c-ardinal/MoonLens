<script lang="ts">
  import {
    ChevronDown,
    ClipboardCopy,
    Eye,
    EyeOff,
    Languages,
    Maximize2,
    Minimize2,
    MousePointerBan,
    MousePointerClick,
    Plus,
    Radio,
    RefreshCw,
    TextSelect,
    X
  } from 'lucide-svelte';
  import { LANGUAGES, languageLabel } from '$lib/lens-overlay';

  /**
   * ウィンドウ翻訳の窓の操作列。配色は親要素の CSS 変数（--la-hover・--la-pressed）で指定。
   * compact：アイコンのみ（狭い窓向け、項目名は title へ表示）。幅不足時は横スクロール（ホイール対応）、
   * 「閉じる」は右端へ固定。
   */
  let {
    continuous,
    showOriginal,
    showText,
    sourceLanguage = null,
    targetLanguage,
    hasText = true,
    busy = false,
    compact = false,
    maximized = false,
    operate = false,
    operateKey = '',
    oncontinuous,
    onrefresh,
    onoriginal,
    onlanguage,
    oncopy,
    ontext,
    onexplain,
    onmaximize,
    onoperate,
    onadd,
    onclose
  }: {
    continuous: boolean;
    showOriginal: boolean;
    showText: boolean;
    sourceLanguage?: string | null;
    targetLanguage: string;
    /** 翻訳対象の文字あり（原文切替・コピー・説明の可否）。 */
    hasText?: boolean;
    busy?: boolean;
    compact?: boolean;
    /** 窓が最大化中（全画面翻訳）。 */
    maximized?: boolean;
    /** 操作モード（訳文の選択・コピー可）。false は透過モード（クリックは背面へ）。 */
    operate?: boolean;
    /** 操作モード切替のショートカット表記（title へ併記）。 */
    operateKey?: string;
    oncontinuous: () => void;
    /** 再取得。省略時は非表示（静止画上では範囲の再選択で代替）。 */
    onrefresh?: () => void;
    onoriginal: () => void;
    onlanguage: (code: string) => void;
    oncopy: () => void;
    ontext: () => void;
    onexplain: () => void;
    /** 最大化／元のサイズ。省略時は非表示。 */
    onmaximize?: () => void;
    /** 透過／操作モードの切替。省略時は非表示。 */
    onoperate?: () => void;
    /** ウィンドウの追加。省略時は非表示。 */
    onadd?: () => void;
    onclose: () => void;
  } = $props();

  /** compact 時の縦ホイールを横スクロールへ変換（幅不足で隠れた項目へ到達）。 */
  function wheel(e: WheelEvent) {
    const el = e.currentTarget as HTMLElement;
    if (!compact || el.scrollWidth <= el.clientWidth || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
    el.scrollLeft += e.deltaY;
    e.preventDefault();
  }
</script>

<div class="la" class:compact role="group" aria-label="ウィンドウ翻訳の操作" onwheel={wheel}>
  <button
    class="tb"
    aria-pressed={continuous}
    onclick={oncontinuous}
    title={continuous ? '常時翻訳：ON（画面変化時に再翻訳）' : '常時翻訳：OFF（クリックで画面変化へ追従）'}
  >
    <Radio class="h-4 w-4" /><span class="lb">常時</span>
  </button>
  {#if onoperate}
    <button
      class="tb"
      aria-pressed={operate}
      onclick={onoperate}
      title={(operate
        ? '操作モード：訳文の選択・コピー可（クリックで透過モードへ）'
        : '透過モード：窓上のクリックは背面へ（クリックで操作モードへ）') + (operateKey ? ` (${operateKey})` : '')}
    >
      {#if operate}<MousePointerClick class="h-4 w-4" />{:else}<MousePointerBan class="h-4 w-4" />{/if}<span
        class="lb">{operate ? '操作' : '透過'}</span
      >
    </button>
  {/if}
  {#if onrefresh}
    <button class="tb" onclick={onrefresh} disabled={busy} title="再取得">
      <RefreshCw class="h-4 w-4 {busy ? 'animate-spin' : ''}" /><span class="lb">再取得</span>
    </button>
  {/if}
  <button
    class="tb"
    aria-pressed={showOriginal}
    disabled={!hasText}
    onclick={onoriginal}
    title="原文／訳文の切替"
  >
    {#if showOriginal}<Eye class="h-4 w-4" /><span class="lb">訳文</span>{:else}<EyeOff class="h-4 w-4" /><span class="lb">原文</span>{/if}
  </button>
  <label class="tb lang" title="翻訳先言語">
    {#if !compact}<span class="opacity-70">{languageLabel(sourceLanguage) || '自動'} →</span>{/if}
    <select
      style="width: calc({languageLabel(targetLanguage).length}em + 6px)"
      value={targetLanguage}
      onchange={(e) => onlanguage((e.currentTarget as HTMLSelectElement).value)}
    >
      {#if !LANGUAGES.some((l) => l.code === targetLanguage)}
        <option value={targetLanguage}>{languageLabel(targetLanguage)}</option>
      {/if}
      {#each LANGUAGES as l (l.code)}
        <option value={l.code}>{l.label}</option>
      {/each}
    </select>
    <ChevronDown class="h-3 w-3 opacity-70" />
  </label>
  <button class="tb" disabled={!hasText} onclick={oncopy} title="コピー (Ctrl+C)">
    <ClipboardCopy class="h-4 w-4" /><span class="lb">コピー</span>
  </button>
  <button class="tb" aria-pressed={showText} onclick={ontext} title="テキスト表示">
    <TextSelect class="h-4 w-4" /><span class="lb">テキスト</span>
  </button>
  <button class="tb" disabled={!hasText} onclick={onexplain} title="内容説明">
    <Languages class="h-4 w-4" /><span class="lb">説明</span>
  </button>
  {#if onmaximize}
    <button
      class="tb"
      onclick={onmaximize}
      title={maximized ? '元のサイズ' : '最大化（全画面翻訳）'}
    >
      {#if maximized}<Minimize2 class="h-4 w-4" />{:else}<Maximize2 class="h-4 w-4" />{/if}<span class="lb"
        >{maximized ? '元のサイズ' : '最大化'}</span
      >
    </button>
  {/if}
  {#if onadd}
    <button class="tb" onclick={onadd} title="ウィンドウ追加（同一サイズで隣へ配置）">
      <Plus class="h-4 w-4" /><span class="lb">追加</span>
    </button>
  {/if}
  <button class="tb close" onclick={onclose} title="閉じる (Esc)"><X class="h-4 w-4" /></button>
</div>

<style>
  .la {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .la.compact {
    gap: 1px;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .la.compact::-webkit-scrollbar {
    display: none;
  }
  .la.compact .tb {
    flex: none;
  }
  /* 幅不足時も「閉じる」は常時表示 */
  .la.compact .close {
    position: sticky;
    right: 0;
    background: var(--la-bg, #18181c);
  }
  .tb {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 10px;
    padding: 6px 10px;
    font-size: 13px;
    white-space: nowrap;
    transition: background 120ms;
  }
  .compact .tb {
    border-radius: 6px;
    padding: 4px;
    font-size: 11px;
  }
  .compact .lb {
    display: none;
  }
  .tb:hover:not(:disabled) {
    background: var(--la-hover, rgba(255, 255, 255, 0.1));
  }
  .tb[aria-pressed='true'] {
    background: var(--la-pressed, rgba(139, 92, 246, 0.35));
  }
  .tb:disabled {
    opacity: 0.4;
  }
  /* 独自描画（WebKitGTK の既定描画は明背景で文字が不可視） */
  .lang select {
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    color: inherit;
    border: none;
    outline: none;
    font: inherit;
    cursor: pointer;
    padding: 0 2px;
  }
  .lang select option {
    color: #111;
  }
</style>
