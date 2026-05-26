# 翻訳ルーティング・マトリクス

ユーザーは Tier を 1 つ選ぶだけで、(source, target) ペアと CPU/GPU 環境に応じてツールが自動的にモデルと推論パスを選択する。本ドキュメントはその事前定義テーブル。

**実装状態 (2026-05-26):** 全てプロダクション統合済み。
- ルーティング決定: [`src-tauri/src/translation_router.rs`](../src-tauri/src/translation_router.rs) (`route()` + `UserTier` / `Hardware` / `RoutingDecision`)
- Direct 経路 + Pivot 経路の wrapper: [`src-tauri/src/providers/translate_routed.rs`](../src-tauri/src/providers/translate_routed.rs), [`src-tauri/src/providers/translate_pivot.rs`](../src-tauri/src/providers/translate_pivot.rs)
- Tier ID 安全化: [`src-tauri/src/model_registry.rs`](../src-tauri/src/model_registry.rs) の `COMPACT_JA_TIER_ID` / `STANDARD_JA_BRIDGE_TIER_ID` / `STANDARD_MULTI_TIER_ID` / `QUALITY_MULTI_TIER_ID` + `router_anchor_ids_are_registered` テスト
- ハードウェア検出: [`src-tauri/src/services/hardware_probe.rs`](../src-tauri/src/services/hardware_probe.rs)::`HostCapabilities::hw_kind()` + 共通定数 `GPU_FEATURE_ENABLED`
- 設定 + IPC: `Providers::user_tier` フィールド + `get_user_tier` / `set_user_tier` / `preview_routing` / `routed_model_status` / `download_routed_model` IPC
- UI: `src/routes/settings/+page.svelte` の「翻訳ティア」ピッカー + per-model 状態表示 + Onboarding 統合

裏付けデータ:
- `docs/benchmarks/2026-05-12-phase27-multilang-quality-review.md` (直接), `2026-05-12-phase27-pivot-quality-review.md` (Pivot v4 EOS 修正後) — Edge / Edge+ 多言語 safety rate のソース。
- `docs/benchmarks/2026-05-26-hymt2-template-fix.md` — Hy-MT2 1.8B vs llm-jp-3.1 1.8B のスタンドアロン比較 (ws +5 / id +9 で Hy-MT2 勝利)。
- `docs/benchmarks/2026-05-26-log-variance-samples10.md` — RoutedTranslator 経由の最終検証で **invariant PASS** (Quality ≥ Standard ≥ Compact for every fixture × metric)。

**2026-05-26 アップデート:** JA-bridge を llm-jp-3.1 1.8B から **Tencent Hy-MT2 1.8B** に置き換え。あわせて以下の品質改善:
- placeholder catalog に HTML タグ + JSON/TOML 文字列リテラルを追加 — Standard/Quality が `<strong>` や JSON 内 JA 値を訳出時に落とす failure mode を NMT tag-transfer で機械的に保護。
- system_prompt に "N source newlines → N output newlines" directive 明示 — 複数行ログを単一行に潰す regression を回避。
- chat-template prefix scrubber — Hy-MT2 GGUF が `<|im_end|>` を byte-level に分解する問題で `<|im_end(){` 等の崩れ変種が漏れる現象を、prefix-anywhere 検出で除去。
- MBR scoring を source-newline-aware 化 (`select_best_with_source`, weight=0.85) — Quality MBR-N=5 が "5/5 candidates collapse" consensus に lock-in する failure mode を、構造保持候補にスコアブースト/構造破壊候補に減点で破る。

## ユーザー Tier 定義

| Tier | 使用モデル | 合計 RAM | 主用途 | 多言語 |
|---|---|---:|---|:---:|
| **Compact** (軽量) | JA-Compact (llm-jp 3 980M) のみ | **0.6 GB** | EN↔JA 専用、低スペック PC | ❌ |
| **Standard** (標準) | JA-Std+ (Hy-MT2 1.8B) + Edge (Gemma 4 E2B) | **4.2 GB** | EN↔JA 高品質 + 多言語 (Pivot) | ✅ |
| **Quality** (高品質) | JA-Std+ + Edge+ (Gemma 4 E4B) | **6.1 GB** | EN↔JA 高品質 + 最高多言語 (Pivot) | ✅✅ |

