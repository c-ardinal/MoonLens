> ⚠️ **2026-05-27 INVALID DATA WARNING** — Same caveat as the
> 2026-05-26-routed-* reports. The N=10 invariant PASS claim here
> used qwen2.5-1.5b for every tier due to the routing bug; the
> per-tier ordering reflects only per-tier sampling differences on
> a single model, not the routing matrix. See
> `docs/benchmarks/2026-05-27-routed-after-filename-fix.md`.

# Translate Quality Eval (routed)

**Samples per fixture**: 10

**Hardware**: Gpu

**Fixtures evaluated**: 1

## Invariant check: ✅ PASS (0.05-tolerance)

Quality ≥ Standard ≥ Compact holds for every (fixture, metric).

## UserTier ranking (sorted by id_score, supported-only mean)

| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |
|---|---:|---:|---:|---:|---:|---:|
| Quality | 0.55 | 0.99 | 0.78 | 100% | 0.0 | 10/10 |
| Standard | 0.48 | 0.89 | 0.92 | 100% | 0.1 | 10/10 |
| Compact | 0.46 | 0.86 | 0.78 | 100% | 27.1 | 10/10 |

## Per-UserTier × fixture detail

### UserTier::Quality

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.84 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.82 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (16/16) | 0.69 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ms(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.81 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.79 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 1.00 (16/16) | 0.73 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.79 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.81 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 0.94 (15/16) | 0.69 | yes | `2026-05-18T00:51:11.674022Z:ッシュの初期化：38ミリ秒（コンパイルされたバックエンドのキーード）` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.78 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |

### UserTier::Standard

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 1.00 (16/16) | 0.83 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.39 | 1.00 (16/16) | 0.76 | yes | `2026-05-18T00:51:11.674022Z:ットワークエンジンの初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.12 | 1.00 (16/16) | 1.66 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.39 | 1.00 (16/16) | 0.74 | yes | `2026-05-18T00:51:11.674022Z:ンチ後理の嵌入LLM:38ms(コンパイルされたバックエンドのキース)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 0.94 (15/16) | 0.67 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOとembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 0.94 (15/16) | 0.66 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 0.94 (15/16) | 0.74 | yes | `2026-05-18T00:51:11.674022Z:ッシュされたLLMの初期化：38秒(コンパイルされたバックエンドのキーング)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.67 | 0.94 (15/16) | 0.77 | yes | `2026-05-18T00:51:11.674022Z:ッシュされたLLMの初期化：38秒(コンパイルされたバックエンドのキーング)` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.17 | 0.19 (3/16) | 1.65 | yes | `2026-05-18T00:51:11.674022Z:INFO:ンコーーの初期化：38ミリ秒(embedded_llm_perf):2026-05-18T00` |
| log-rust-tracing-multi | logs | `Direct:hy-mt2-1.8b-q4_k_m` | 0.52 | 0.94 (15/16) | 0.75 | yes | `2026-05-18T00:51:11.674022Z:ッシュのLLM初期化：38ミリ秒（コンパイルされたバックエンドのキース）` |

### UserTier::Compact

| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |
|---|---|---|---:|---:|---:|:-:|---|
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.39 | 0.88 (14/16) | 0.66 | yes | `この文を日本語に翻します：` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.33 | 0.81 (13/16) | 0.55 | yes | `2026-05-18T00:51:11.674022Z:ーストアーテクチャの嵌入LLM初期化:38ミリ秒(INFOembedded_llm_perf:ーストアー` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.94 (15/16) | 0.68 | yes | `2026-05-18T00:51:11.674022Z:ンソルマッピングの初期化時間：38ms(INFOから)embedded_llm_perf` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.39 | 1.00 (16/16) | 0.78 | yes | `2026-05-18T00:51:11.674022Z:部のLLMを初期化:38ms(コンパイルされたバックエンドのキーード)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.33 | 0.12 (2/16) | 0.26 | yes | `2026-05-18T00:51:11.674022Z:備されたLSTM:38ミリ秒（コンパイルバックエンドのKernel）` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.10 | 1.00 (16/16) | 1.59 | yes | `2026-05-18T00:51:11.674022Z:ンコーーの初期化：38ミリ秒(INFOembedded_llm_perf)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 1.00 (16/16) | 0.89 | yes | `2026-05-18T00:51:11.674022Z:ンベロード・LLMの初期化：38ms(コンパイルされたバックエンドのキーング)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.39 | 1.00 (16/16) | 0.91 | yes | `2026-05-18T00:51:11.674022Z:想言語モデルの初期化:38イクロ秒(コンパイルされたバックエンドのニュールケース)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.94 (15/16) | 0.68 | yes | `2026-05-18T00:51:11.674022Z:想言語モデルの初期化:38イクロ秒(コンパイルされたバックエンドのケール)` |
| log-rust-tracing-multi | logs | `Direct:llm-jp-3-980m-instruct3-q4_k_m` | 0.67 | 0.94 (15/16) | 0.77 | yes | `2026-05-18T00:51:11.674022Z:ンベロード・LLM・ウォームアップ：38ms（コンパイルされたバックエンドのキーード）` |

## Fixture catalogue

| Id | Category | Direction | Length (chars) |
|---|---|---|---:|
| log-rust-tracing-multi | logs | en → ja | 535 |

