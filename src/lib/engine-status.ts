/**
 * Home page "local engine" card: one row per built-in engine with the
 * model in use and whether it is ready.
 */
import type { AsrModels, LlmModels, OcrModelsStatus } from './ipc';

export type EngineState = 'ready' | 'missing' | 'downloading' | 'unset';

export interface EngineRow {
  id: 'translate' | 'assistant' | 'ocr' | 'asr';
  label: string;
  model: string;
  state: EngineState;
}

export const STATE_LABELS: Record<EngineState, string> = {
  ready: '準備完了',
  missing: '未導入',
  downloading: 'ダウンロード中',
  unset: '未設定'
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
  if (!selected || !m) return { id, label, model: selected || '—', state: 'unset' };
  const state: EngineState = m.installed ? 'ready' : m.downloading ? 'downloading' : 'missing';
  return { id, label, model: m.label, state };
}

export function engineRows(llm: LlmModels, ocr: OcrModelsStatus, asr: AsrModels): EngineRow[] {
  return [
    modelRow('translate', '翻訳', llm.selected, llm.models),
    modelRow('assistant', '説明・要約', llm.assistant, llm.models),
    {
      id: 'ocr',
      label: '文字認識',
      model: ocr.ppocr ? (ocr.current ? 'PP-OCRv6' : 'PP-OCRv5') : ocr.ocrs ? 'ocrs（英語のみ）' : 'PP-OCRv6',
      state: ocr.ppocr || ocr.ocrs ? 'ready' : 'missing'
    },
    modelRow('asr', '音声認識', asr.selected, asr.models)
  ];
}
