<script lang="ts">
  import { t, textEm } from '$lib/i18n';
  import { ChevronDown, X } from 'lucide-svelte';
  import { TEXT_PLACEMENTS, parsePlacement, type TextPlacement, type TextView } from '$lib/lens-text';

  /**
   * ウィンドウ翻訳のテキスト表示（訳文・原文・内容説明）。ウィンドウ内のパネルとウィンドウ外表示用ウィンドウで共用。
   */
  let {
    view,
    drag = false,
    onplacement,
    onclose
  }: {
    view: TextView;
    /** 見出しのドラッグでウィンドウを移動（ウィンドウ外の自由配置）。 */
    drag?: boolean;
    onplacement: (p: TextPlacement) => void;
    onclose: () => void;
  } = $props();

  const hasText = $derived(!!(view.source?.trim() || view.translation.trim()));
  const placementLabel = $derived(
    t(TEXT_PLACEMENTS.find((p) => p.value === view.placement)?.label ?? '')
  );
</script>

<div class="lt">
  <header class="head" class:drag data-tauri-drag-region={drag ? '' : undefined}>
    <span class="name" data-tauri-drag-region={drag ? '' : undefined}>{t('テキスト')}</span>
    <label class="place" title={t('表示位置')}>
      <select
        aria-label={t('表示位置')}
        style="width: calc({textEm(placementLabel)}em + 6px)"
        value={view.placement}
        onchange={(e) => onplacement(parsePlacement((e.currentTarget as HTMLSelectElement).value))}
      >
        {#each TEXT_PLACEMENTS as p (p.value)}
          <option value={p.value}>{t(p.label)}</option>
        {/each}
      </select>
      <ChevronDown class="h-3 w-3 opacity-70" />
    </label>
    <button class="close" onclick={onclose} title={t('閉じる')}><X class="h-3.5 w-3.5" /></button>
  </header>
  <div class="content">
    {#if view.explanation}
      <section class="explain">{view.explanation}</section>
    {/if}
    {#if view.blocks.length > 0}
      {#each view.blocks as b, i (i)}
        {#if b.same_language}
          <p class="para">{b.source}</p>
        {:else}
          <p class="para tight">{b.translation ?? t('（翻訳中）')}</p>
          <p class="para src">{b.source}</p>
        {/if}
      {/each}
    {:else if hasText}
      <p class="para tight">{view.translation}</p>
      {#if view.source}<p class="para src">{view.source}</p>{/if}
    {:else}
      <p class="none">{t('文字なし')}</p>
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
  /* 独自描画。選択肢は文字色・背景色を対で指定（背景色なしでは OS の配色設定の既定色となり、
     暗色設定で文字が判読不可） */
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
    background-color: #fff;
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
