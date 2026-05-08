<script lang="ts">
  /**
   * Grouped settings container — rendered as a card with a small
   * uppercase kicker title above. Each `SettingRow` slotted inside
   * lines up label-on-left / control-on-right.
   */
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    description?: string;
    children: Snippet;
  }

  let { title, description, children }: Props = $props();
</script>

<section class="setting-group">
  <header>
    <h3 class="kicker">{title}</h3>
    {#if description}
      <p class="desc">{description}</p>
    {/if}
  </header>
  <div class="group-body card-soft">
    {@render children()}
  </div>
</section>

<style>
  .setting-group {
    margin-bottom: 28px;
  }
  header {
    margin-bottom: 10px;
  }
  .kicker {
    margin: 0;
    font-size: 10.5px;
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-weight: 500;
  }
  .desc {
    margin: 4px 0 0;
    font-size: 11.5px;
    color: var(--ink-2);
    line-height: 1.5;
  }
  .group-body {
    padding: 0;
    overflow: hidden;
  }
</style>
