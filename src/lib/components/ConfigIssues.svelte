<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { AlertTriangle } from 'lucide-svelte';
  import { applyConfigFixes, listConfigIssues, onConfigIssues, type ConfigIssue } from '$lib/ipc';
  import { errorText } from '$lib/errors';

  /** Initial list (tests); the live list comes from the backend. */
  let { initial = [] }: { initial?: ConfigIssue[] } = $props();

  let issues = $state<ConfigIssue[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let unlisten: (() => void) | null = null;

  onMount(async () => {
    issues = initial;
    listConfigIssues()
      .then((i) => (issues = i.length ? i : issues))
      .catch(() => {});
    try {
      unlisten = await onConfigIssues((i) => (issues = i));
    } catch {
      /* pure frontend dev */
    }
  });
  onDestroy(() => unlisten?.());

  const fixable = $derived(issues.filter((i) => i.fix_label));

  async function fix(ids: string[]) {
    busy = true;
    error = null;
    try {
      await applyConfigFixes(ids);
      issues = await listConfigIssues();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if issues.length > 0}
  <section
    class="flex flex-col gap-2 rounded-ml border border-amber-500/50 bg-amber-500/10 p-3 text-sm"
    role="alert"
    aria-label="設定の不正値"
  >
    <div class="flex items-center gap-2">
      <AlertTriangle class="h-4 w-4 shrink-0 text-amber-600" />
      <h2 class="min-w-0 flex-1 font-medium">設定の不正値 {issues.length} 件</h2>
      {#if fixable.length > 1}
        <button
          class="shrink-0 rounded-ml-sm bg-accent px-2 py-1 text-xs text-white disabled:opacity-50"
          disabled={busy}
          onclick={() => fix([])}
        >
          全件修正
        </button>
      {/if}
    </div>
    <ul class="flex flex-col gap-2">
      {#each issues as i (i.id)}
        <li class="flex flex-wrap items-center gap-x-2 gap-y-1 border-t border-amber-500/30 pt-2">
          <div class="min-w-0 flex-1">
            <code class="block break-all text-xs text-ink-muted">{i.path}</code>
            <span class="break-words">{i.problem}</span>
          </div>
          {#if i.fix_label}
            <button
              class="shrink-0 rounded-ml-sm border border-accent/50 px-2 py-1 text-xs text-accent hover:bg-accent-soft disabled:opacity-50"
              disabled={busy}
              onclick={() => fix([i.id])}
            >
              {i.fix_label}
            </button>
          {/if}
        </li>
      {/each}
    </ul>
    <p class="text-xs text-ink-muted">修正前の設定は config.toml.bak-（時刻）として保存。</p>
    {#if error}
      <p class="text-xs text-red-600">{error}</p>
    {/if}
  </section>
{/if}
