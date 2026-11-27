/**
 * Home page "local engine" card: one row per built-in engine with the
 * model in use and whether it is ready.
 */
import type { AsrModels, LlmModels, OcrModelsStatus } from './ipc';
import { t } from './i18n';

export type EngineState = 'ready' | 'missing' | 'downloading' | 'unset';

export interface EngineRow {
  id: 'translate' | 'assistant' | 'ocr' | 'asr';
  label: string;
  model: string;
  state: EngineState;
}

/** 状態名（文言のキー ID）。表示時は `t()` で取得。 */
export const STATE_LABELS: Record<EngineState, string> = {
  ready: 'common:ready',
  missing: 'common:missing',
  downloading: 'engine-status:downloading',
  unset: 'common:not_set'
};

function modelRow(
  id: EngineRow['id'],
  label: string,
  selected: string,
  models: {
    id: string;
    label: string;
    installed: boolean;
    downloading: boolean;
  }[]
): EngineRow {
  const m = models.find((x) => x.id === selected);
  if (!selected || !m) return { id, label: t(label), model: selected || '—', state: 'unset' };
  const state: EngineState = m.installed ? 'ready' : m.downloading ? 'downloading' : 'missing';
  return { id, label: t(label), model: m.label, state };
}

export function engineRows(llm: LlmModels, ocr: OcrModelsStatus, asr: AsrModels): EngineRow[] {
  return [
    modelRow('translate', 'engine-status:translate_engine', llm.selected, llm.models),
    modelRow('assistant', 'engine-status:explain_engine', llm.assistant, llm.models),
    {
      id: 'ocr',
      label: t('engine-status:ocr_engine'),
      model: ocr.ppocr ? (ocr.current ? 'PP-OCRv6' : 'PP-OCRv5') : ocr.ocrs ? t('engine-status:ocrs_english_only') : 'PP-OCRv6',
      state: ocr.ppocr || ocr.ocrs ? 'ready' : 'missing'
    },
    modelRow('asr', 'engine-status:speech_engine', asr.selected, asr.models)
  ];
}