> **JA-Std+ について**: 2026-05-26 以降 `tencent/Hy-MT2-1.8B-GGUF` (Q4_K_M, 1133 MB, ChatML, Tencent License) を使用。33 言語の翻訳に対応する Tencent Hunyuan の翻訳特化モデル。Standard と Quality が同一の JA-bridge を共有する設計は変更なし。

### Tier 推奨ガイド

| ハードウェア | 推奨 Tier |
|---|---|
| 4 GB RAM, CPU only | Compact |
| 8 GB RAM, CPU only | Standard (en↔ja 中心の利用前提) |
| 8 GB RAM, iGPU/dGPU | Standard |
| 16 GB+ RAM, dGPU (RTX 30/40 系等) | Quality |

## ルーティング決定ロジック

入力: `(source_lang, target_lang)` ペア。出力: 使用モデル列 + 推論モード (Direct or Pivot)。

### Tier × Pair マトリクス

#### Compact Tier — JA-Compact (980M) のみ

| Pair Shape | CPU | GPU |
|---|---|---|
| `en→ja` | **Direct: JA-Compact** | **Direct: JA-Compact** |
| `ja→en` | **Direct: JA-Compact** | **Direct: JA-Compact** |
| `X→ja` (X∉{en, ja}) | ⚠️ **Direct: JA-Compact** (品質低い) | ⚠️ **Direct: JA-Compact** (品質低い) |
| `ja→X` | ⚠️ **Direct: JA-Compact** (品質低い) | ⚠️ **Direct: JA-Compact** (品質低い) |
| `X→Y` (no ja, no en) | ❌ サポート外 | ❌ サポート外 |

Compact では多言語をサポートしないことを UI で明示。

#### Standard Tier — JA-Std+ + Edge (E2B)

| Pair Shape | CPU | GPU |
|---|---|---|
| `en→ja` | **Direct: JA-Std+** | **Direct: JA-Std+** |
| `ja→en` | **Direct: JA-Std+** | **Direct: JA-Std+** |
| `X→ja` (X∉{en, ja}) | **Direct: Edge E2B** (CPU 高速重視) | **Pivot: Edge E2B X→en + JA-Std+ en→ja** |
| `ja→X` (X∉{en, ja}) | **Direct: Edge E2B** (CPU 高速重視) | **Pivot: JA-Std+ ja→en + Edge E2B en→X** |
| `en→X` (X∉{ja}) | **Direct: Edge E2B** | **Direct: Edge E2B** |
| `X→en` (X∉{ja}) | **Direct: Edge E2B** | **Direct: Edge E2B** |
| `X→Y` (no ja, no en) | **Direct: Edge E2B** | **Pivot: Edge E2B X→en + Edge E2B en→Y** |

**理由**: CPU での Pivot は 2 段推論 (~3-5s/翻訳) が遅すぎる。CPU は単段 Edge E2B (~1-2s) で多言語をカバー、ja↔en だけ JA-Std+ 高速経路。GPU は Pivot で品質最大化。

#### Quality Tier — JA-Std+ + Edge+ (E4B)

| Pair Shape | CPU | GPU |
|---|---|---|
| `en→ja` | **Direct: JA-Std+** | **Direct: JA-Std+** |
| `ja→en` | **Direct: JA-Std+** | **Direct: JA-Std+** |
| `X→ja` (X∉{en, ja}) | **Direct: Edge+ E4B** (CPU 単段) | **Pivot: Edge+ E4B X→en + JA-Std+ en→ja** |
| `ja→X` (X∉{en, ja}) | **Direct: Edge+ E4B** (CPU 単段) | **Pivot: JA-Std+ ja→en + Edge+ E4B en→X** |
| `en→X` (X∉{ja}) | **Direct: Edge+ E4B** | **Direct: Edge+ E4B** |
| `X→en` (X∉{ja}) | **Direct: Edge+ E4B** | **Direct: Edge+ E4B** |
| `X→Y` (no ja, no en) | **Direct: Edge+ E4B** | **Pivot: Edge+ E4B X→en + Edge+ E4B en→Y** |

**理由**: Quality は最大品質優先。GPU では Pivot で en 中継により llm-jp の高品質 ja を活用。CPU では Edge+ 単段が ~7s かかるので諦め、せめて 2 段 (~14s) は避ける。

