# MoonLens Speed Feasibility Benchmark — 2026-04-20 14:08:47 UTC

## Environment

- Host: `NYARLATHOTEP`
- OS: `windows x86_64`
- Iterations: 10 (warmup 3)
- Models: `gemma4:e2b`

## Fixtures

| name   | size     | png bytes |
| ------ | -------- | --------- |
| small  | 600x120  | 71459     |
| medium | 1000x250 | 141049    |
| large  | 1600x500 | 317472    |

## 1. Screen Capture

| label                                         |   n |   min |       P50 |   P95 |   P99 |   max |  mean |
| --------------------------------------------- | --: | ----: | --------: | ----: | ----: | ----: | ----: |
| capture_primary_full+png (entire monitor)     |  10 | 129ms | **139ms** | 173ms | 173ms | 173ms | 148ms |
| capture_global_rect(600x300)+png (lens-sized) |  10 |  78ms |  **88ms** | 104ms | 104ms | 104ms |  89ms |

## 2. Hide-sleep baseline (60ms constant)

| label               |   n |  min |      P50 |  P95 |  P99 |  max | mean |
| ------------------- | --: | ---: | -------: | ---: | ---: | ---: | ---: |
| thread::sleep(60ms) |  10 | 60ms | **60ms** | 60ms | 60ms | 60ms | 60ms |

## 3. OCR latency (OcrsProvider)

| label  |   n |   min |       P50 |   P95 |   P99 |   max |  mean |
| ------ | --: | ----: | --------: | ----: | ----: | ----: | ----: |
| small  |  10 | 229ms | **248ms** | 328ms | 328ms | 328ms | 253ms |
| medium |  10 | 344ms | **368ms** | 401ms | 401ms | 401ms | 373ms |
| large  |  10 | 756ms | **875ms** | 953ms | 953ms | 953ms | 869ms |

## 4. LLM first-token (Ollama)

| label      |   n |   min |       P50 |   P95 |   P99 |   max |  mean |
| ---------- | --: | ----: | --------: | ----: | ----: | ----: | ----: |
| gemma4:e2b |  10 | 359ms | **373ms** | 383ms | 383ms | 383ms | 373ms |

## 5. End-to-end (hide-sleep + OCR + LLM first-token)

| label  |   n |    min |        P50 |    P95 |    P99 |    max |   mean |
| ------ | --: | -----: | ---------: | -----: | -----: | -----: | -----: |
| small  |  10 |  664ms |  **684ms** |  745ms |  745ms |  745ms |  693ms |
| medium |  10 |  745ms |  **788ms** |  849ms |  849ms |  849ms |  795ms |
| large  |  10 | 1212ms | **1290ms** | 1390ms | 1390ms | 1390ms | 1308ms |

## Verdict

- category: **beta**
- E2E P50 (small): 684ms
- E2E P50 (medium): 788ms
- OCR P50 (small): 248ms

**Recommendation**: 続行前提、ただし S2 で DXGI + OCR 差し替え必須。ocrs 単独では SLO <0.5s は届かず。

## Decision rule (plan.md Q18)

- `alpha`: E2E P50 < 600ms → ocrs + Ollama で MVP 着手
- `beta`: 600-1200ms → 続行、S2 で DXGI + OCR 差し替え必須
- `gamma`: > 1200ms → 抜本再設計
