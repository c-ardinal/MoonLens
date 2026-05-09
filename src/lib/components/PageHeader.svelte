<script lang="ts">
  /**
   * Page header that matches the design's `.ml-pageheader` pattern:
   * h1 on the left, optional subtitle separated by a hairline rule, and
   * action slot pushed to the right edge.
   */
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    subtitle?: string;
    actions?: Snippet;
  }

  let { title, subtitle, actions }: Props = $props();
</script>

<header class="ml-pageheader">
  <h1>{title}</h1>
  {#if subtitle}
    <span class="sub">{subtitle}</span>
  {/if}
  <div class="actions">
    {#if actions}{@render actions()}{/if}
  </div>
</header>

<style>
  .ml-pageheader {
    display: flex;
    align-items: center;
    padding: 18px 28px 14px;
    gap: var(--s-3);
    /* Prevent header content from overflowing the viewport width and
       pushing the actions button off-screen when the window is narrow. */
    min-width: 0;
    overflow: hidden;
  }
  h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.015em;
    /* Prevent the title from growing past available space when window is narrow */
    flex-shrink: 0;
  }
  .sub {
    color: var(--ink-2);
    font-size: 12.5px;
    margin-left: 12px;
    padding-left: 14px;
    border-left: 1px solid var(--line);
    /* Allow subtitle to shrink and truncate before the actions button disappears */
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex-shrink: 1;
  }
  .actions {
    margin-left: auto;
    display: flex;
    gap: 8px;
    /* Ensure the actions slot never shrinks — it must always be visible */
    flex-shrink: 0;
  }
</style>
