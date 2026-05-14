# OCR Engine Bench — Iteration 2 (Final)

**Date**: 2026-05-14
**Harness**: `cargo run --release --example ocr_bench`
**Fixtures**: 12 images under `tests/input/` — 2 marketing photos (en), 4 EN/JA mixed banners, 5 synthetic single-language paragraphs (ja/zh/ko/fr/de), 1 synthetic mixed jp+en
**Ground truth**: `tests/ocr_gt/*.json` (manual transcription)
**Hardware**: CPU only (Windows 11)

## Engines

| ID | Crate / Source | License | Backend |
|---|---|---|---|
| `ocrs` | `ocrs` 0.8 + `rten` 0.13 | MIT | Pure-Rust (rten tensor lib) |
| `ppocrv5-mobile` | `pure-onnx-ocr` 0.1 + PP-OCRv5 mobile ONNX | Apache 2.0 | Pure-Rust (tract-onnx) |
| `ppocrv5-oar` | `oar-ocr` 0.6 + PP-OCRv5 mobile ONNX | Apache 2.0 | ONNX Runtime via `ort` 2.0 |

The two PP-OCRv5 paths use the **same model files** (`det.onnx` 4.7 MB, `rec.onnx` 16.5 MB, `ppocrv5_dict.txt` with a trailing space appended). The only difference is the inference runtime.

