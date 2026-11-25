<script lang="ts">
  // 推論デバイスの表示行（設定「GPU 使用」の直下）。表示のみ、操作部なし。
  import { onMount } from 'svelte';
  import { inferenceDevice } from '$lib/model-setup';
  import { systemProfile } from '$lib/system-profile.svelte';

  interface Props {
    /** 「GPU 使用」の現在値（未保存の変更を含む）。 */
    useGpu: boolean;
  }
  let { useGpu }: Props = $props();

  const profile = $derived(systemProfile.report?.profile ?? null);
  let failed = $state(false);

  onMount(() => {
    systemProfile.load().then((r) => (failed = !r));
  });
</script>

<div class="flex min-h-[60px] items-center justify-between gap-6 py-2">
  <span class="row-label text-sm font-medium text-ink">推論デバイス</span>
  <span class="row-value value-text text-right text-sm text-ink-muted">
    {#if profile}
      {inferenceDevice(profile, useGpu)}
    {:else if failed}
      取得不可
    {:else}
      確認中…
    {/if}
  </span>
</div>