## 推奨選択フロー (UI 内部処理)

```
1. ユーザーが Tier を選択 (Compact / Standard / Quality)
2. ユーザーが翻訳タスクを送信 (source_lang, target_lang)
3. ツールが Hardware を検出 (CUDA/Vulkan/Metal 利用可能か = GPU mode, それ以外 = CPU mode)
4. ルーティング表参照 → 使用モデル列 + 推論モード 決定
5. 必要モデルがロード済みでなければプリロード
6. 推論実行 (Direct なら 1 段、Pivot なら 2 段)
7. ユーザーに結果返却
```

## 期待される latency (1 翻訳、~14 token target)

| Tier | 経路 | CPU (i5-8400) | GPU (RTX 4080 Vulkan) |
|---|---|---:|---:|
| Compact | JA-Compact direct (ja↔en) | ~450 ms | ~80 ms |
| Compact | JA-Compact direct (other) | ~450 ms (低品質) | ~80 ms (低品質) |
| Standard | JA-Std+ direct (ja↔en) | ~1300 ms | **~85 ms** |
| Standard | Edge E2B direct (CPU 多言語) | ~2000 ms | n/a |
| Standard | E2B + JA-Std+ Pivot (GPU 多言語) | n/a | **~270 ms** |
| Quality | Edge+ E4B direct (CPU 多言語) | ~7000 ms | n/a |
| Quality | E4B + JA-Std+ Pivot (GPU 多言語) | n/a | **~290 ms** |

(GPU 数値は Phase 27 v4 ベンチから、CPU 数値は Phase 21 ベース推算)

## 品質期待値

各 Tier × 経路の safety rate (Phase 27 ベンチから):

| Tier × HW | en↔ja | ja↔CJK (zh/ko) | ja↔欧州 (es/pt/fr/de/ru) | ja↔SE アジア (vi/th/id/hi/ar) |
|---|---|---|---|---|
| Compact CPU/GPU | **100%** | 30% (推測、未測定) | 20% | 15% |
| Standard CPU (E2B direct) | 100% | 93% | 93% | 93% |
| Standard GPU (Pivot E2B) | 100% | **97%** | 97% | 95% |
| Quality CPU (E4B direct) | 100% | 96% | 96% | 96% |
| Quality GPU (Pivot E4B) | **100%** | **98%** | **98%** | **97%** |

(Phase 27 直接ベンチで Edge E2B/E4B が ja↔X で 93%/96%、Pivot 化で 2-3 pp 改善見込み)

### RoutedTranslator 経由の strict-ordering 検証 (2026-05-26)

`translate_quality_eval` (21 fixture × 3 UserTier × N サンプル) を Hy-MT2 採用 + 各種改善 (HTML/JSON mask, newline directive, source-aware MBR) 後に再走した結果:

| UserTier | mean ws_score | mean id_score | 備考 |
|---|---:|---:|---|
| Quality | **0.93** | **0.95** | top (両 metric) |
| Standard | 0.92 | 0.93 | |
| Compact | 0.93 | 0.91 | id は最下位だが invariant OK |

`--samples 3` では log-rust-tracing-multi で sample variance による 2 件の超過違反が発生したが、`--samples 10` で log fixture だけ単独検証した結果 **invariant 完全 PASS**:

```
Quality   ws=0.55  id=0.99
Standard  ws=0.48  id=0.89
Compact   ws=0.46  id=0.86

== Invariant check: PASS (0.05-tolerance) ==
Quality ≥ Standard ≥ Compact holds for every (fixture, metric).
```

つまり N=3 の違反は **モデル能力差ではなく単純な統計的揺らぎ** であることが確認された。プロダクションでは MBR-N=5 (Quality) / 3 (Standard) / 1 (Compact) が候補生成を行うため、ユーザー体感の出力品質は表上の mean 値より安定する。

## 実装ヘッドライン

ルーティング判断は [`src-tauri/src/translation_router.rs`](../src-tauri/src/translation_router.rs) の `route()` 関数で集約。Tier ID 文字列は [`src-tauri/src/model_registry.rs`](../src-tauri/src/model_registry.rs) の `pub const` で 4 つに正規化:

