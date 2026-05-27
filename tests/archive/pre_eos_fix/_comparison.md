# OCR detector comparison

Detector | Mean total (ms) | Mean residual chars | Bottleneck stage | Quality gain
---|---:|---:|---|---
**WinRT only** (`none`) | 715 | 0.5 | WinRT OCR (318 ms) | baseline
**WinRT + ocrs** (`ocrs`) | 971 | 0.5 | ocrs detect (281 ms) | same
**WinRT + PaddleOCR** (`paddle`) | 3658 | 0.5 | paddle detect (2951 ms) | same

## Per-image residual fragments

Image | WinRT only | +ocrs | +paddle
---|---|---|---
04ea5e (watch UI) | 0 | 0 | 0
51fdac (product) | 0 | 0 | 0
english0001 | 0 | 0 | 0
english0002 | 0 | 0 | 0
english0003 | 3 chars `:ID` | 3 chars `:ID` | 3 chars `:ID`
english0004 | 0 | 0 | 0

## Conclusions

- **All three detectors leave the same residual** on english0003. The
  `:ID` fragment lies in a region neither WinRT, ocrs, nor PaddleOCR
  PP-OCRv5 detects — the pixels are too small / too low-contrast to
  pass any current text detector's threshold.
- **ocrs adds +256 ms** for zero quality improvement over WinRT.
- **PaddleOCR adds +2943 ms (>4× total latency)** for zero quality
  improvement. The PP-OCRv5 model is comprehensive but runs multiple
  full-image inference passes per frame (det + rec) in pure-Rust
  `tract-onnx`, which is much slower than WinRT's optimised pipeline.
- The remaining fragments are an **OCR ceiling**, not an inpainting
  issue. Further gains would require either (a) region-of-interest
  scaling to 4-8× around suspect areas, (b) a larger model, or
  (c) training a text-presence classifier on stroke patterns.

## Recommendation

Ship the **WinRT-only** (default `none`) pipeline. 715 ms/image is
within the Lens real-time budget, and residual quality is identical
to the heavier alternatives. Keep the secondary detectors behind
`MOONLENS_EVAL_DETECTOR` for offline experimentation.
