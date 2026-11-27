<script lang="ts">
  import { t } from '$lib/i18n';
  import { onDestroy, onMount } from 'svelte';
  import {
    cancelAsrDownload,
    deleteAsrModel,
    installAsrModel,
    listAsrModels,
    onAsrProgress,
    selectAsrModel,
    type AsrModels
  } from '$lib/ipc';
  import { systemProfile } from '$lib/system-profile.svelte';
  import { errorText } from '$lib/errors';
  import { notify } from '$lib/toast.svelte';
  import ModelChoice from './ModelChoice.svelte';

  let data = $state<AsrModels>({ selected: '', dir: '', models: [] });
  let progress = $state<Record<string, number | undefined>>({});
  let busy = $state<Record<string, boolean>>({});
  let error = $state<string | null>(null);
  let unlisten: (() => void) | undefined;
  const recommended = $derived(systemProfile.recommended?.asr.model ?? '');

  async function refresh() {
    data = await listAsrModels();
  }

  async function guard(task: () => Promise<void>) {
    error = null;
    try {
      await task();
    } catch (e) {
      if (!String(e).includes('cancelled')) error = errorText(e);
    }
    await refresh().catch((e) => (error = errorText(e)));
  }

  onMount(async () => {
    systemProfile.load();
    await guard(async () => {});
    unlisten = await onAsrProgress((p) => {
      progress = { ...progress, [p.id]: p.fraction };
    });
  });
  onDestroy(() => unlisten?.());

  /** 取得。select 指定時、または選択中モデルが未取得の場合は完了後に切替。 */
  async function download(id: string, select: boolean) {
    busy = { ...busy, [id]: true };
    try {
      await installAsrModel(id);
      const current = data.models.find((x) => x.id === data.selected);
      if (select || !current?.installed || current.id === id) await selectAsrModel(id);
    } finally {
      busy = { ...busy, [id]: false };
      const { [id]: _done, ...rest } = progress;
      progress = rest;
    }
  }

  const select = (id: string) => guard(() => selectAsrModel(id));
  const install = (id: string, sel: boolean) => guard(() => download(id, sel));

  /** 使用中モデルの削除：代替モデルを先に切替（未取得なら取得後）。 */
  const remove = (id: string, replacement: string | null) =>
    guard(async () => {
      if (replacement) {
        const alt = data.models.find((m) => m.id === replacement);
        if (alt && !alt.installed) await download(replacement, true);
        else await selectAsrModel(replacement);
        await refresh();
        if (data.selected !== replacement) return;
      }
      await deleteAsrModel(id);
      notify(t('{name}：削除完了', { name: data.models.find((m) => m.id === id)?.label ?? id }));
    });
</script>

<ModelChoice
  key="asr"
  feature={t('音声認識')}
  rows={data.models}
  selected={data.selected}
  {recommended}
  {progress}
  {busy}
  onselect={select}
  oninstall={install}
  oncancel={(id) => cancelAsrDownload(id)}
  onremove={remove}
/>
{#if error}
  <div class="srow error"><p class="row-label">{error}</p></div>
{/if}
