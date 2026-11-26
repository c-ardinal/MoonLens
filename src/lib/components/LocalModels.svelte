<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { cancelLlmDownload, deleteLlmModel, selectLlmModel, type LlmRole } from '$lib/ipc';
  import { llmModels } from '$lib/llm-models.svelte';
  import { systemProfile } from '$lib/system-profile.svelte';
  import { errorText } from '$lib/errors';
  import { notify } from '$lib/toast.svelte';
  import ModelChoice from './ModelChoice.svelte';

  interface Props {
    /** 表示する用途：翻訳（全モデル）／説明・要約（汎用モデルのみ）。 */
    role?: LlmRole;
  }
  let { role = 'translate' }: Props = $props();

  const store = llmModels;
  const data = $derived(store.data);
  let error = $state<string | null>(null);

  /** 説明・要約は汎用（チャット形式）モデルのみ対応。 */
  const shown = $derived(
    role === 'assistant' ? data.models.filter((m) => m.style === 'chat_template') : data.models
  );
  const current = $derived(role === 'assistant' ? data.assistant : data.selected);
  const recommended = $derived(systemProfile.recommended?.[role]?.model ?? '');
  const progress = $derived(Object.fromEntries(Object.entries(store.progress).map(([k, p]) => [k, p.fraction])));
  const feature = $derived(role === 'assistant' ? '説明・要約' : '翻訳');

  /** 他用途で使用中のモデル（削除不可）。 */
  function usedElsewhere(id: string): string | null {
    if (role === 'translate' && id === data.assistant) return '説明・要約';
    if (role === 'assistant' && id === data.selected) return '翻訳';
    return null;
  }

  async function guard(task: () => Promise<void>) {
    error = null;
    try {
      await task();
    } catch (e) {
      if (!String(e).includes('cancelled')) error = errorText(e);
    }
    await store.refresh().catch(() => {});
  }

  onMount(async () => {
    await store.attach();
    systemProfile.load();
    await guard(async () => {});
  });
  onDestroy(() => store.detach());

  const select = (id: string) => guard(() => selectLlmModel(id, role));
  const install = (id: string, sel: boolean) => guard(() => store.download(id, role, sel));

  /** 使用中モデルの削除：代替モデルを先に切替（未取得なら取得後）。 */
  const remove = (id: string, replacement: string | null) =>
    guard(async () => {
      if (replacement) {
        const alt = data.models.find((m) => m.id === replacement);
        if (alt && !alt.installed) await store.download(replacement, role, true);
        else await selectLlmModel(replacement, role);
        await store.refresh();
        // 取得の中止・失敗時は削除しない（モデル不在の回避）。
        if (current !== replacement) return;
      }
      await deleteLlmModel(id);
      notify(`${data.models.find((m) => m.id === id)?.label ?? id}：削除完了`);
    });
</script>

<ModelChoice
  key="llm:{role}"
  {feature}
  rows={shown}
  selected={current}
  {recommended}
  {progress}
  busy={store.busy}
  {usedElsewhere}
  onselect={select}
  oninstall={install}
  oncancel={(id) => cancelLlmDownload(id)}
  onremove={remove}
/>
{#if error}
  <div class="srow error"><p class="row-label">{error}</p></div>
{/if}
