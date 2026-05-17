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
    flex-wrap: wrap;
    row-gap: 8px;
    min-width: 0;
    box-sizing: border-box;
    width: 100%;
  }
  h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.015em;
    flex-shrink: 0;
    white-space: nowrap;
  }
  .sub {
    color: var(--ink-2);
    font-size: 12.5px;
    margin-left: 12px;
    padding-left: 14px;
    border-left: 1px solid var(--line);
    /* Subtitle shrinks first; on very narrow widths it disappears so the
       actions cluster stays visible. */
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 0 1 auto;
  }
  .actions {
    margin-left: auto;
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    flex-shrink: 0;
    /* Prevent the actions cluster from itself becoming so wide that it
       pushes past the visible header width when wrap+auto-margin combine. */
    max-width: 100%;
  }
  /* Hide the subtitle entirely when the row gets cramped — actions stay
     visible. Triggered at ~720px window with the sidebar expanded, the
     same threshold used across the rest of the app. */
  @media (max-width: 760px) {
    .sub {
      display: none;
    }
  }
  /* The keyboard hint (e.g. `⌘L`) inside primary action buttons is the
     first thing to drop when room is tight — the icon + label still
     communicate the action. */
  :global(.ml-pageheader .kbd-inline) {
    margin-left: 8px;
    opacity: 0.55;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  @media (max-width: 880px) {
    :global(.ml-pageheader .kbd-inline) {
      display: none;
    }
  }
</style>
