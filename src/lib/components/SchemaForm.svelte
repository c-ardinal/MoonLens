<script lang="ts">
  /**
   * Auto-generated settings form driven by a JSON Schema.
   *
   * The schema comes from Rust's `schemars` and uses `$ref` heavily,
   * so we walk `definitions` / `$defs` to resolve them. Supports the
   * subset this project actually emits:
   *   - objects (rendered as a collapsible <details> section)
   *   - enums (select)
   *   - string / integer / number / boolean (matching input types)
   *   - string[] (comma-joined text input)
   *   - string→string maps via `additionalProperties` (dynamic key/value)
   *   - `format: "hotkey"` → HotkeyInput widget
   * Unknown shapes render a warning + collapsible JSON dump so the user
   * at least sees the value rather than a broken control.
   */
  import { ChevronRight, AlertTriangle, Plus, Trash2 } from 'lucide-svelte';
  import HotkeyInput from './HotkeyInput.svelte';
  import { matchesQuery } from '$lib/schema-filter';

  interface Props {
    schema: any;
    value: any;
    query?: string;
    onChange?: () => void;
  }
  let { schema, value = $bindable(), query = '', onChange }: Props = $props();

  const root = $derived(resolveRoot(schema));

  function resolveRoot(s: any): any {
    if (!s) return null;
    const defs: Record<string, any> = s.definitions ?? s.$defs ?? {};
    if (defs.Settings) return defs.Settings;
    return s;
  }

  /**
   * Walk a `$ref` like "#/definitions/Foo" into the full definition,
   * so downstream rendering sees a complete schema node. Keeps cycles
   * safe via a visited-set.
   */
  function resolveRef(ref: string, seen = new Set<string>()): any {
    if (seen.has(ref)) return null;
    seen.add(ref);
    // Only handle local refs; remote refs aren't in scope here.
    if (!ref.startsWith('#/')) return null;
    const parts = ref.slice(2).split('/');
    let node: any = schema;
    for (const p of parts) {
      if (node == null) return null;
      node = node[p];
    }
    return resolve(node, seen);
  }

  function resolve(node: any, seen = new Set<string>()): any {
    if (!node || typeof node !== 'object') return node;
    if (typeof node.$ref === 'string') {
      const merged = resolveRef(node.$ref, seen) ?? {};
      // Let the original node's fields override the referenced one
      // (e.g. local `description`, `default`).
      const rest: Record<string, unknown> = {};
      for (const [k, val] of Object.entries(node)) {
        if (k !== '$ref') rest[k] = val;
      }
      return { ...merged, ...rest };
    }
    return node;
  }

  function change() {
    onChange?.();
  }

  function isMap(prop: any): boolean {
    return (
      prop?.type === 'object' &&
      !prop.properties &&
      prop.additionalProperties &&
      prop.additionalProperties !== false
    );
  }
</script>

