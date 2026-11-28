<script lang="ts">
  import { t } from '$lib/i18n';
  import { onMount } from 'svelte';
  import { Volume2 } from 'lucide-svelte';
  import { listVoices, speak } from '$lib/ipc';
  import { sampleFor, voiceLabel, type Voice } from '$lib/speech';

  /**
   * 読上音声の選択（導入済みの音声一覧）と試聴。空文字：自動（対象言語の先頭の音声）。
   */
  interface Props {
    id: string;
    value: string;
    /** 自動選択時の試聴言語（訳文の言語）。 */
    language: string;
    onChange: (v: string) => void;
  }
  let { id, value, language, onChange }: Props = $props();

  let voices = $state<Voice[]>([]);
  let loaded = $state(false);
  const chosen = $derived(voices.find((v) => v.id === value));

  onMount(() => {
    listVoices()
      .then((v) => (voices = v))
      .catch(() => {})
      .finally(() => (loaded = true));
  });

  function preview() {
    const sample = sampleFor(chosen, language);
    speak(sample.text, sample.lang).catch(() => {});
  }
</script>

<select
  {id}
  class="max-w-full rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm"
  value={value ?? ''}
  onchange={(e) => onChange((e.currentTarget as HTMLSelectElement).value)}
>
  <option value="">{t('speech:voice_auto')}</option>
  {#if value && !chosen}
    <option {value}>{loaded ? t('speech:voice_missing') : value}</option>
  {/if}
  {#each voices as v (v.id)}
    <option value={v.id}>{voiceLabel(v)}</option>
  {/each}
</select>
<button
  type="button"
  class="preview rounded-ml-sm p-1 text-ink-muted transition hover:bg-surface-muted hover:text-ink"
  title={t('speech:preview')}
  aria-label={t('speech:preview')}
  disabled={loaded && voices.length === 0}
  onclick={preview}
>
  <Volume2 class="h-4 w-4" />
</button>

<style>
  /* 設定行の操作列：選択欄のみ伸長（試聴ボタンは固定幅） */
  .preview {
    flex: 0 0 auto !important;
  }
</style>
