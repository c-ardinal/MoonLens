import { describe, expect, it } from 'vitest';
import {
  buildFeatures,
  ocrChoice,
  downloadQueue,
  formatBytes,
  hardwareLines,
  inferenceDevice,
  pendingConsents,
  replacementCandidates,
  totalBytes
} from './model-setup';
import type { AsrModels, LlmModels, Recommendation, SystemReport } from './ipc';

const llmModel = (id: string, style: 'pfml3' | 'chat_template', size: number, installed = false) => ({
  id,
  label: id,
  note: '',
  file: '',
  url: '',
  size,
  sha256: '',
  license: '',
  license_url: '',
  style,
  consent: null as string | null,
  installed,
  partial_bytes: 0,
  downloading: false
});

const llm = (installed: string[] = []): LlmModels => ({
  selected: 'plamo-3-translate-2b',
  assistant: 'gemma-4-e4b',
  dir: '',
  accepted_licenses: [] as string[],
  models: [
    llmModel('plamo-3-translate-2b', 'pfml3', 1_800_000_000),
    llmModel('plamo-3-translate-0.8b', 'pfml3', 570_000_000),
    llmModel('gemma-4-e4b', 'chat_template', 5_000_000_000),
    llmModel('gemma-4-e2b', 'chat_template', 3_100_000_000)
  ].map((m) => ({ ...m, installed: installed.includes(m.id) }))
});

const asr: AsrModels = {
  selected: 'whisper-large-v3-turbo',
  dir: '',
  models: [
    { ...llmModel('whisper-large-v3-turbo', 'pfml3', 874_000_000) },
    { ...llmModel('whisper-small', 'pfml3', 190_000_000) }
  ]
};

const rec: Recommendation = {
  translate: { model: 'plamo-3-translate-2b', reason: '' },
  assistant: { model: 'gemma-4-e2b', reason: '' },
  ocr: { model: 'ppocr', reason: '' },
  asr: { model: 'whisper-small', reason: '' }
};

const noOcr = { ppocr: false, current: false, ocrs: false, fast: false, dir: '' };

describe('buildFeatures', () => {
  it('selects the recommended model of every feature', () => {
    const f = buildFeatures(llm(), asr, noOcr, rec);
    expect(f.map((x) => [x.feature, x.selected])).toEqual([
      ['translate', 'plamo-3-translate-2b'],
      ['ocr', 'ppocr'],
      ['assistant', 'gemma-4-e2b'],
      ['asr', 'whisper-small']
    ]);
    expect(f.filter((x) => x.required).map((x) => x.feature)).toEqual(['translate', 'ocr']);
  });

  it('offers only general models for explain / summarize', () => {
    const a = buildFeatures(llm(), asr, noOcr, rec).find((x) => x.feature === 'assistant')!;
    expect(a.choices.map((c) => c.id)).toEqual(['gemma-4-e4b', 'gemma-4-e2b']);
  });

  it('falls back to the current model when the recommendation is unknown', () => {
    const bad = { ...rec, translate: { model: 'gone', reason: '' } };
    const f = buildFeatures(llm(), asr, noOcr, bad);
    expect(f[0].selected).toBe('plamo-3-translate-2b');
    expect(buildFeatures(llm(), asr, noOcr, null)[2].selected).toBe('gemma-4-e4b');
  });
});

describe('ocrChoice', () => {
  it('offers the faster runtime alone when the OCR models are already installed', () => {
    const c = ocrChoice({ ppocr: true, current: true, ocrs: true, fast: false, dir: '' });
    expect(c.installed).toBe(false);
    expect(c.size).toBe(25_000_000);
    expect(c.note).toContain('高速実行部品');
  });

  it('counts as installed once the faster runtime is there too', () => {
    expect(ocrChoice({ ppocr: true, current: true, ocrs: true, fast: true, dir: '' }).installed).toBe(true);
  });

  it('offers the new generation alone to an install that still runs the earlier one', () => {
    const c = ocrChoice({ ppocr: true, current: false, ocrs: true, fast: true, dir: '' });
    expect(c.installed).toBe(false);
    expect(c.size).toBe(31_000_000);
    expect(c.note).toContain('PP-OCRv5');
  });

  it('asks for everything when nothing is installed', () => {
    expect(ocrChoice(noOcr).size).toBe(68_000_000);
  });
});

describe('downloadQueue', () => {
  it('downloads every selected model that is missing, translation first', () => {
    const q = downloadQueue(buildFeatures(llm(), asr, noOcr, rec));
    expect(q.map((d) => [d.kind, d.id])).toEqual([
      ['llm', 'plamo-3-translate-2b'],
      ['ocr', 'ppocr'],
      ['llm', 'gemma-4-e2b'],
      ['asr', 'whisper-small']
    ]);
    expect(totalBytes(q)).toBe(1_800_000_000 + 68_000_000 + 3_100_000_000 + 190_000_000);
  });

  it('skips installed models and features left out', () => {
    const f = buildFeatures(llm(['plamo-3-translate-2b']), asr, { ppocr: true, current: true, ocrs: true, fast: true, dir: '' }, rec);
    f[3].enabled = false;
    expect(downloadQueue(f).map((d) => d.id)).toEqual(['gemma-4-e2b']);
  });

  it('counts only the bytes still missing from an interrupted download', () => {
    const l = llm();
    l.models[0].partial_bytes = 800_000_000;
    const q = downloadQueue(buildFeatures(l, asr, noOcr, rec));
    expect(q[0].size).toBe(1_000_000_000);
  });

  it('fetches a model shared by two features once', () => {
    const f = buildFeatures(llm(), asr, noOcr, rec);
    f[0].selected = 'gemma-4-e2b';
    const q = downloadQueue(f).filter((d) => d.id === 'gemma-4-e2b');
    expect(q).toHaveLength(1);
    expect(q[0].features).toEqual(['translate', 'assistant']);
  });
});

