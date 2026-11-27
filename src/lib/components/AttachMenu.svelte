<script lang="ts">
  import { onMount } from 'svelte';
  import { Cast, Check, Magnet, RefreshCw, Unlink } from 'lucide-svelte';
  import { attachWindow, detachWindow, listAttachTargets } from '$lib/ipc';
  import {
    isUnder,
    targetLines,
    targetName,
    type AttachCandidate,
    type AttachDock,
    type Attachment
  } from '$lib/attach';
  import { errorText } from '$lib/errors';
  import type { StreamState } from '$lib/stream';

  /**
   * 吸着メニュー：他アプリのウィンドウ一覧から吸着先を選択、吸着中は配置の変更と解除。
   * 配色は親要素の CSS 変数（--la-hover・--la-pressed・--la-bg）で指定。
   */
  let {
    label,
    attachment,
    docks,
    stream,
    onstream,
    onclose
  }: {
    /** 吸着するウィンドウ（自ウィンドウ）のラベル。 */
    label: string;
    attachment: Attachment;
    /** 選択可能な配置（先頭：新規吸着時の配置）。 */
    docks: { value: AttachDock; label: string; title: string }[];
    /** 吸着先の配信用ウィンドウの状態（非対応環境では項目なし）。 */
    stream?: StreamState;
    /** 配信用ウィンドウの生成（true）・終了。 */
    onstream?: (on: boolean) => Promise<void>;
    onclose: () => void;
  } = $props();

  let candidates = $state<AttachCandidate[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    try {
      candidates = await listAttachTargets(label);
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }
  onMount(load);

  async function run(action: () => Promise<void>, close: boolean) {
    error = null;
    try {
      await action();
      if (close) onclose();
    } catch (e) {
      error = errorText(e);
      // 候補が閉鎖済みの場合に備え、一覧を再取得
      load();
    }
  }
  const attach = (c: AttachCandidate) =>
    run(() => attachWindow(label, c.id, docks[0].value), true);
  const setDock = (dock: AttachDock) => {
    const t = attachment.target;
    if (t) run(() => attachWindow(label, t.id, dock), false);
  };
  const detach = () => run(() => detachWindow(label), true);
  const toggleStream = () => {
    const next = !stream?.active;
    if (onstream) run(() => onstream(next), false);
  };
</script>

<div class="am" role="dialog" aria-label="吸着">
  {#if attachment.target}
    <div class="cur">
      <Magnet class="h-4 w-4" />
      <span class="nm" title={attachment.target.title}>{targetName(attachment.target)}</span>
      <button class="ib" onclick={detach} title="吸着解除（現在位置へ残留）">
        <Unlink class="h-4 w-4" />
      </button>
    </div>
    <div class="seg" role="radiogroup" aria-label="配置">
      {#each docks as d (d.value)}
        <button
          role="radio"
          aria-checked={attachment.dock === d.value}
          onclick={() => setDock(d.value)}
          title={d.title}>{d.label}</button
        >
      {/each}
    </div>
    {#if stream?.supported}
      <button
        class="sw"
        role="switch"
        aria-checked={stream.active}
        onclick={toggleStream}
        title="吸着先と訳文を合成した取込専用ウィンドウを生成（配信・録画ツールのウィンドウ取込用）"
      >
        <Cast class="h-4 w-4" />
        <span class="nm">配信用ウィンドウ</span>
        <span class="st">{stream.active ? '稼働中' : '停止中'}</span>
      </button>
      {#if stream.active && stream.title}
        <p class="note nm" title={stream.title}>取込対象：{stream.title}</p>
        <p class="note nm" title="OBS のウィンドウキャプチャでは、キャプチャ方法に「Windows 10 (1903 以降)」を指定">
          OBS の方式：Windows 10 (1903 以降)
        </p>
      {/if}
    {/if}
  {/if}

  <div class="hd">
    <span>吸着先</span>
    <button class="ib" onclick={load} disabled={loading} title="一覧更新">
      <RefreshCw class="h-4 w-4 {loading ? 'animate-spin' : ''}" />
    </button>
  </div>
  {#if error}<p class="note err">{error}</p>{/if}
  <div class="list">
    {#each candidates as c, i (c.id)}
      {@const lines = targetLines(c)}
      {@const current = attachment.target?.id === c.id}
      <button class="row" aria-current={current} onclick={() => attach(c)} title={c.title || c.app}>
        <span class="l1">
          <span class="nm">{lines.name}</span>
          {#if current}
            <Check class="h-4 w-4" />
          {:else if isUnder(candidates, i)}
            <span class="tag">直下</span>
          {/if}
        </span>
        {#if lines.detail}<span class="dt">{lines.detail}</span>{/if}
      </button>
    {/each}
    {#if !loading && candidates.length === 0}
      <p class="note">吸着可能なウィンドウなし</p>
    {/if}
  </div>
</div>

<style>
  /* タイトルバー直下の左端。ウィンドウの高さ不足時は一覧を内部スクロール。 */
  .am {
    position: absolute;
    top: calc(100% + 4px);
    left: 4px;
    z-index: 10;
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: min(280px, calc(100vw - 8px));
    /* ウィンドウの高さ − タイトルバー（32px）− 上下の余白 */
    max-height: calc(100vh - 40px);
    box-sizing: border-box;
    border-radius: 8px;
    padding: 4px;
    font-size: 12px;
    background: var(--la-bg, rgb(24, 24, 28));
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.4);
  }
  .cur,
  .hd {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 0 0 8px;
  }
  .cur {
    color: var(--ml-accent, #a78bfa);
  }
  .hd {
    font-size: 11px;
    opacity: 0.75;
  }
  .cur .nm,
  .hd span {
    flex: 1 1 0;
    min-width: 0;
  }
  .nm,
  .dt {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ib {
    flex: none;
    display: inline-flex;
    border-radius: 6px;
    padding: 4px;
    color: #f4f4f5;
    transition: background 120ms;
  }
  .ib:disabled {
    opacity: 0.4;
  }
  .seg {
    flex: none;
    display: flex;
    gap: 4px;
  }
  .seg button {
    flex: 1 1 0;
    border-radius: 6px;
    padding: 4px 8px;
    white-space: nowrap;
    transition: background 120ms;
  }
  .sw {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 6px;
    padding: 4px 8px;
    text-align: left;
    transition: background 120ms;
  }
  .sw .nm {
    flex: 1 1 0;
    min-width: 0;
  }
  .sw .st {
    flex: none;
    font-size: 11px;
    opacity: 0.75;
  }
  .sw[aria-checked='true'] {
    background: var(--la-pressed, rgba(139, 92, 246, 0.35));
  }
  .sw[aria-checked='true'] .st {
    opacity: 1;
    color: var(--ml-accent, #a78bfa);
  }
  .sw:hover,
  .ib:hover:not(:disabled),
  .seg button:hover,
  .row:hover {
    background: var(--la-hover, rgba(255, 255, 255, 0.1));
  }
  .seg button[aria-checked='true'],
  .row[aria-current='true'] {
    background: var(--la-pressed, rgba(139, 92, 246, 0.35));
  }
  .list {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }
  .row {
    flex: none;
    display: flex;
    flex-direction: column;
    border-radius: 6px;
    padding: 4px 8px;
    text-align: left;
    transition: background 120ms;
  }
  .l1 {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .l1 .nm {
    flex: 0 1 auto;
    min-width: 0;
  }
  .l1 :global(svg) {
    flex: none;
  }
  .tag {
    flex: none;
    border-radius: 4px;
    padding: 0 4px;
    font-size: 11px;
    color: var(--ml-accent, #a78bfa);
    box-shadow: inset 0 0 0 1px currentColor;
  }
  .dt {
    font-size: 11px;
    opacity: 0.6;
  }
  .note {
    margin: 0;
    padding: 4px 8px;
    font-size: 11px;
    opacity: 0.75;
  }
  .note.err {
    opacity: 1;
    color: #fca5a5;
  }
</style>
