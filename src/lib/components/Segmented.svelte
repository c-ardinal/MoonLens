<script lang="ts" generics="T extends string">
  /**
   * Segmented control — a row of radio-button-style buttons sharing a
   * single underlying value. Selecting one calls `onChange` with that
   * option's value. The active button is highlighted via the
   * `.segmented` CSS in design/components.css.
   */
  interface Option {
    value: T;
    label: string;
  }

  interface Props {
    options: Option[];
    value: T;
    onChange: (next: T) => void;
    'aria-label'?: string;
  }

  let { options, value, onChange, 'aria-label': ariaLabel }: Props = $props();
</script>

<div class="segmented" role="radiogroup" aria-label={ariaLabel}>
  {#each options as option (option.value)}
    <button
      type="button"
      role="radio"
      aria-checked={value === option.value}
      class:active={value === option.value}
      onclick={() => onChange(option.value)}
    >
      {option.label}
    </button>
  {/each}
</div>
