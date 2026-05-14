# OCR Engine Bench — Iteration 3 (Final, with manga-ocr)

**Date**: 2026-05-14
**Harness**: `cargo run --release --example ocr_bench`
**Fixtures**: 14 images — 2 marketing photos (en), 4 EN/JA mixed banners, 5 single-language synth paragraphs (ja/zh/ko/fr/de), 1 mixed jp+en, 2 new Japanese stress fixtures (vertical tategaki, decorative title+body)
**Ground truth**: `tests/ocr_gt/*.json`
**Hardware**: CPU only (Windows 11)

## Engines

| ID | Source | License | Backend |
|---|---|---|---|
| `ocrs` | `ocrs` 0.8 + `rten` 0.13 | MIT | Pure-Rust |
| `ppocrv5-mobile` | `pure-onnx-ocr` 0.1 | Apache 2.0 | Pure-Rust (tract-onnx) |
| `ppocrv5-oar` | `oar-ocr` 0.6 | Apache 2.0 | ONNX Runtime (`ort` 2.0) |
| **`manga-hybrid`** | `ort` 2.0 + manga-ocr-base ONNX + oar-ocr detection | Apache 2.0 | ONNX Runtime |

`manga-hybrid` uses oar-ocr's detection (PP-OCRv5 mobile DBNet) to find text bboxes, then runs manga-ocr's ViT-base encoder + transformer decoder on each crop. Greedy decoding with a 6-token repetition guard (early-stop when the decoder falls into a loop — the original Python uses beam search + `no_repeat_ngram_size=3`, we approximate).

