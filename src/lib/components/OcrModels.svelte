<script lang="ts">
  import { t } from '$lib/i18n';
  import { onDestroy, onMount } from 'svelte';
  import { installOcrModels, ocrModelsStatus, onModelProgress, type OcrModelsStatus } from '$lib/ipc';
  import { ocrChoice, OCR_CHOICE_ID } from '$lib/model-setup';
  import { errorText } from '$lib/errors';
  import ModelChoice from './ModelChoice.svelte';
  import OcrScripts from './OcrScripts.svelte';

  let status = $state<OcrModelsStatus | null>(null);
  let installing = $state(false);
  let fraction = $state<number | undefined>(undefined);
  let error = $state<string | null>(null);
  let unlisten: (() => void) | undefined;

  async function refresh() {
    try {
      status = await ocrModelsStatus();
    } catch (e) {
      error = errorText(e);
    }
  }

  onMount(async () => {
    await refresh();
    unlisten = await onModelProgress((p) => (fraction = p.fraction ?? 0));
  });
  onDestroy(() => unlisten?.());

  /** PP-OCR・予備の ocrs・高速実行部品を一括取得（個別取得は不可）。 */
  async function install() {
    installing = true;
    error = null;
    try {
      await installOcrModels();
    } catch (e) {
      error = errorText(e);
    } finally {
      installing = false;
      fraction = undefined;
      await refresh();
    }
  }
</script>

{#if status}
  <!-- 選択肢は PP-OCR のみ（常に選択状態）。未取得時は取得を案内 -->
  <ModelChoice
    key="ocr"
    feature={t('common:text_recognition')}
    rows={[ocrChoice(status)]}
    selected={OCR_CHOICE_ID}
    recommended={OCR_CHOICE_ID}
    progress={installing ? { [OCR_CHOICE_ID]: fraction ?? 0 } : {}}
    busy={{ [OCR_CHOICE_ID]: installing }}
    removable={false}
    onselect={async () => {}}
    oninstall={install}
  />
  <OcrScripts ready={status.ppocr} />
{/if}
{#if error}
  <div class="srow error"><p class="row-label">{error}</p></div>
{/if}