<div class="flex flex-col gap-3">
  {#if root?.properties}
    {#each Object.entries(root.properties) as [key, rawProp] (key)}
      {@const prop = resolve(rawProp)}
      {#if matchesQuery(key, query)}
        <details
          class="group rounded-ml border border-border bg-surface-elevated shadow-ml transition-colors open:border-accent/30"
          open
        >
          <summary
            class="flex cursor-pointer items-center gap-2 px-4 py-3 text-sm font-semibold tracking-tight hover:bg-surface-muted/30"
          >
            <ChevronRight
              class="h-3.5 w-3.5 text-ink-faint transition-transform group-open:rotate-90"
              aria-hidden="true"
            />
            <span class="flex-1">{key}</span>
            {#if prop?.description}
              <span class="text-xs font-normal text-ink-faint">{prop.description}</span>
            {/if}
          </summary>
          <div class="border-t border-border px-4 py-3">
            {@render node(prop, value[key], (v: any) => {
              value = { ...value, [key]: v };
              change();
            }, key, 0)}
          </div>
        </details>
      {/if}
    {/each}
  {/if}
</div>

{#snippet node(prop: any, v: any, set: (v: any) => void, path: string, depth: number)}
  {@const p = resolve(prop)}
  {#if !p}
    <code class="text-xs text-ink-faint">{JSON.stringify(v)}</code>
  {:else if isMap(p)}
    {@render mapEditor(p, v, set, path)}
  {:else if p.type === 'object' && p.properties}
    {@render objectEditor(p, v, set, path, depth)}
  {:else if p.enum}
    <select
      class="rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm transition-colors focus-visible:border-accent"
      value={v ?? p.default ?? p.enum[0]}
      onchange={(e) => set((e.target as HTMLSelectElement).value)}
    >
      {#each p.enum as opt}
        <option value={opt}>{opt}</option>
      {/each}
    </select>
  {:else if p.type === 'boolean'}
    <label class="inline-flex cursor-pointer items-center gap-2">
      <input
        type="checkbox"
        checked={!!v}
        onchange={(e) => set((e.target as HTMLInputElement).checked)}
      />
      <span class="text-xs text-ink-muted">{v ? '有効' : '無効'}</span>
    </label>
  {:else if p.type === 'number' || p.type === 'integer'}
    <input
      type="number"
      class="w-28 rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm transition-colors focus-visible:border-accent"
      value={v ?? ''}
      oninput={(e) => {
        const raw = (e.target as HTMLInputElement).value;
        set(raw === '' ? undefined : Number(raw));
      }}
    />
  {:else if p.type === 'array' && p.items?.type === 'string'}
    <input
      type="text"
      class="w-full rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm font-mono transition-colors focus-visible:border-accent"
      value={Array.isArray(v) ? v.join(', ') : ''}
      oninput={(e) =>
        set(
          (e.target as HTMLInputElement).value
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean)
        )}
    />
  {:else if p.type === 'string' && p.format === 'hotkey'}
    <HotkeyInput value={v ?? ''} onChange={(next) => set(next)} />
  {:else if p.type === 'string'}
    <input
      type="text"
      class="w-full rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm transition-colors focus-visible:border-accent"
      value={v ?? ''}
      oninput={(e) => set((e.target as HTMLInputElement).value)}
    />
  {:else}
    {@render unsupported(p, v)}
  {/if}
{/snippet}

{#snippet objectEditor(
  p: any,
  v: any,
  set: (v: any) => void,
  path: string,
  depth: number
)}
  {@const effective = v ?? {}}
  {#if depth === 0}
    <div class="grid grid-cols-[minmax(140px,200px)_1fr] gap-x-4 gap-y-3">
      {#each Object.entries(p.properties) as [k, child] (k)}
        {#if matchesQuery(`${path}.${k}`, query)}
          {@const childResolved = resolve(child)}
          <label class="pt-1 text-xs text-ink-muted" for={`f-${path}-${k}`}>
            <span class="font-mono">{k}</span>
            {#if childResolved?.description}
              <span class="block text-[10px] font-normal text-ink-faint">
                {childResolved.description}
              </span>
            {/if}
          </label>
          <div id={`f-${path}-${k}`}>
            {@render node(
              child,
              effective[k],
              (vv: any) => set({ ...effective, [k]: vv }),
              `${path}.${k}`,
              depth + 1
            )}
          </div>
        {/if}
      {/each}
    </div>
  {:else}
    <details class="rounded-ml-sm border border-border/60 bg-surface/50 p-3">
      <summary class="cursor-pointer text-xs text-ink-muted">
        <span class="font-mono">{path.split('.').slice(-1)[0] ?? ''}</span>
      </summary>
      <div class="mt-3 grid grid-cols-[minmax(140px,200px)_1fr] gap-x-4 gap-y-3">
        {#each Object.entries(p.properties) as [k, child] (k)}
          {#if matchesQuery(`${path}.${k}`, query)}
            {@const childResolved = resolve(child)}
            <label class="pt-1 text-xs text-ink-muted" for={`f-${path}-${k}`}>
              <span class="font-mono">{k}</span>
              {#if childResolved?.description}
                <span class="block text-[10px] font-normal text-ink-faint">
                  {childResolved.description}
                </span>
              {/if}
            </label>
            <div id={`f-${path}-${k}`}>
              {@render node(
                child,
                effective[k],
                (vv: any) => set({ ...effective, [k]: vv }),
                `${path}.${k}`,
                depth + 1
              )}
            </div>
          {/if}
        {/each}
      </div>
    </details>
  {/if}
{/snippet}

{#snippet mapEditor(p: any, v: any, set: (v: any) => void, path: string)}
  {@const entries = Object.entries((v ?? {}) as Record<string, unknown>)}
  <div class="flex flex-col gap-2">
    {#each entries as [mk, mv] (mk)}
      <div class="flex items-center gap-2">
        <input
          type="text"
          class="w-40 rounded-ml-sm border border-border bg-surface px-2 py-1 font-mono text-xs"
          value={mk}
          onchange={(e) => {
            const nk = (e.target as HTMLInputElement).value;
            if (!nk || nk === mk) return;
            const next = { ...(v ?? {}) } as Record<string, unknown>;
            next[nk] = next[mk];
            delete next[mk];
            set(next);
          }}
        />
        <input
          type="text"
          class="flex-1 rounded-ml-sm border border-border bg-surface px-2 py-1 text-sm"
          value={typeof mv === 'string' ? mv : JSON.stringify(mv)}
          oninput={(e) => {
            const next = { ...(v ?? {}) } as Record<string, unknown>;
            next[mk] = (e.target as HTMLInputElement).value;
            set(next);
          }}
        />
        <button
          type="button"
          onclick={() => {
            const next = { ...(v ?? {}) } as Record<string, unknown>;
            delete next[mk];
            set(next);
          }}
          class="inline-flex h-7 w-7 items-center justify-center rounded-ml-sm text-ink-faint hover:bg-red-500/10 hover:text-red-500"
          title="削除"
          aria-label="キーを削除"
        >
          <Trash2 class="h-3.5 w-3.5" aria-hidden="true" />
        </button>
      </div>
    {/each}
    <button
      type="button"
      onclick={() => {
        const next = { ...(v ?? {}) } as Record<string, unknown>;
        let k = 'new';
        let n = 1;
        while (k in next) k = `new${n++}`;
        next[k] = '';
        set(next);
      }}
      class="inline-flex w-fit items-center gap-1 rounded-ml-sm border border-dashed border-border px-2 py-1 text-xs text-ink-muted hover:border-accent/60 hover:text-ink"
    >
      <Plus class="h-3 w-3" aria-hidden="true" />
      キーを追加
    </button>
  </div>
{/snippet}

{#snippet unsupported(p: any, v: any)}
  <details class="rounded-ml-sm border border-yellow-500/30 bg-yellow-500/10 p-2">
    <summary class="flex cursor-pointer items-center gap-2 text-xs text-yellow-600 dark:text-yellow-400">
      <AlertTriangle class="h-3.5 w-3.5" aria-hidden="true" />
      <span>このフィールドはまだ GUI 編集に対応していません</span>
    </summary>
    <div class="mt-2 space-y-2">
      <div class="text-[10px] text-ink-faint">
        schema: <code class="font-mono">{JSON.stringify(p)}</code>
      </div>
      <div class="text-[10px] text-ink-faint">
        現在値: <code class="font-mono">{JSON.stringify(v)}</code>
      </div>
    </div>
  </details>
{/snippet}
