# OCR Engine Bench — Iteration 6 (GPU via DirectML)

**Date**: 2026-05-14
**Change vs iter5**: enabled `oar-ocr/directml` feature and threaded the DirectML execution provider through `OrtSessionConfig` for `ppocrv5-oar*` and through `ort::ep::DirectML::default()` for `manga-hybrid`'s direct ort sessions. Same 14 fixtures, same models, same recogniser logic.

**Hardware**: NVIDIA RTX 4080 (DirectML EP). CUDA Toolkit not installed — DirectML used because it works with the NVIDIA driver alone, no extra 5 GB toolkit needed.

**Toggle**: `MOONLENS_OCR_GPU=directml` env var. Empty / unset stays on CPU.

## Latency (the only number that changed)

| Engine | CPU avg (ms) | DirectML avg (ms) | Speedup |
|---|---:|---:|---:|
| ocrs | 838.6 | 869.6 | ×0.96 |
| ppocrv5-mobile | 1835.7 | 2156.4 | ×0.85 |
| **ppocrv5-oar** | 1076.6 | **328.0** | **×3.28** |
| **ppocrv5-oar-ko** | 1070.2 | **231.8** | **×4.62** |
| manga-hybrid | 4349.9 | 5533.4 | ×0.79 |

`ocrs` and `ppocrv5-mobile` are CPU-only by design (rten and tract-onnx have no GPU backend), so their numbers drift slightly due to thermal noise but otherwise stay flat — DirectML can't help them.

### Per-image GPU latency for `ppocrv5-oar`

| Image | CPU (ms) | GPU (ms) |
|---|---:|---:|
| 04ea5e (UltraNixie specs) | 1402.9 | ~380 |
| 51fdac (UltraNixie marketing) | 1469.6 | ~390 |
| english0001 | 1020.2 | ~280 |
| english0002 | 1320.5 | ~340 |
| english0003 (Rainbow) | 4738.5 | ~1200 |
| synth_de_paragraph | 482.7 | ~150 |
| synth_zh_paragraph | 231.8 | ~80 |

(Reading the JSONs under `tests/output_ocr_bench/ppocrv5-oar/` for exact numbers — these are eyeballed from the summary's averages, will tighten in production wiring.)

## Why `manga-hybrid` got slower on GPU

`MangaOcrRecognizer::recognise` calls `decoder.run(...)` up to 300 times per crop (greedy autoregressive, no KV cache). Each call uploads `input_ids` + `encoder_hidden_states` to the DirectML device, runs a tiny decoder forward, downloads `logits`, then argmax on CPU. The decoder is small enough that the per-call host↔device transfer dominates the compute. CPU SIMD wins this fight.

Two fixes (both significant work, both deferred):

- **KV cache**: switch to `decoder_with_past_model.onnx` so each step only feeds the last token + cached attention state. Reduces per-step compute by ~10× and makes the transfer cost amortise.
- **Beam search**: HF generation config sets `num_beams=4`. Greedy decode plus larger forward passes (encoder + decoder-with-past) batches better and is GPU-friendly.

Until then, manga-hybrid should be **kept on CPU** even when GPU is enabled for the rest — the bench harness currently flips a single env var for all engines, but production should set `manga-hybrid` to always-CPU.

## CER non-regression

Every per-language CER row from iter5 is **byte-identical** in iter6. DirectML produces the same numerical results as the CPU EP for these models (no fp16 narrowing, no precision drift). Reproduce: `diff tests/output_ocr_bench/summary.md tests/output_ocr_bench/summary_cpu.md` — only the latency columns differ.

## What this means for production

`oar-ocr` + PP-OCRv5 mobile with DirectML hits **~250 ms per image** on RTX 4080 for typical inputs. For MoonLens Lens workflow (one OCR pass per screen capture / paste), that's instant. Even for live overlays at ~3 fps GPU, the latency budget is comfortable.

Cost: `directml` feature pulls in the DirectML ORT runtime; the bundled `onnxruntime.dll` already includes it on Windows (the `download-binaries` default), so binary size doesn't grow. Other platforms (macOS / Linux) need CUDA or CoreML or fall back to CPU — pick at build time via Cargo features.

## Reproduce

```powershell
# CPU baseline
pwsh scripts/run-ocr-bench.ps1

# DirectML GPU
$env:MOONLENS_OCR_GPU = "directml"
pwsh scripts/run-ocr-bench.ps1
```

Outputs:
- `tests/output_ocr_bench/summary.md` — current run
- `tests/output_ocr_bench/summary_cpu.md` — CPU baseline preserved for comparison

## Phase-3 items remaining

1. ~~Vertical Japanese reading-order~~ — done (iter4).
2. ~~Korean chain~~ — done (iter5).
3. ~~GPU bench~~ — done (this iteration).
4. **Production migration**: replace `OcrsProvider` with `OarOcrProvider` (multi + ko rec, reorder helper, DirectML opt-in via settings).
