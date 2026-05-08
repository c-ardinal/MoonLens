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
  }
  h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.015em;
  }
  .sub {
    color: var(--ink-2);
    font-size: 12.5px;
    margin-left: 12px;
    padding-left: 14px;
    border-left: 1px solid var(--line);
  }
  .actions {
    margin-left: auto;
    display: flex;
    gap: 8px;
  }
</style>