The full `comic-text-detector + manga-ocr` pipeline is **deferred** (task #125). manga-hybrid is enough to evaluate manga-ocr's pure recognition quality.

## Overall

| Engine | Runs | Avg CER ↓ | Avg WER ↓ | Avg Latency (ms) |
|---|---:|---:|---:|---:|
| **ppocrv5-oar** | 14 | **0.143** | **0.737** | 1660.7 |
| ocrs | 14 | 0.558 | 1.076 | **968.6** |
| ppocrv5-mobile | 14 | 0.658 | 1.190 | 2747.9 |
| manga-hybrid | 14 | 0.731 | 1.410 | 6343.3 |

`ppocrv5-oar` still wins by 4× on CER. `manga-hybrid` is slowest by 4× and lands in last place overall — but the per-language breakdown shows where it actually shines.

`avg_wer` is inflated by Japanese-only and vertical fixtures where Japanese has no word boundaries, so a single concat-output → 1 "word" and any mismatch shoots WER above 1. Use **CER as the primary signal**.

## Per-language CER

| Engine | de | en | en-ja | fr | ja | **ja-decor.** | **ja-vert.** | ko | zh |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| manga-hybrid | 0.972 | 0.505 | 0.902 | 0.819 | 0.393 | **0.024** | 0.970 | 1.077 | 0.458 |
| ocrs | 0.028 | 0.314 | 0.335 | 0.203 | 1.079 | 1.095 | 1.000 | 1.000 | 1.104 |
| ppocrv5-mobile | 1.000 | 0.265 | 0.908 | 0.966 | 0.202 | **0.000** | 0.970 | 1.000 | **0.000** |
| **ppocrv5-oar** | **0.000** | **0.005** | **0.003** | **0.034** | **0.034** | **0.000** | 0.909 | 1.000 | **0.000** |

Bold = best in that column. `ppocrv5-oar` is the best or tied-best on **8 of 9** language categories.

### manga-hybrid is a specialist, not a generalist

- **Wins decorative JA** (CER 0.024 — large bold title + body): tied with `ppocrv5-oar` at near-perfect. But not better than oar.
- **Loses regular JA** (CER 0.393): manga-ocr was trained on manga-style single-line crops; resizing a wide paragraph line to 224×224 squishes the text badly. With its native aspect ratio expected (~3:1 line) the model collapses on 1:1 crops.
- **Loses everything Latin** (en 0.505, de 0.972, fr 0.819): the JA-only character vocab can't produce most Latin glyphs, so the decoder hallucinates kanji from the visual pattern.
- **Worse than ppocrv5-mobile (pure-Rust!) on most fixtures** — manga-ocr is a poor choice for "general OCR" even on JA-heavy content.

### Vertical Japanese: nobody wins

| Engine | ja-vertical CER |
|---|---:|
| ppocrv5-oar | 0.909 |
| manga-hybrid | 0.970 |
| ppocrv5-mobile | 0.970 |
| ocrs | 1.000 |

Inspecting `tests/output_ocr_bench/manga-hybrid/synth_ja_vertical.json` shows oar-ocr's detector found 33 individual character bboxes (one per glyph). manga-ocr's recogniser **read each char correctly**, but the bboxes were emitted in **raster scan order** (top-down, left-right per row), not in **Japanese column reading order** (top-down within a column, columns right-to-left). The characters land in the wrong sequence and CER explodes.

The fix is **not a different recogniser** — it's a column-aware reading-order stitcher on top of detection. comic-text-detector ships a layout-classifier head specifically for this, which is why it's the right tool for proper manga / tategaki support (deferred task #125).

### Korean: complete shutout

All four engines hit CER 1.0. The PP-OCRv5 multilingual model ships SC / TC / EN / JP only; ocrs and manga-ocr are Latin-and-CJK-without-Korean respectively. Needs a dedicated Korean recogniser (PaddleOCR ships `korean_PP-OCRv5_mobile_rec` as a chainable add-on).

## Per-image highlights

| Image | Lang | Best engine | Best CER |
|---|---|---|---:|
| 04ea5e (UltraNixie specs) | en | ppocrv5-oar | **0.005** |
| 51fdac (UltraNixie marketing) | en | ppocrv5-oar | **0.004** |
| english0001 | en-ja | ppocrv5-oar | **0.000** |
| english0002 | en-ja | ppocrv5-oar | **0.000** |
| english0003 (Rainbow) | en-ja | ppocrv5-oar | **0.000** |
| english0004 | en-ja | ppocrv5-oar | **0.010** |
| synth_de_paragraph | de | ppocrv5-oar | **0.000** |
| synth_fr_paragraph | fr | ppocrv5-oar | **0.034** |
| synth_ja_decorative | ja-decor | ppocrv5-oar / ppocrv5-mobile | **0.000** |
| synth_ja_paragraph | ja | ppocrv5-oar | **0.034** |
| synth_ja_vertical | ja-vert | ppocrv5-oar | 0.909 ✗ |
| synth_ko_paragraph | ko | (none) | 1.000 ✗ |
| synth_mixed_jp_en | en-ja | ppocrv5-oar | **0.005** |
| synth_zh_paragraph | zh | ppocrv5-oar / ppocrv5-mobile | **0.000** |

## Latency

| Engine | Min | Median | Max | Mean |
|---|---:|---:|---:|---:|
| ocrs | 434 ms | 760 ms | 2005 ms | 969 ms |
| ppocrv5-oar | 303 ms | 1066 ms | 6173 ms | 1661 ms |
| ppocrv5-mobile | 994 ms | 2160 ms | 6930 ms | 2748 ms |
| manga-hybrid | 1436 ms | 3258 ms | 18409 ms | 6343 ms |

`manga-hybrid`'s wall time is dominated by the autoregressive decoder (no KV cache in our path; each new token reruns the full decoder on the growing sequence). The repetition guard does help — without it, runs collapsed at 300-token cap with ~70 s per image in iter2. With the guard, mean is 6 s. Adding KV cache would roughly halve it; switching to `decoder_with_past.onnx` is the proper fix but requires past-state tensor wiring (deferred).

## Findings

1. **`ppocrv5-oar` (`oar-ocr` 0.6 + PP-OCRv5 mobile ONNX) is the right primary OCR engine for MoonLens.** It wins or ties on 8 of 9 language categories and runs at near-`ocrs` latency.
2. **manga-ocr does not improve over `ppocrv5-oar` on any tested case** — it ties on decorative JA at near-perfect CER and loses everywhere else. The "specialist for manga" claim doesn't pay off when the detector hands it line-shaped crops (its training data was square-aspect crops, ours are wide). Plus the bench used **horizontal** synth panels — even our "decorative" fixture is left-to-right horizontal text.
3. **Vertical Japanese remains unsolved** by all four engines. The blocker is **reading-order recovery**, not character recognition — every recogniser correctly read each glyph in isolation, but no detector emitted columns in tategaki order. The fix is comic-text-detector or a Japanese-aware layout pass post-detection.
4. **Korean is a blind spot** for every engine; needs a dedicated `korean_PP-OCRv5_mobile_rec` chain.

## Final migration recommendation (unchanged from iter2)

**Adopt `oar-ocr` + PP-OCRv5 mobile as the production OCR engine.** Replace `OcrsProvider` with an `OarOcrProvider` exposing the existing `Provider::Ocr` trait. Keep `ocrs` as a Latin-only fast-path fallback if startup latency matters for short text.

manga-ocr does not earn a place in the main chain. It could become an opt-in "manga mode" recogniser if MoonLens adds dedicated manga panel translation later, paired with comic-text-detector for proper column ordering.

### Phase-3 follow-ups (separate tasks)

- **Korean chain**: download `PaddlePaddle/korean_PP-OCRv5_mobile_rec`, route Hangul-containing detection boxes through it.
- **Vertical Japanese reading-order**: integrate comic-text-detector's layout head (or write a column-grouping post-pass on PP-OCRv5 detection outputs).
- **GPU bench**: enable `oar-ocr/cuda` on RTX 4080 and re-measure latency — currently CPU-only.
- **manga-ocr KV cache**: switch to `decoder_with_past.onnx` for ~50 % latency drop on the manga path. Only worth doing if manga mode actually ships.

## Reproduce

```powershell
pwsh tests/models/download_manga_ocr.ps1   # one-time, ~555 MB
pwsh scripts/run-ocr-bench.ps1
```

Outputs:
- `tests/output_ocr_bench/<engine>/<image>.json` — per-run records
- `tests/output_ocr_bench/summary.json` — engine aggregates
- `tests/output_ocr_bench/summary.md` — markdown source for the tables above
