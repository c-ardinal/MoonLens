<script lang="ts">
  import { ChevronDown, RotateCw, X } from 'lucide-svelte';
  import { TEXT_PLACEMENTS, parsePlacement, type TextPlacement, type TextView } from '$lib/lens-text';

  /**
   * ウィンドウ翻訳のテキスト表示（訳文・原文・内容説明）。窓内のパネルと窓外表示用ウィンドウで共用。
   */
  let {
    view,
    drag = false,
    onplacement,
    onexplain,
    onclose
  }: {
    view: TextView;
    /** 見出しのドラッグでウィンドウを移動（窓外の自由配置）。 */
    drag?: boolean;
    onplacement: (p: TextPlacement) => void;
    /** 内容説明の再試行。 */
    onexplain: () => void;
    onclose: () => void;
  } = $props();

  const hasText = $derived(!!(view.source?.trim() || view.translation.trim()));
  const placementLabel = $derived(
    TEXT_PLACEMENTS.find((p) => p.value === view.placement)?.label ?? ''
  );
</script>

<div class="lt">
  <header class="head" class:drag data-tauri-drag-region={drag ? '' : undefined}>
    <span class="name" data-tauri-drag-region={drag ? '' : undefined}>テキスト</span>
    <label class="place" title="表示位置">
      <select
        aria-label="表示位置"
        style="width: calc({placementLabel.length}em + 6px)"
        value={view.placement}
        onchange={(e) => onplacement(parsePlacement((e.currentTarget as HTMLSelectElement).value))}
      >
        {#each TEXT_PLACEMENTS as p (p.value)}
          <option value={p.value}>{p.label}</option>
        {/each}
      </select>
      <ChevronDown class="h-3 w-3 opacity-70" />
    </label>
    <button class="close" onclick={onclose} title="閉じる"><X class="h-3.5 w-3.5" /></button>
  </header>
  <div class="content">
    {#if view.explanation?.status === 'pending'}
      <section class="explain" role="status">説明を生成中…</section>
    {:else if view.explanation?.status === 'done'}
      <section class="explain">{view.explanation.text}</section>
    {:else if view.explanation?.status === 'failed'}
      <section class="explain failed" role="alert">
        <span>説明の生成に失敗：{view.explanation.message}</span>
        <button class="retry" onclick={onexplain}>
          <RotateCw class="h-3 w-3" />再試行
        </button>
      </section>
    {/if}
    {#if view.blocks.length > 0}
      {#each view.blocks as b, i (i)}
        {#if b.same_language}
          <p class="para">{b.source}</p>
        {:else}
          <p class="para tight">{b.translation ?? '（翻訳中）'}</p>
          <p class="para src">{b.source}</p>
        {/if}
      {/each}
    {:else if hasText}
      <p class="para tight">{view.translation}</p>
      {#if view.source}<p class="para src">{view.source}</p>{/if}
    {:else}
      <p class="none">文字なし</p>
    {/if}
  </div>
</div>

<style>
  .lt {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-height: 0;
    color: #f4f4f5;
  }
  .head {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px 4px 12px;
    user-select: none;
  }
  .head.drag {
    cursor: move;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .place {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border-radius: 6px;
    padding: 4px 4px 4px 8px;
    font-size: 11px;
    cursor: pointer;
  }
  .place:hover,
  .close:hover {
    background: rgba(255, 255, 255, 0.12);
  }
  /* 独自描画（WebKitGTK の既定描画は明背景で文字が不可視） */
  .place select {
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
  .place select option {
    color: #111;
  }
  .close {
    flex: none;
    border-radius: 6px;
    padding: 4px;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 12px 12px;
    font-size: 14px;
    line-height: 1.625;
    user-select: text;
    -webkit-user-select: text;
  }
  .explain {
    margin-bottom: 12px;
    border-radius: 6px;
    padding: 8px;
    white-space: pre-wrap;
    background: rgba(255, 255, 255, 0.05);
  }
  .explain.failed {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .retry {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 6px;
    padding: 2px 8px;
    font-size: 12px;
    white-space: nowrap;
  }
  .retry:hover {
    background: rgba(255, 255, 255, 0.12);
  }
  .para {
    margin: 0 0 12px;
    white-space: pre-wrap;
  }
  .para.tight {
    margin-bottom: 4px;
  }
  .para.src {
    font-size: 12px;
    opacity: 0.6;
  }
  .none {
    margin: 0;
    opacity: 0.6;
  }
</style>
