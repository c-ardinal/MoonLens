# 翻訳ルーティング・マトリクス

ユーザーは Tier を 1 つ選ぶだけで、(source, target) ペアと CPU/GPU 環境に応じてツールが自動的にモデルと推論パスを選択する。本ドキュメントはその事前定義テーブル。

**実装状態 (2026-05-14):** 全てプロダクション統合済み。
- ルーティング決定: [`src-tauri/src/translation_router.rs`](../src-tauri/src/translation_router.rs) (`route()` + `UserTier` / `Hardware` / `RoutingDecision`)
- Direct 経路 + Pivot 経路の wrapper: [`src-tauri/src/providers/translate_routed.rs`](../src-tauri/src/providers/translate_routed.rs), [`src-tauri/src/providers/translate_pivot.rs`](../src-tauri/src/providers/translate_pivot.rs)
- Tier ID 安全化: [`src-tauri/src/model_registry.rs`](../src-tauri/src/model_registry.rs) の `COMPACT_JA_TIER_ID` / `STANDARD_JA_BRIDGE_TIER_ID` / `STANDARD_MULTI_TIER_ID` / `QUALITY_MULTI_TIER_ID` + `router_anchor_ids_are_registered` テスト
- ハードウェア検出: [`src-tauri/src/services/hardware_probe.rs`](../src-tauri/src/services/hardware_probe.rs)::`HostCapabilities::hw_kind()` + 共通定数 `GPU_FEATURE_ENABLED`
- 設定 + IPC: `Providers::user_tier` フィールド + `get_user_tier` / `set_user_tier` / `preview_routing` / `routed_model_status` / `download_routed_model` IPC
- UI: `src/routes/settings/+page.svelte` の「翻訳ティア」ピッカー + per-model 状態表示 + Onboarding 統合

裏付けデータ: `docs/benchmarks/2026-05-12-phase27-multilang-quality-review.md` (直接), `2026-05-12-phase27-pivot-quality-review.md` (Pivot v4 EOS 修正後)。

## ユーザー Tier 定義

| Tier | 使用モデル | 合計 RAM | 主用途 | 多言語 |
|---|---|---:|---|:---:|
| **Compact** (軽量) | JA-Compact (llm-jp 3 980M) のみ | **0.6 GB** | EN↔JA 専用、低スペック PC | ❌ |
| **Standard** (標準) | JA-Std+ (llm-jp 3.1 1.8B) + Edge (Gemma 4 E2B) | **4.2 GB** | EN↔JA 高品質 + 多言語 (Pivot) | ✅ |
| **Quality** (高品質) | JA-Std+ + Edge+ (Gemma 4 E4B) | **6.1 GB** | EN↔JA 高品質 + 最高多言語 (Pivot) | ✅✅ |

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

## 実装ヘッドラインの定数化案

```rust
// src/translation_router.rs
pub enum UserTier { Compact, Standard, Quality }
pub enum Hardware { Cpu, Gpu }
pub enum RoutingDecision {
    Direct { tier_id: &'static str },
    Pivot { stage1_tier: &'static str, stage2_tier: &'static str },
    Unsupported,
}

pub fn route(
    user_tier: UserTier,
    hw: Hardware,
    src: &str,
    tgt: &str,
) -> RoutingDecision {
    use UserTier::*;
    use Hardware::*;
    let is_ja = |s: &str| s.eq_ignore_ascii_case("ja") || s.eq_ignore_ascii_case("japanese");
    let is_en = |s: &str| s.eq_ignore_ascii_case("en") || s.eq_ignore_ascii_case("english");

    match (user_tier, is_ja(src), is_ja(tgt), is_en(src), is_en(tgt)) {
        // en↔ja 系は全 Tier 共通: JA bridge を直接使用
        (Compact, true, false, _, true) | (Compact, false, true, true, _) =>
            RoutingDecision::Direct { tier_id: "llm-jp-3-980m-instruct3-q4_k_m" },
        (Standard | Quality, true, false, _, true) | (Standard | Quality, false, true, true, _) =>
            RoutingDecision::Direct { tier_id: "llm-jp-3.1-1.8b-instruct4-q4_k_m" },

        // Compact: ja↔X (X != en) は直接の劣化版を使用
        (Compact, _, _, _, _) =>
            RoutingDecision::Direct { tier_id: "llm-jp-3-980m-instruct3-q4_k_m" },

        // Standard/Quality 多言語の Pivot 経路
        (tier, true, false, _, _) =>  // ja → X (X != en)
            match (tier, hw) {
                (Standard, Cpu) => RoutingDecision::Direct { tier_id: "gemma-4-e2b-it-q4_k_m" },
                (Standard, Gpu) => RoutingDecision::Pivot {
                    stage1_tier: "llm-jp-3.1-1.8b-instruct4-q4_k_m",
                    stage2_tier: "gemma-4-e2b-it-q4_k_m",
                },
                (Quality, Cpu) => RoutingDecision::Direct { tier_id: "gemma-4-e4b-it-q4_k_m" },
                (Quality, Gpu) => RoutingDecision::Pivot {
                    stage1_tier: "llm-jp-3.1-1.8b-instruct4-q4_k_m",
                    stage2_tier: "gemma-4-e4b-it-q4_k_m",
                },
                _ => unreachable!(),
            },
        (tier, false, true, _, _) =>  // X → ja (X != en)
            match (tier, hw) {
                (Standard, Cpu) => RoutingDecision::Direct { tier_id: "gemma-4-e2b-it-q4_k_m" },
                (Standard, Gpu) => RoutingDecision::Pivot {
                    stage1_tier: "gemma-4-e2b-it-q4_k_m",
                    stage2_tier: "llm-jp-3.1-1.8b-instruct4-q4_k_m",
                },
                (Quality, Cpu) => RoutingDecision::Direct { tier_id: "gemma-4-e4b-it-q4_k_m" },
                (Quality, Gpu) => RoutingDecision::Pivot {
                    stage1_tier: "gemma-4-e4b-it-q4_k_m",
                    stage2_tier: "llm-jp-3.1-1.8b-instruct4-q4_k_m",
                },
                _ => unreachable!(),
            },
        // Non-ja 多言語 (en↔X や X↔Y) — Standard/Quality は単段 Edge / Edge+ 直接、Compact は未対応
        (Standard, _, _, _, _) =>
            RoutingDecision::Direct { tier_id: "gemma-4-e2b-it-q4_k_m" },
        (Quality, _, _, _, _) =>
            RoutingDecision::Direct { tier_id: "gemma-4-e4b-it-q4_k_m" },
    }
}
```

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