`manga-ocr` deferred to a future iteration (task #113).

## Overall

| Engine | Runs | Avg CER ↓ | Avg WER ↓ | Avg Latency (ms) |
|---|---:|---:|---:|---:|
| ocrs | 12 | 0.477 | 0.631 | **533.3** |
| ppocrv5-mobile (`pure-onnx-ocr`) | 12 | 0.686 | 0.763 | 1639.7 |
| **ppocrv5-oar (`oar-ocr`)** | 12 | **0.091** | **0.172** | 910.0 |

`ppocrv5-oar` wins overall accuracy by a wide margin (5x lower CER than ocrs, 7x lower than pure-onnx-ocr) while staying close to ocrs latency.

## Per-language CER

| Engine | de | en | en-ja | fr | ja | ko | zh |
|---|---:|---:|---:|---:|---:|---:|---:|
| ocrs | 0.028 | 0.314 | 0.335 | 0.203 | 1.079 ✗ | 1.000 ✗ | 1.104 ✗ |
| ppocrv5-mobile | 1.000 ✗ | 0.265 | 0.908 | 0.966 ✗ | 0.202 | 1.000 ✗ | 0.000 |
| **ppocrv5-oar** | **0.000** | **0.005** | **0.003** | **0.034** | **0.034** | 1.000 ✗ | **0.000** |

`✗` = effectively unusable.

`ppocrv5-oar` is the only engine that handles every language we tested **except Korean**. ocrs is Latin-only in practice (fails outright on every CJK language). `pure-onnx-ocr` is CJK-only in practice (fails outright on every European language; works on Chinese specifically).

**Korean is a blind spot for both PP-OCRv5 paths** — the Baidu-shipped PP-OCRv5 multilingual model covers SC / TC / EN / JP but not KO. PaddleOCR ships a separate `korean_PP-OCRv5_mobile_rec` model on Hugging Face; wiring that in as a second recogniser would close the gap.

## Per-image highlights

| Image | Lang | Best engine | Best CER | ocrs CER | ppocrv5-oar CER |
|---|---|---|---:|---:|---:|
| 04ea5e (UltraNixie spec table) | en | ppocrv5-oar | **0.005** | 0.612 | 0.005 |
| 51fdac (UltraNixie marketing) | en | ppocrv5-oar | **0.004** | 0.015 | 0.004 |
| english0001 | en-ja | ppocrv5-oar | **0.000** | 0.393 | 0.000 |
| english0002 | en-ja | ppocrv5-oar | **0.000** | 0.307 | 0.000 |
| english0003 (Rainbow Passage) | en-ja | ppocrv5-oar | **0.000** | 0.065 | 0.000 |
| english0004 | en-ja | ppocrv5-oar | **0.010** | 0.206 | 0.010 |
| synth_de_paragraph | de | ppocrv5-oar | **0.000** | 0.028 | 0.000 |
| synth_fr_paragraph | fr | ppocrv5-oar | **0.034** | 0.203 | 0.034 |
| synth_ja_paragraph | ja | ppocrv5-oar | **0.034** | 1.079 ✗ | 0.034 |
| synth_ko_paragraph | ko | (none) | 1.000 ✗ | 1.000 ✗ | 1.000 ✗ |
| synth_mixed_jp_en | en-ja | ppocrv5-oar | **0.005** | 0.707 | 0.005 |
| synth_zh_paragraph | zh | ppocrv5-oar | **0.000** | 1.104 ✗ | 0.000 |

Full per-run JSON: `tests/output_ocr_bench/<engine>/<image>.json`.

## Why `pure-onnx-ocr` underperformed

Same PP-OCRv5 ONNX models, dramatically different results. Two factors:

1. **Detection coverage gap**: `pure-onnx-ocr` 0.1 hides the DBNet thresholds (`det_db_thresh`, `det_db_box_thresh`, `use_dilation`, `det_score_mode`) — only `det_limit_side_len`, `det_unclip_ratio`, and `rec_batch_size` are configurable. The default thresholds miss small body text on banner-shaped images (english0001 detected 4 lines out of 8 visible).
2. **Dictionary plumbing**: The shipped `ppocrv5_dict.txt` did not include a trailing space, so the recogniser emitted `[UNK]` tokens between words on Latin text. Appending a single space fixed Latin (en CER 0.640 → 0.265) but the detection gap persisted.

`oar-ocr` runs the same models via ONNX Runtime with PaddleOCR-faithful defaults — detection coverage is correct, recognition picks up spaces from the model's own space class, and recognition logits round-trip cleanly.

## Latency breakdown

| Engine | Min | Median | Max | Mean |
|---|---:|---:|---:|---:|
| ocrs | 257 ms | 480 ms | 1134 ms | 533 ms |
| ppocrv5-mobile | 703 ms | 1295 ms | 4025 ms | 1640 ms |
| ppocrv5-oar | 150 ms | 651 ms | 3100 ms | 910 ms |

`oar-ocr` is fast on small / simple images (150-400 ms for single-paragraph synth panels) and slower on dense text (3.1 s for the full Rainbow Passage). All numbers are CPU-only — GPU comparison (CUDA / DirectML) would require enabling the `cuda` feature on `oar-ocr` and a re-bench, deferred.

## Migration recommendation

**Adopt `oar-ocr` + PP-OCRv5 mobile as the production OCR engine.** Specifically:

- Replace the current `OcrsProvider` (or chain it as a Latin-only fast-path fallback) with a new `OarOcrProvider` that owns an `oar_ocr::prelude::OAROCR` instance and exposes the existing `Provider::Ocr` trait.
- Keep the existing model-download UX (`install_ocr_models` Tauri command) but point it at the PaddleOCR PP-OCRv5 mobile artifacts on Hugging Face (`PaddlePaddle/PP-OCRv5_mobile_det`, `PaddlePaddle/PP-OCRv5_mobile_rec`).
- Keep `pure-onnx-ocr` as a dev-only crate for layout-detection-only use in `lens_eval.rs` (its existing role); do **not** promote it to production.

### Costs

- **Binary size**: `ort` 2.0 bundles `onnxruntime.dll` (~50 MB Windows release). Acceptable for a desktop app; bigger concern on macOS (different DLL) and Linux (.so).
- **Build time**: cold `cargo build --release --features ocr-bench` adds ~6 minutes on first compile (mostly `ort` + `oar-ocr`). Subsequent incremental builds are fast.
- **License**: Apache 2.0 on both `oar-ocr` and `onnxruntime`. Compatible with MoonLens.
- **No GPU yet**: this iteration measured CPU only. GPU offload via `oar-ocr/cuda` (NVIDIA) or DirectML (any DX12 GPU) is a follow-up.

### Korean gap

Both PP-OCRv5 paths fail on Korean (the model doesn't ship Hangul classes). Options:

1. **Dedicated KO recogniser**: chain `PaddlePaddle/korean_PP-OCRv5_mobile_rec` after the shared detector. Same det.onnx, second rec.onnx routed when the detection box contains Hangul Unicode ranges (heuristic: pre-classify a single character via lightweight script detection).
2. **WinRT fallback on Windows**: `Windows.Media.Ocr` handles Korean natively on Windows 10+ machines.
3. **Defer**: ship without Korean and surface it as a known limitation in the language picker.

Recommendation: pick #1 as a phase-3 item; track via a follow-up task.

## Reproduce

```powershell
pwsh scripts/run-ocr-bench.ps1
# or directly:
cd src-tauri ; cargo run --release --example ocr_bench
```

Outputs:
- `tests/output_ocr_bench/<engine>/<image>.json` — per-run records
- `tests/output_ocr_bench/summary.json` — engine aggregates
- `tests/output_ocr_bench/summary.md` — markdown table (this file's data source)
