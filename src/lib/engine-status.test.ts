import { describe, expect, it } from 'vitest';
import type { AsrModels, LlmModels, OcrModelsStatus } from './ipc';
import { engineRows } from './engine-status';

const llmModel = (id: string, label: string, installed: boolean, downloading = false) => ({
  id,
  label,
  note: '',
  file: '',
  url: '',
  size: 0,
  sha256: '',
  license: '',
  license_url: '',
  style: 'pfml3' as const,
  consent: null,
  installed,
  partial_bytes: 0,
  downloading
});

const llm = (installed: boolean, downloading = false): LlmModels => ({
  selected: 'plamo-2b',
  assistant: 'gemma',
  dir: '',
  accepted_licenses: [],
  models: [
    llmModel('plamo-2b', 'PLaMo 3 Translate 2B', installed, downloading),
    llmModel('gemma', 'Gemma 4 E4B', false)
  ]
});
const ocr = (ppocr: boolean, ocrs = false): OcrModelsStatus => ({
  ppocr,
  ocrs,
  fast: true,
  dir: ''
});
const asr: AsrModels = { selected: '', dir: '', models: [] };

describe('engineRows', () => {
  it('reports each engine with its model and state', () => {
    const rows = engineRows(llm(true), ocr(true), asr);
    expect(rows.map((r) => [r.id, r.model, r.state])).toEqual([
      ['translate', 'PLaMo 3 Translate 2B', 'ready'],
      ['assistant', 'Gemma 4 E4B', 'missing'],
      ['ocr', 'PP-OCRv5', 'ready'],
      ['asr', '—', 'unset']
    ]);
  });

  it('shows a download in progress', () => {
    expect(engineRows(llm(false, true), ocr(true), asr)[0].state).toBe('downloading');
  });

  it('falls back to ocrs when PP-OCR is missing', () => {
    const row = engineRows(llm(true), ocr(false, true), asr)[2];
    expect(row).toMatchObject({ model: 'ocrs（英語のみ）', state: 'ready' });
  });
});
