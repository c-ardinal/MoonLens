<script lang="ts">
  /**
   * One labelled setting line. Label + helper text live on the left;
   * the actual control is rendered through the `control` snippet on
   * the right. A divider is automatically placed below unless `last`.
   */
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    description?: string;
    /** When true, omits the bottom divider so the row sits flush with
     *  its parent's bottom edge. Set on the final row of each group. */
    last?: boolean;
    /** When true, lays the control beneath the label rather than to
     *  the right — used for wide controls like sliders or hotkey
     *  pickers that need full row width. */
    stacked?: boolean;
    control: Snippet;
  }

  let { label, description, last = false, stacked = false, control }: Props =
    $props();
</script>

<div class="setting-row" class:last class:stacked>
  <div class="row-text">
    <div class="row-label">{label}</div>
    {#if description}
      <div class="row-desc">{description}</div>
    {/if}
  </div>
  <div class="row-control">
    {@render control()}
  </div>
</div>

<style>
  .setting-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 16px;
    align-items: center;
    padding: 14px 16px;
    border-bottom: 1px solid var(--line);
  }
  .setting-row.stacked {
    grid-template-columns: minmax(0, 1fr);
    gap: 10px;
    align-items: stretch;
  }
  .setting-row.last {
    border-bottom: 0;
  }
  .row-text {
    min-width: 0;
  }
  .row-label {
    font-size: 12.5px;
    color: var(--ink-0);
    font-weight: 500;
  }
  .row-desc {
    margin-top: 2px;
    font-size: 11px;
    color: var(--ink-2);
    line-height: 1.5;
  }
  .row-control {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .setting-row.stacked .row-control {
    justify-content: stretch;
  }
</style>
