<script lang="ts">
  /**
   * Auto-generated settings form driven by a JSON Schema.
   *
   * The schema is produced on the Rust side by `schemars`; adding a new
   * settings field there surfaces it here automatically. Supports the
   * subset of JSON Schema the settings struct uses: objects, arrays of
   * strings, enums, booleans, numbers, strings, and maps (additionalProperties).
   */

  interface Props {
    schema: any;
    value: any;
    query?: string;
    onChange?: () => void;
  }
  let { schema, value = $bindable(), query = '', onChange }: Props = $props();

  const root = $derived(schema?.definitions?.Settings ?? schema);

  function matches(path: string): boolean {
    if (!query) return true;
    return path.toLowerCase().includes(query.toLowerCase());
  }

  function change() {
    onChange?.();
  }
</script>

<div class="flex flex-col gap-6">
  {#if root?.properties}
    {#each Object.entries(root.properties) as [key, prop]}
      {#if matches(key)}
        <section
          class="rounded-ml border border-border bg-surface-elevated p-4 shadow-ml"
        >
          <h2 class="mb-3 text-sm font-semibold tracking-tight">{key}</h2>
          {@render node(prop, value[key], (v: any) => {
            value = { ...value, [key]: v };
            change();
          }, key)}
        </section>
      {/if}
    {/each}
  {/if}
</div>

{#snippet node(prop: any, v: any, set: (v: any) => void, path: string)}
  {#if prop?.type === 'object' && prop.properties}
    <div class="grid grid-cols-[200px_1fr] gap-x-4 gap-y-2">
      {#each Object.entries(prop.properties) as [k, child]}
        {#if matches(`${path}.${k}`)}
          <label class="pt-1 text-xs text-ink-muted">{k}</label>
          <div>
            {@render node(child, v?.[k], (vv: any) => set({ ...v, [k]: vv }), `${path}.${k}`)}
          </div>
        {/if}
      {/each}
    </div>
  {:else if prop?.enum}
    <select
      class="rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm"
      value={v ?? prop.default ?? prop.enum[0]}
      onchange={(e) => set((e.target as HTMLSelectElement).value)}
    >
      {#each prop.enum as opt}
        <option value={opt}>{opt}</option>
      {/each}
    </select>
  {:else if prop?.type === 'boolean'}
    <input
      type="checkbox"
      checked={!!v}
      onchange={(e) => set((e.target as HTMLInputElement).checked)}
    />
  {:else if prop?.type === 'number' || prop?.type === 'integer'}
    <input
      type="number"
      class="w-28 rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm"
      value={v ?? ''}
      oninput={(e) => set(Number((e.target as HTMLInputElement).value))}
    />
  {:else if prop?.type === 'array' && prop.items?.type === 'string'}
    <input
      type="text"
      class="w-full rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm font-mono"
      value={Array.isArray(v) ? v.join(', ') : ''}
      oninput={(e) =>
        set(
          (e.target as HTMLInputElement).value
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean)
        )}
    />
  {:else if prop?.type === 'string'}
    <input
      type="text"
      class="w-full rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm"
      value={v ?? ''}
      oninput={(e) => set((e.target as HTMLInputElement).value)}
    />
  {:else}
    <code class="text-xs text-ink-faint">{JSON.stringify(v)}</code>
  {/if}
{/snippet}
