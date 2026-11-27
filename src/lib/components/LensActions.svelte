<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import {
    ChevronDown,
    ClipboardCopy,
    EllipsisVertical,
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
  import { fitCount } from '$lib/lens-bar';

  /**
   * ウィンドウ翻訳のタイトルバー。左端：ウィンドウ追加・見出し（lead）、右端：最大化・閉じる（常時表示）。
   * 中間の操作項目は幅へ収納可能な数のみ表示し、残余は縦メニューへ収納。
   * 配色は親要素の CSS 変数（--la-hover・--la-pressed・--la-bg）で指定。
   */
  let {
    continuous,
    showOriginal,
    showText,
    targetLanguage,
    hasText = true,
    busy = false,
    maximized = false,
    operate = false,
    operateKey = '',
    menuOpen = $bindable(false),
    lead,
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
    targetLanguage: string;
    /** 翻訳対象の文字あり（原文切替・コピー・説明の可否）。 */
    hasText?: boolean;
    busy?: boolean;
    /** ウィンドウが最大化中（全画面翻訳）。 */
    maximized?: boolean;
    /** 操作モード（訳文の選択・コピー可）。false は透過モード（クリックは背面へ）。 */
    operate?: boolean;
    /** 操作モード切替のショートカット表記（title へ併記）。 */
    operateKey?: string;
    /** 縦メニューの表示中（表示中はウィンドウ本体でのマウス操作の受付が必要）。 */
    menuOpen?: boolean;
    /** 見出し（ウィンドウ追加ボタンの右隣）。 */
    lead?: Snippet;
    oncontinuous: () => void;
    onrefresh: () => void;
    onoriginal: () => void;
    onlanguage: (code: string) => void;
    oncopy: () => void;
    ontext: () => void;
    onexplain: () => void;
    onmaximize: () => void;
    onoperate: () => void;
    onadd: () => void;
    onclose: () => void;
  } = $props();

  interface Action {
    key: string;
    icon: typeof X;
    label: string;
    title: string;
    pressed?: boolean;
    disabled?: boolean;
    spin?: boolean;
    run: () => void;
  }

  /** 操作項目（表示順 = 収納の優先順：後方から縦メニューへ収納）。 */
  const actions = $derived<(Action | 'language')[]>([
    {
      key: 'continuous',
      icon: Radio,
      label: '常時翻訳',
      title: continuous
        ? '常時翻訳：ON（画面変化時に再翻訳）'
        : '常時翻訳：OFF（クリックで画面変化へ追従）',
      pressed: continuous,
      run: oncontinuous
    },
    {
      key: 'operate',
      icon: operate ? MousePointerClick : MousePointerBan,
      label: operate ? '操作モード' : '透過モード',
      title:
        (operate
          ? '操作モード：訳文の選択・コピー可（クリックで透過モードへ）'
          : '透過モード：ウィンドウ上のクリックは背面へ（クリックで操作モードへ）') +
        (operateKey ? ` (${operateKey})` : ''),
      pressed: operate,
      run: onoperate
    },
    {
      key: 'refresh',
      icon: RefreshCw,
      label: '再取得',
      title: '再取得',
      disabled: busy,
      spin: busy,
      run: onrefresh
    },
    {
      key: 'original',
      icon: showOriginal ? Eye : EyeOff,
      label: showOriginal ? '訳文表示' : '原文表示',
      title: '原文／訳文の切替',
      pressed: showOriginal,
      disabled: !hasText,
      run: onoriginal
    },
    'language',
    {
      key: 'copy',
      icon: ClipboardCopy,
      label: 'コピー',
      title: 'コピー (Ctrl+C)',
      disabled: !hasText,
      run: oncopy
    },
    {
      key: 'text',
      icon: TextSelect,
      label: 'テキスト表示',
      title: 'テキスト表示',
      pressed: showText,
      run: ontext
    },
    {
      key: 'explain',
      icon: Languages,
      label: '内容説明',
      title: '内容説明',
      disabled: !hasText,
      run: onexplain
    }
  ]);

  /** 項目間隔・縦メニューボタンの占有幅（px、CSS の .fit の gap・.tb の寸法・.bar の gap と同値）。 */
  const GAP = 0;
  const MORE = 24 + 4;

  let fitEl: HTMLDivElement;
  /** バーへ表示する項目数（先頭から）。 */
  let shown = $state(Number.MAX_SAFE_INTEGER);
  const overflow = $derived(shown < actions.length);

  function measure() {
    if (!fitEl) return;
    const widths = [...fitEl.querySelectorAll<HTMLElement>(':scope > [data-item]')].map(
      (el) => el.offsetWidth
    );
    // 縦メニューボタンの表示中は、その占有幅も収納可能な幅へ算入（表示有無による振動の防止）
    const available = fitEl.clientWidth + (overflow ? MORE : 0);
    shown = fitCount(widths, GAP, available, MORE);
  }

  onMount(() => {
    const ro = new ResizeObserver(measure);
    ro.observe(fitEl);
    return () => ro.disconnect();
  });
  // 項目の幅が変化する更新（言語名など）の後に再計測
  $effect(() => {
    void actions;
    void targetLanguage;
    measure();
  });
  $effect(() => {
    if (!overflow) menuOpen = false;
  });

  function outside(e: PointerEvent) {
    if (menuOpen && !(e.target as Element | null)?.closest?.('.menu, .more')) menuOpen = false;
  }

  function run(a: Action, fromMenu: boolean) {
    a.run();
    if (fromMenu) menuOpen = false;
  }
</script>

<svelte:window onpointerdown={outside} onblur={() => (menuOpen = false)} />

{#snippet language(inMenu: boolean)}
  <label class={inMenu ? 'mi lang' : 'tb lang'} title="翻訳先言語">
    {#if inMenu}<Languages class="h-4 w-4" /><span class="lb">翻訳先</span>{/if}
    <span class="sel">
      <select
        aria-label="翻訳先言語"
        style="width: calc({languageLabel(targetLanguage).length}em + 2px)"
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
    </span>
  </label>
{/snippet}

<button class="tb" onclick={onadd} title="ウィンドウ追加（同一サイズで隣へ配置）">
  <Plus class="h-4 w-4" />
</button>
{@render lead?.()}

<div class="fit" bind:this={fitEl} role="group" aria-label="ウィンドウ翻訳の操作" data-tauri-drag-region>
  {#each actions as a, i (a === 'language' ? a : a.key)}
    {#if a === 'language'}
      <span class="item" class:off={i >= shown} data-item>{@render language(false)}</span>
    {:else}
      <button
        class="tb"
        class:off={i >= shown}
        data-item
        aria-pressed={a.pressed}
        disabled={a.disabled}
        onclick={() => run(a, false)}
        title={a.title}
      >
        <a.icon class="h-4 w-4 {a.spin ? 'animate-spin' : ''}" />
      </button>
    {/if}
  {/each}
</div>

{#if overflow}
  <button
    class="tb more"
    aria-haspopup="menu"
    aria-expanded={menuOpen}
    aria-pressed={menuOpen}
    onclick={() => (menuOpen = !menuOpen)}
    title="他の操作"
  >
    <EllipsisVertical class="h-4 w-4" />
  </button>
{/if}
<button class="tb" onclick={onmaximize} title={maximized ? '元のサイズ' : '最大化（全画面翻訳）'}>
  {#if maximized}<Minimize2 class="h-4 w-4" />{:else}<Maximize2 class="h-4 w-4" />{/if}
</button>
<button class="tb" onclick={onclose} title="閉じる (Esc)"><X class="h-4 w-4" /></button>

{#if menuOpen && overflow}
  <div class="menu" role="menu" aria-label="他の操作">
    {#each actions.slice(shown) as a (a === 'language' ? a : a.key)}
      {#if a === 'language'}
        {@render language(true)}
      {:else}
        <button
          class="mi"
          role={a.pressed === undefined ? 'menuitem' : 'menuitemcheckbox'}
          aria-checked={a.pressed}
          disabled={a.disabled}
          onclick={() => run(a, true)}
          title={a.title}
        >
          <a.icon class="h-4 w-4 {a.spin ? 'animate-spin' : ''}" /><span class="lb">{a.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  /* 収納可能な項目の表示域。余白はウィンドウのドラッグ領域。 */
  .fit {
    position: relative;
    flex: 1 1 0;
    min-width: 0;
    align-self: stretch;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    overflow: hidden;
  }
  .item {
    flex: none;
    display: inline-flex;
  }
  /* 縦メニューへ収納中の項目：幅の計測用に配置は維持し、表示・操作の対象から除外 */
  .off {
    position: absolute;
    left: 0;
    top: 0;
    visibility: hidden;
    pointer-events: none;
  }
  .tb {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border-radius: 6px;
    padding: 4px;
    font-size: 11px;
    white-space: nowrap;
    transition: background 120ms;
  }
  .tb:hover:not(:disabled),
  .mi:hover:not(:disabled) {
    background: var(--la-hover, rgba(255, 255, 255, 0.1));
  }
  .tb[aria-pressed='true'],
  .mi[aria-checked='true'] {
    background: var(--la-pressed, color-mix(in srgb, var(--ml-accent) 35%, transparent));
  }
  .tb:disabled,
  .mi:disabled {
    opacity: 0.4;
  }
  .sel {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  /* 独自描画。選択肢は文字色・背景色を対で指定（背景色なしでは OS の配色設定の既定色となり、
     暗色設定で文字が判読不可） */
  .lang select {
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    color: inherit;
    border: none;
    outline: none;
    font: inherit;
    cursor: pointer;
    padding: 0;
  }
  .lang select option {
    color: #111;
    background-color: #fff;
  }
  /* 縦メニュー：タイトルバー直下の右端。ウィンドウの高さ不足時は内部スクロール。 */
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 4px;
    z-index: 10;
    display: flex;
    flex-direction: column;
    min-width: min(148px, calc(100vw - 8px));
    max-width: calc(100vw - 8px);
    /* ウィンドウの高さ − タイトルバー（32px）− 上下の余白 */
    max-height: calc(100vh - 40px);
    box-sizing: border-box;
    overflow-y: auto;
    border-radius: 8px;
    padding: 4px;
    background: var(--la-bg, rgb(24, 24, 28));
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.4);
  }
  .mi {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 12px;
    text-align: left;
    white-space: nowrap;
    transition: background 120ms;
  }
  .mi :global(svg) {
    flex: none;
  }
  .mi.lang .sel {
    margin-left: auto;
  }
</style>
