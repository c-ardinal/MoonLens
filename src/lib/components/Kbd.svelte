<script lang="ts">
  /**
   * Keyboard hint chip — wraps a single key glyph (e.g. `⌘`, `S`, `↵`)
   * in a chiclet that matches the design's kbd treatment.
   */
  interface Props {
    label: string;
  }

  let { label }: Props = $props();
</script>

<span class="kbd">{label}</span>
