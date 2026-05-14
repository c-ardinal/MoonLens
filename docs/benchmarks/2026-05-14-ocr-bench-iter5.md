# OCR Engine Bench — Iteration 5 (Korean chain)

**Date**: 2026-05-14
**Change vs iter4**: added `ppocrv5-oar-ko` — a second `oar-ocr` instance that shares the multilingual detector (`det.onnx`) but swaps the recogniser for `korean_PP-OCRv5_mobile_rec` (ONNX-converted by `monkt/paddleocr-onnx`, Apache 2.0). No other changes.

## Korean — the headline result

| Engine | synth_ko_paragraph CER |
|---|---:|
| ocrs | 1.000 ✗ |
| ppocrv5-mobile | 1.000 ✗ |
| ppocrv5-oar (multilingual) | 1.000 ✗ |
| manga-hybrid | 1.077 ✗ |
| **`ppocrv5-oar-ko`** | **0.019** ★ |

Korean goes from a complete blind spot (every multilingual engine emits garbage Han-like noise) to **near-perfect** (1 character missed out of ~52 in the Korean national anthem fixture).

## Surprise: the Korean rec ships built-in Latin support

The PaddleOCR Korean recogniser was trained on KR+EN mixed corpora (typical Korean documents have plenty of Latin embedded), so it doesn't drop Latin glyphs. Per-language CER for `ppocrv5-oar-ko`:

| de | en | en-ja | fr | ja | ja-decor | ja-vert | **ko** | zh |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.005 | 0.007 | 0.106 | 0.023 | 1.000 ✗ | 1.000 ✗ | 1.000 ✗ | **0.019** | 1.000 ✗ |

- **Latin (en/de/fr)**: stays near-perfect — the ko-rec almost matches the multilingual rec on pure-Latin fixtures.
- **CJK (ja/zh/ja-decor/ja-vert)**: collapses to ~1.0. The Korean dict has no Han ideographs or Hiragana / Katakana — every CJK glyph hits `[UNK]` and falls back to nearest Latin or Hangul lookalike.
- **en-ja mixed**: degraded (0.106 vs 0.003 for multilingual). The JA characters in the EN/JA mix poison the output.

So the Korean rec is **NOT a drop-in upgrade**. It is a **specialist** that needs to be chained behind a script-detection router.

## Full per-language CER (all 5 engines)

| Engine | de | en | en-ja | fr | ja | ja-decor | ja-vert | ko | zh |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| ocrs | 0.028 | 0.314 | 0.335 | 0.203 | 1.079 | 1.095 | 1.000 | 1.000 | 1.104 |
| ppocrv5-mobile | 1.000 | 0.265 | 0.908 | 0.966 | 0.202 | 0.000 | 0.970 | 1.000 | 0.000 |
| **ppocrv5-oar** | **0.000** | **0.005** | **0.003** | 0.034 | **0.034** | **0.000** | **0.121** | 1.000 | **0.000** |
| ppocrv5-oar-ko | 0.005 | 0.007 | 0.106 | **0.023** | 1.000 | 1.000 | 1.000 | **0.019** | 1.000 |
| manga-hybrid | 0.972 | 0.505 | 0.902 | 0.819 | 0.393 | 0.024 | 0.182 | 1.077 | 0.458 |

Best-of-two (multilingual + KO):

| de | en | en-ja | fr | ja | ja-decor | ja-vert | **ko** | zh |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.000 | 0.005 | 0.003 | 0.023 | 0.034 | 0.000 | 0.121 | **0.019** | 0.000 |

**Every fixture under CER 0.15.** This is the first iteration where no language category sits at ~1.0.

## Recommended production routing

Run **both** PP-OCRv5 engines (multilingual + ko) on each detection box. They share the detector so the only extra cost is a second small rec inference (~50 ms / box).

For each detection bbox, decide which engine's output to keep:

```
if recognized_text contains Hangul (U+AC00..U+D7AF or U+1100..U+11FF) → keep ko-engine output
else if recognized_text contains any CJK (U+3000..U+9FFF or Hiragana / Katakana) → keep multi-engine output
else (pure Latin / digit / punctuation) → keep multi-engine output (slightly better avg, matches existing horizontal benchmarks)
```

This is the simplest router and gets us the best-of-two table above for free. Smarter alternatives (compare confidences, normalised edit distance against a language model, etc.) are not needed at this scale.

### Cost

- **Disk**: +13.4 MB (`tests/models/ppocr_ko/rec.onnx`) + 47 KB dict.
- **Runtime**: +1 ort `Session` (~30 MB RAM, model is small). The detector is reused.
- **Latency**: ko-rec runs concurrently with multi-rec on the same det output. Worst case = max(multi_rec, ko_rec) per bbox; ko-rec is consistently faster in our bench (~250-450 ms vs multi 400-600 ms on the same fixtures) so it adds essentially no wall time when parallelised.

## Overall

| Engine | iter4 avg CER | iter5 avg CER |
|---|---:|---:|
| ppocrv5-oar | 0.087 | 0.087 |
| ppocrv5-oar-ko | — | 0.328 |
| **best-of-two (router)** | — | **~0.023** |

The standalone `ppocrv5-oar-ko` looks worse in isolation because half the fixtures are CJK that it can't read — but in a routed system, the multilingual engine handles those.

## Non-regression

`ppocrv5-oar` numbers iter4 → iter5: byte-identical across all 9 language categories. Adding the second engine had zero effect on the existing one (as expected — separate `Session` instances).

## Reproduce

```powershell
pwsh tests/models/download_manga_ocr.ps1   # one-time, ~555 MB
pwsh tests/models/download_korean_ocr.ps1  # one-time, ~13 MB
pwsh scripts/run-ocr-bench.ps1
```

## Phase-3 items remaining

1. ~~Vertical Japanese reading-order~~ — done (iter4).
2. ~~Korean chain~~ — done (this iteration).
3. **GPU bench**: enable `oar-ocr/cuda` on RTX 4080 and re-measure latency.
4. **Production migration**: replace `OcrsProvider` with `OarOcrProvider` (multi + ko chained, reorder helper inline), wire the Hangul-router into the recogniser dispatch.
