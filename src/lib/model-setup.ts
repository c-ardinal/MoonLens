/**
 * 機能別モデル選択の共通処理：初回セットアップの選択肢・取得計画、
 * PC 構成の表示文、削除時の代替候補。UI 非依存（単体テスト対象）。
 */
import type {
  SystemProfile,
  AsrModels,
  LlmModels,
  OcrModelsStatus,
  Recommendation,
  SystemReport
} from '$lib/ipc';
import { t } from './i18n';

export type Feature = 'translate' | 'ocr' | 'assistant' | 'asr';

/** 選択肢 1 件。取得・未取得は内部状態（表示はラジオ選択が主体）。 */
export interface Choice {
  id: string;
  label: string;
  /** 取得容量（バイト）。 */
  size: number;
  installed: boolean;
  /** 中断した取得の既存分（再開時は残りのみ取得）。 */
  partial: number;
  note: string;
  /** 取得前に同意が必要なライセンス id（不要なら null）。 */
  consent: string | null;
}

export interface FeaturePlan {
  feature: Feature;
  title: string;
  /** 翻訳・文字認識：MoonLens の動作に必須（取得対象から除外不可）。 */
  required: boolean;
  choices: Choice[];
  /** ラジオの選択値（常に 1 件）。 */
  selected: string;
  recommended: string;
  reason: string;
  /** 今回の一括取得の対象。 */
  enabled: boolean;
}

/** PP-OCRv6（31 MB）・予備の ocrs（12 MB）・高速実行部品（ONNX Runtime、約 25 MB）を一括取得。 */
export const OCR_CHOICE_ID = 'ppocr';
const OCR_MODEL_BYTES = 31_000_000;
const OCRS_BYTES = 12_000_000;
const OCR_FAST_BYTES = 25_000_000;

/** 機能名（日本語の原文）。表示時は `t()` で翻訳。 */
export const FEATURE_TITLES: Record<Feature, string> = {
  translate: '翻訳',
  ocr: '文字認識',
  assistant: '説明・要約',
  asr: '音声認識'
};

export function ocrChoice(ocr: OcrModelsStatus): Choice {
  // 旧版で取得済みの環境：文字認識は利用可能、不足分のみ取得。
  const usable = ocr.ppocr && ocr.ocrs;
  const outdated = usable && !ocr.current;
  const fastOnly = usable && ocr.current && !ocr.fast;
  return {
    id: OCR_CHOICE_ID,
    label: 'PP-OCRv6',
    size:
      (ocr.current ? 0 : OCR_MODEL_BYTES) + (ocr.ocrs ? 0 : OCRS_BYTES) + (ocr.fast ? 0 : OCR_FAST_BYTES) ||
      OCR_MODEL_BYTES + OCRS_BYTES + OCR_FAST_BYTES,
    installed: usable && ocr.current && ocr.fast,
    partial: 0,
    note: outdated
      ? '旧世代モデル（PP-OCRv5）で動作中。新世代モデルが未取得（取得後は読取精度が向上、旧モデルは自動削除）'
      : fastOnly
        ? '高速実行部品が未取得（取得後は読取を高速化、環境別の最適構成は自動選択）'
        : '日本語・中国語・英語に対応（予備の英語専用 ocrs を同梱）',
    consent: null
  };
}

/** 追加可能な文字体系の表示名と説明（id：バックエンドの ocr_scripts）。 */
export const OCR_SCRIPTS: Record<string, { label: string; note: string }> = {
  korean: { label: '韓国語', note: 'ハングルの認識モデル' },
  cyrillic: { label: 'キリル文字', note: 'ロシア語・ウクライナ語・ブルガリア語・カザフ語等の認識モデル' },
  arabic: { label: 'アラビア文字', note: 'アラビア語・ペルシア語・ウルドゥー語の認識モデル' },
  thai: { label: 'タイ語', note: 'タイ文字の認識モデル' },
  devanagari: { label: 'デーヴァナーガリー', note: 'ヒンディー語・マラーティー語・ネパール語の認識モデル' },
  tamil: { label: 'タミル語', note: 'タミル文字の認識モデル' },
  telugu: { label: 'テルグ語', note: 'テルグ文字の認識モデル' }
};

const pick = <
  T extends {
    id: string;
    label: string;
    size: number;
    installed: boolean;
    partial_bytes: number;
    note: string;
    consent?: string | null;
  }
>(
  m: T
): Choice => ({
  id: m.id,
  label: m.label,
  size: m.size,
  installed: m.installed,
  partial: m.partial_bytes,
  note: m.note,
  consent: m.consent ?? null
});

/** 推奨が候補外（カタログ変更等）の場合は先頭候補へ退避。 */
function initial(choices: Choice[], recommended: string | undefined, current: string): string {
  if (recommended && choices.some((c) => c.id === recommended)) return recommended;
  if (choices.some((c) => c.id === current)) return current;
  return choices[0]?.id ?? '';
}

