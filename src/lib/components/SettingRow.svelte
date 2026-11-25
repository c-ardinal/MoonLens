<script lang="ts">
  import type { Snippet } from 'svelte';
  import { useRowSelection, type RowLink } from '$lib/settings-rows.svelte';

  interface Props {
    /** 行の識別子（設定項目は path、モデルは model:<id> 等）。 */
    id: string;
    /** 選択時に説明窓へ表示する説明文。 */
    help?: string | null;
    link?: RowLink | null;
    children: Snippet;
  }
  let { id, help = null, link = null, children }: Props = $props();
  const rows = useRowSelection();
  const active = $derived(rows.active === id);
</script>

<!-- 選択帯：行と同じ矩形（斜めは分類一覧・見出しのみ） -->
<div
  class="row"
  class:active
  data-row={id}
  role="presentation"
  onmouseenter={() => rows.activate(id, help, link)}
  onfocusin={() => rows.activate(id, help, link)}
>
  {#if active}
    <div class="band" aria-hidden="true"></div>
  {/if}
  {@render children()}
</div>

<style>
  .row {
    position: relative;
    border-bottom: 2px dotted var(--ml-px-dim);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .band {
    position: absolute;
    /* 枠の内側へ収める（枠線との重複なし） */
    inset: -2px -20px 0 -20px;
    background: var(--ml-px-amber);
    box-shadow: 0 0 0 2px var(--ml-px-bone);
    pointer-events: none;
  }
  .row > :global(:not(.band)) {
    position: relative;
  }
  .row.active :global(label),
  .row.active :global(.value-text),
  .row.active :global(.row-label),
  .row.active :global(.row-value),
  .row.active :global(.sprompt),
  .row.active :global(.sprompt.warn > p) {
    color: var(--ml-px-black);
  }
  /* 琥珀の帯上：ラジオ・推奨札・ボタンを黒系へ反転 */
  .row.active :global(.sradio) {
    border-color: var(--ml-px-black);
    background: var(--ml-px-amber);
    box-shadow: inset 0 0 0 4px var(--ml-px-amber);
  }
  .row.active :global(.spick[aria-checked='true'] .sradio) {
    background: var(--ml-px-black);
  }
  .row.active :global(.stag.rec) {
    background: var(--ml-px-black);
    color: var(--ml-px-amber);
  }
  /* 琥珀の帯上のスライダー：つまみ・溝を黒系へ反転 */
  .row.active :global(input[type='range']::-webkit-slider-thumb) {
    background: var(--ml-px-black);
    box-shadow: none;
  }
  .row.active :global(input[type='range']::-webkit-slider-runnable-track) {
    background: var(--ml-px-faint);
  }
</style>
