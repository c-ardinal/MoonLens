# OCR Engine Bench — Iteration 4 (Vertical-JA reading-order patch)

**Date**: 2026-05-14
**Change vs iter3**: added a column-reorder post-pass on the two bbox-aware engines (`ppocrv5-oar`, `manga-hybrid`). No model changes, no new fixtures — same 14 images, same models.

## What changed

A pure post-processing pass that, given the detected bboxes and their recognised text, decides whether the layout is tategaki and reorders the lines into Japanese vertical reading order (top→bottom within a column, right→left across columns). Heuristic:

- ≥ 4 boxes,
- ≥ 50 % of boxes are "char-cell-like" (h ≥ w · 0.5) — rules out sentence-shaped boxes from horizontal layouts,
- greedy x-clustering produces ≥ 2 columns with ≥ 2 boxes per column on average.

When any of those fails, the original detector order is returned unchanged — so horizontal fixtures are not touched.

Source: `src-tauri/examples/ocr_bench.rs::reorder_for_vertical_ja`. Applied inside `OarOcrEngine::run` and `MangaHybridEngine::run`. ocrs and `ppocrv5-mobile` are unchanged (bbox plumbing not worth the effort for engines we are not migrating to).

## Result

| Engine | iter3 ja-vertical CER | iter4 ja-vertical CER | Δ |
|---|---:|---:|---:|
| ocrs (no reorder) | 1.000 | 1.000 | — |
| ppocrv5-mobile (no reorder) | 0.970 | 0.970 | — |
| **ppocrv5-oar** | 0.909 | **0.121** | **−87 %** |
| **manga-hybrid** | 0.970 | **0.182** | **−81 %** |

## Non-regression

Per-language CER for `ppocrv5-oar`, iter3 → iter4:

| de | en | en-ja | fr | ja | ja-decor | **ja-vert** | ko | zh |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.000 → 0.000 | 0.005 → 0.005 | 0.003 → 0.003 | 0.034 → 0.034 | 0.034 → 0.034 | 0.000 → 0.000 | 0.909 → **0.121** | 1.000 → 1.000 | 0.000 → 0.000 |

Every horizontal / non-tategaki category is byte-identical to iter3. The heuristic correctly fires only on the vertical fixture.

## Output inspection

`tests/output_ocr_bench/ppocrv5-oar/synth_ja_vertical.json` detected text:

```
吾 辈 は 猫 で あ る 0 名 前 は ま だ 無 い o ど こ で 生 れ た か と ん と 見 当 が つ か ぬ °
```

Ground truth:

```
吾輩は猫である。 名前はまだ無い。 どこで生れたか とんと見当がつかぬ。
```

The reading order is now exactly right (all four columns in correct right-to-left sequence, each column top-to-bottom). The residual 12 % CER is **character recognition errors**, not order errors:

- `輩` → `辈` (Simplified Chinese variant of the same kanji — PaddleOCR training corpus bias)
- `。` (Japanese full stop) → `0` / `o` / `°` (the model maps the small circular glyph to several different lookalikes)

Both are training-data artefacts of PP-OCRv5 itself; fixing them would need either a Japanese-only recogniser (e.g. PaddleOCR's `japan_PP-OCRv5_mobile_rec`) or a domain-specific dictionary mask.

## Overall

| Engine | iter3 avg CER | iter4 avg CER |
|---|---:|---:|
| **ppocrv5-oar** | 0.143 | **0.087** |
| ocrs | 0.558 | 0.558 |
| ppocrv5-mobile | 0.658 | 0.658 |
| manga-hybrid | 0.731 | 0.675 |

`ppocrv5-oar` average drops from 0.143 to **0.087** — the improvement is entirely the vertical fixture; everything else is unchanged.

## Next phase-3 items (from iter3 report, still open)

1. ~~**Vertical Japanese reading-order**~~ — **done** (this iteration).
2. **Korean chain**: download `PaddlePaddle/korean_PP-OCRv5_mobile_rec` and route Hangul-containing detections.
3. **GPU bench**: enable `oar-ocr/cuda` and re-measure on RTX 4080.
4. **Production migration**: replace `OcrsProvider` with `OarOcrProvider` in `src-tauri/src/providers/`, port the reorder helper alongside.

## Reproduce

```powershell
pwsh scripts/run-ocr-bench.ps1
```