describe('replacementCandidates', () => {
  it('puts the recommended model first, then installed ones', () => {
    const models = llm(['gemma-4-e4b', 'plamo-3-translate-2b']).models;
    const ids = replacementCandidates(models, 'plamo-3-translate-2b', 'plamo-3-translate-0.8b').map((m) => m.id);
    expect(ids).toEqual(['plamo-3-translate-0.8b', 'gemma-4-e4b', 'gemma-4-e2b']);
  });
});

describe('hardwareLines', () => {
  const report = (over: Partial<SystemReport['profile']>, gpu = true): SystemReport => {
    const g = { name: 'RTX 4070', backend: 'Vulkan', memory_total: 12 * 1_073_741_824, integrated: false };
    return {
      profile: {
        os: 'windows',
        cpu_name: 'Ryzen 7',
        cpu_cores: 16,
        ram_total: 32 * 1_073_741_824,
        gpus: [g],
        disk_free: 200 * 1_073_741_824,
        use_gpu: true,
        ...over
      },
      accelerator: gpu ? g : null,
      recommended: rec
    };
  };

  it('names the GPU inference runs on', () => {
    const l = hardwareLines(report({}));
    expect(l.find((x) => x.label === 'GPU')?.value).toBe('RTX 4070（12.0 GB・Vulkan）');
    expect(l.find((x) => x.label === '推論処理')?.value).toBe('GPU：RTX 4070');
    expect(l.find((x) => x.label === 'メモリ')?.value).toBe('32.0 GB');
  });

  it('says when the GPU is switched off', () => {
    const l = hardwareLines(report({ use_gpu: false }, false));
    expect(l.find((x) => x.label === '推論処理')?.value).toBe('CPU（GPU 使用：無効）');
  });

  it('reports a CPU-only PC', () => {
    const l = hardwareLines(report({ gpus: [], disk_free: null }, false));
    expect(l.find((x) => x.label === 'GPU')?.value).toBe('非検出');
    expect(l.find((x) => x.label === '推論処理')?.value).toBe('CPU');
    expect(l.some((x) => x.label === '残容量')).toBe(false);
  });
});

describe('formatBytes', () => {
  it('uses GB from 1 GiB and MB below', () => {
    expect(formatBytes(1_807_203_040)).toBe('1.7 GB');
    expect(formatBytes(190_085_487)).toBe('181 MB');
  });
});

describe('inferenceDevice', () => {
  const profile = (gpus: SystemReport['profile']['gpus']): SystemReport['profile'] => ({
    os: 'windows',
    cpu_name: 'Ryzen 7',
    cpu_cores: 16,
    ram_total: 32 * 1_073_741_824,
    gpus,
    disk_free: null,
    use_gpu: true
  });
  const rtx = { name: 'RTX 4080', backend: 'Vulkan', memory_total: 16 * 1_073_741_824, integrated: false };
  const igpu = { name: 'UHD 770', backend: 'Vulkan', memory_total: 8 * 1_073_741_824, integrated: true };

  it('names every GPU the engine offloads to', () => {
    expect(inferenceDevice(profile([rtx]), true)).toBe('GPU：RTX 4080（Vulkan）');
    expect(inferenceDevice(profile([rtx, igpu]), true)).toBe('GPU：RTX 4080（Vulkan）、UHD 770（Vulkan）');
  });

  it('follows the unsaved GPU switch', () => {
    expect(inferenceDevice(profile([rtx]), false)).toBe('CPU（GPU 使用：無効）');
  });

  it('reports a PC without a usable GPU', () => {
    expect(inferenceDevice(profile([]), true)).toBe('CPU（対応 GPU：非検出）');
  });
});

describe('pendingConsents', () => {
  const withConsent = (l: LlmModels): LlmModels => ({
    ...l,
    models: l.models.map((m) => (m.style === 'pfml3' ? { ...m, consent: 'plamo-community' } : m))
  });

  it('lists a licence once for every queued model that needs it', () => {
    const f = buildFeatures(withConsent(llm()), asr, noOcr, rec);
    const q = downloadQueue(f);
    expect(pendingConsents(q, [])).toEqual(['plamo-community']);
  });

  it('is empty once accepted, or when the queue holds no such model', () => {
    const q = downloadQueue(buildFeatures(withConsent(llm()), asr, noOcr, rec));
    expect(pendingConsents(q, ['plamo-community'])).toEqual([]);
    expect(pendingConsents(downloadQueue(buildFeatures(llm(), asr, noOcr, rec)), [])).toEqual([]);
  });

  it('ignores models that are already installed', () => {
    const f = buildFeatures(withConsent(llm(['plamo-3-translate-2b'])), asr, noOcr, rec);
    expect(pendingConsents(downloadQueue(f), [])).toEqual([]);
  });
});
