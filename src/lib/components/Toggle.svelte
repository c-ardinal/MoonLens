<script lang="ts">
  /**
   * Visual on/off toggle. Uses `role="switch"` + `aria-checked` so it
   * announces correctly to assistive tech without needing a hidden
   * checkbox; the styled track + dot are pure CSS.
   */
  interface Props {
    checked: boolean;
    onChange: (next: boolean) => void;
    disabled?: boolean;
    'aria-label'?: string;
  }

  let {
    checked,
    onChange,
    disabled = false,
    'aria-label': ariaLabel
  }: Props = $props();

  function handleClick() {
    if (disabled) return;
    onChange(!checked);
  }
</script>

<button
  type="button"
  role="switch"
  class="toggle"
  aria-checked={checked}
  aria-label={ariaLabel}
  {disabled}
  onclick={handleClick}
></button>