/** 初回セットアップの機能別選択肢。初期選択は推奨モデル。 */
export function buildFeatures(
  llm: LlmModels,
  asr: AsrModels,
  ocr: OcrModelsStatus,
  rec: Recommendation | null
): FeaturePlan[] {
  const translate = llm.models.map(pick);
  const assistant = llm.models.filter((m) => m.style === 'chat_template').map(pick);
  const speech = asr.models.map(pick);
  const plan = (
    feature: Feature,
    choices: Choice[],
    current: string,
    required: boolean
  ): FeaturePlan => {
    const r = rec?.[feature];
    const selected = initial(choices, r?.model, current);
    return {
      feature,
      title: FEATURE_TITLES[feature],
      required,
      choices,
      selected,
      recommended: r?.model ?? '',
      reason: r?.reason ?? '',
      enabled: true
    };
  };
  return [
    plan('translate', translate, llm.selected, true),
    plan('ocr', [ocrChoice(ocr)], OCR_CHOICE_ID, true),
    plan('assistant', assistant, llm.assistant, false),
    plan('asr', speech, asr.selected, false)
  ];
}

export interface DownloadItem {
  /** 取得対象のモデルを使用する機能（同一モデルの重複取得は 1 件へ集約）。 */
  features: Feature[];
  id: string;
  label: string;
  /** 今回の取得量（中断分の再開時は残りのみ）。 */
  size: number;
  kind: 'llm' | 'asr' | 'ocr';
  /** 取得前に同意が必要なライセンス id（不要なら null）。 */
  consent: string | null;
}

const kindOf = (f: Feature): DownloadItem['kind'] =>
  f === 'ocr' ? 'ocr' : f === 'asr' ? 'asr' : 'llm';

/** 一括取得の対象：取得対象かつ未取得の選択モデル。翻訳を最優先。 */
export function downloadQueue(features: FeaturePlan[]): DownloadItem[] {
  const out: DownloadItem[] = [];
  for (const f of features) {
    if (!f.enabled) continue;
    const c = f.choices.find((x) => x.id === f.selected);
    if (!c || c.installed) continue;
    const kind = kindOf(f.feature);
    const same = out.find((d) => d.id === c.id && d.kind === kind);
    if (same) same.features.push(f.feature);
    else
      out.push({
        features: [f.feature],
        id: c.id,
        label: c.label,
        size: Math.max(0, c.size - c.partial),
        kind,
        consent: c.consent
      });
  }
  return out;
}

/** 取得前に同意が未了のライセンス id（取得順、重複なし）。 */
export function pendingConsents(items: DownloadItem[], accepted: string[]): string[] {
  const out: string[] = [];
  for (const d of items) {
    if (d.consent && !accepted.includes(d.consent) && !out.includes(d.consent)) out.push(d.consent);
  }
  return out;
}

export const totalBytes = (items: { size: number }[]) => items.reduce((n, d) => n + d.size, 0);

/** 容量表記：1 GB 以上は小数 1 桁の GB、未満は MB。 */
export function formatBytes(n: number): string {
  if (n >= 1_073_741_824) return `${(n / 1_073_741_824).toFixed(1)} GB`;
  return `${Math.max(1, Math.round(n / 1_048_576))} MB`;
}

export interface HardwareLine {
  label: string;
  value: string;
}

/** PC 構成の表示行（初回セットアップの診断欄）。 */
export function hardwareLines(r: SystemReport): HardwareLine[] {
  const p = r.profile;
  const cpu = p.cpu_name
    ? t('{name}（{n} スレッド）', { name: p.cpu_name, n: p.cpu_cores })
    : t('{n} スレッド', { n: p.cpu_cores });
  const gpus = p.gpus.length
    ? p.gpus
        .map((g) =>
          t(g.integrated ? '{name}（{memory}・{backend}・内蔵）' : '{name}（{memory}・{backend}）', {
            name: g.name,
            memory: formatBytes(g.memory_total),
            backend: g.backend
          })
        )
        .join(t('、'))
    : t('非検出');
  let device: string;
  if (r.accelerator) device = t('GPU：{name}', { name: r.accelerator.name });
  else if (!p.use_gpu && p.gpus.length) device = t('CPU（GPU 使用：無効）');
  else device = 'CPU';
  const lines: HardwareLine[] = [
    { label: 'CPU', value: cpu },
    { label: t('メモリ'), value: formatBytes(p.ram_total) },
    { label: 'GPU', value: gpus },
    { label: t('推論処理'), value: device }
  ];
  if (p.disk_free != null) lines.push({ label: t('残容量'), value: formatBytes(p.disk_free) });
  return lines;
}

/**
 * 翻訳・音声認識の推論デバイス（設定画面の表示）。内蔵エンジンは検出した
 * GPU 全てを使用するため、全 GPU 名を列挙。`useGpu` は「GPU 使用」の現在値。
 */
export function inferenceDevice(p: SystemProfile, useGpu: boolean): string {
  if (!p.gpus.length) return t('CPU（対応 GPU：非検出）');
  if (!useGpu) return t('CPU（GPU 使用：無効）');
  return t('GPU：{name}', {
    name: p.gpus.map((g) => t('{name}（{backend}）', { name: g.name, backend: g.backend })).join(t('、'))
  });
}

/**
 * 使用中モデルの削除前に提示する代替候補。推奨モデルを先頭、
 * 次に取得済みモデル、残りは元の順序。
 */
export function replacementCandidates<T extends { id: string; installed: boolean }>(
  models: T[],
  removing: string,
  recommended: string | undefined
): T[] {
  const rest = models.filter((m) => m.id !== removing);
  const rank = (m: T) => (m.id === recommended ? 0 : m.installed ? 1 : 2);
  return rest
    .map((m, i) => ({ m, i }))
    .sort((a, b) => rank(a.m) - rank(b.m) || a.i - b.i)
    .map((x) => x.m);
}