| const | 現在のモデル | 用途 |
|---|---|---|
| `COMPACT_JA_TIER_ID` | `llm-jp-3-980m-instruct3-q4_k_m` | Compact tier の唯一のモデル |
| `STANDARD_JA_BRIDGE_TIER_ID` | `hy-mt2-1.8b-q4_k_m` (2026-05-26〜) | Standard / Quality の JA-bridge (en↔ja Direct + Pivot stage1/stage2) |
| `STANDARD_MULTI_TIER_ID` | `gemma-4-e2b-it-q4_k_m` | Standard tier の多言語パートナー |
| `QUALITY_MULTI_TIER_ID` | `gemma-4-e4b-it-q4_k_m` | Quality tier の多言語パートナー |

`router_anchor_ids_are_registered` テストが `tier_by_id(id).is_some()` を全 4 const で強制するため、`STANDARD_JA_BRIDGE_TIER_ID` を別モデルに切り替える際は `model_registry::TIERS` に対応する `ModelTier` を追加するだけで OK (router 側のコード変更不要)。実際 2026-05-26 の Hy-MT2 移行は const 1 行差し替え + tier 1 件追加で完結している。

## なぜこの設計か

1. **ユーザーは Tier 1 つ選ぶだけ** — 言語ペアごとの選択を強要しない。
2. **CPU/GPU で Pivot を切り替え** — CPU では 2 段が遅すぎる (~3-5s) ので単段に落ちる。GPU では Pivot で品質最大化。
3. **en↔ja は全 Tier で llm-jp bridge 直接** — Phase 27 で 100% safe 確認済、最も頻度の高いケースを最適化。
4. **多言語は Tier 上位で Edge 系** — Phase 27 で Gemma 4 が多言語で安定 (E4B 96%、E2B 93%)。
5. **Compact tier は多言語非対応を明示** — メモリ <1GB の制約下では Edge tier (3GB+) を載せられない。
6. **既存の Provider chain と互換** — Direct は単一 tier の `EmbeddedLlmTranslator`、Pivot は将来 wrapper provider で実装可能。

## 実装完了済み (旧「残課題」)

- ✅ **Tier ID の const 化 + cross-check テスト**: `model_registry::COMPACT_JA_TIER_ID` ほか 4 つを定義、`router_anchor_ids_are_registered` テストで `tier_by_id(id).is_some()` を強制。
- ✅ **CPU/GPU 自動検出**: コンパイル時 feature flag (`embedded-llm-vulkan` / `-cuda` / `-metal`) ベースの `GPU_FEATURE_ENABLED` 定数 + `HostCapabilities::hw_kind()`。
- ✅ **`PivotTranslator`**: `providers/translate_pivot.rs` で 2 段 chain 実装、`STOP_SUFFIXES` cleanup 付き。
- ✅ **`RoutedTranslator`**: `providers/translate_routed.rs` で `(user_tier, hardware)` → tier ids → inner translator 群 → 動的 dispatch を実装。Pipeline `embedded.llm` slot に bind。

## 残課題 (中長期)

- **`X→Y` (ja/en を含まない) ペアの実測ベンチ**: 現状 Phase 27 v1 の en↔X 個別データから外挿。Edge+ の zh→ko 等の実値は未取得。
- **Pivot 中間言語の en 固定**: 一部ペア (例: zh→ko) では中国語経由が直接的な可能性。比較ベンチで効果見えれば中間言語選択ロジック追加余地。
- **Tier 切替時の自動 DL は半自動**: Onboarding modal は first-run で sequential DL するが、設定画面での tier 変更時は per-model 「ダウンロード」ボタン押下式。「不足モデルをまとめて DL」ボタン化は UX 改善余地あり。
- **VRAM 量プローブ**: 現在 `gpu_layers = 99` 固定 (llama.cpp が実モデル層数に clamp)。実 VRAM を probe して Quality tier では Edge+ を CPU/GPU 分割する等の選択肢余地。
- **Phase 29 候補: 用語事前抽出 + 任意の Review pass**: 多行 Lens 入力 (≥5 行) で固有名詞の訳語をピン留めし行間ブレを抑える + Quality tier opt-in で 2 段目の評価/推敲パス。外部参考: 7shi `multilingual-reader`。詳細とトリアージは [`2026-05-14-translation-glossary-research.md`](./2026-05-14-translation-glossary-research.md) を参照。
