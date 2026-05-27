# MoonLens Test Assets

This directory hosts every test fixture, ground truth, baseline and
per-run output that the MoonLens test / bench / eval toolchain
touches. Industry conventions adopted:

- **Snapshot testing** (Jest / pytest-regressions / `insta`) —
  committed golden baselines in `snapshots/`, fresh runs in
  `runs/`, diff against the baseline.
- **ML experiment tracking** — gitignored `runs/` per execution,
  committed `archive/` for historical reference points worth
  keeping for provenance.

## Layout

```
tests/
├── README.md                ← you are here
├── input/                   ← committed test images
│   ├── *.png                ← real screenshots (md5-hashed names + numbered)
│   └── synth/*.png          ← gen_synth_inputs.rs output (deterministic)
├── ocr_gt/                  ← committed OCR ground truth (JSON, 1:1 with input/)
├── models/                  ← LOCAL model cache (gitignored)
│   └── download_*.ps1       ← committed download scripts
├── snapshots/               ← committed golden baselines
│   ├── ocr_bench/           ← OCR CER/WER baseline (current)
│   └── lens_eval/           ← Lens 3-way tier baseline (current)
├── runs/                    ← gitignored per-execution scratch dir
│   ├── ocr_bench/           ← latest ocr_bench output
│   ├── lens_eval/           ← latest lens_eval output (default tier)
│   └── lens_eval_3way/      ← per-tier subdirs from the 3-way harness
└── archive/                 ← committed historical baselines for provenance
    └── pre_eos_fix/         ← lens_eval snapshot from before the
                                Phase 27 EOS fix (kept as a "what
                                we looked like before" reference)
```

## Test categories

| # | Category | Files | External dep | When to run |
|---|---|---|---|---|
| 1 | Unit tests | `src-tauri/src/**/#[cfg(test)]` | none | every commit (`cargo test --lib`) |
| 2 | Integration (no-dep) | `src-tauri/tests/pipeline_flow.rs`, `plugin_*.rs`, `settings_file_roundtrip.rs`, `storage_persistence.rs` | none | every commit (`cargo test --test '*'`) |
| 3 | E2E live | `src-tauri/tests/lens_pipeline_live.rs` | Ollama running + `gemma4:e2b` pulled + ocrs models | manual, before pipeline-shape changes (`cargo test --release --test lens_pipeline_live -- --ignored --nocapture`) |
| 4 | Lens quality eval | `src-tauri/examples/lens_eval.rs`, `translate_quality_eval.rs` | embedded GGUFs in `<app-data>/models/` | before release; after any translate path change |
| 5 | OCR bench | `src-tauri/examples/ocr_bench.rs` | `tests/models/` (download via PowerShell scripts) | after OCR provider changes |
| 6 | Throughput bench | `src-tauri/examples/bench_*.rs` | embedded GGUFs | after tier registry or sampling changes |

## Standard invocations

Every command assumes you are in the repo root.

### 1. Unit + (2) integration tests (every commit)

```pwsh
# Unit tests + no-dep integration tests:
cd src-tauri
cargo test --features embedded-llm-vulkan
```

### 3. Lens pipeline live E2E (manual)

```pwsh
# Needs Ollama (`gemma4:e2b` pulled) and ocrs models in app-data.
cd src-tauri
cargo test --release --features embedded-llm-vulkan `
           --test lens_pipeline_live -- --ignored --nocapture
```

### 4a. Lens quality eval (single tier)

```pwsh
cargo build --example lens_eval --features embedded-llm-vulkan --release
src-tauri\target\release\examples\lens_eval.exe
# Output: tests\runs\lens_eval\
```

### 4b. Lens 3-way tier comparison

```pwsh
pwsh -File scripts/run-lens-eval-3way.ps1
# Output: tests\runs\lens_eval_3way\<tier>\

pwsh -File scripts/compare-lens-eval-3way.ps1
# Output: tests\runs\lens_eval_3way\_compare.md
```

### 4c. Routed translate quality eval

```pwsh
cd src-tauri
cargo run --release --example translate_quality_eval `
          --features embedded-llm-vulkan -- `
          --samples 3 --write-report ..\docs\benchmarks\YYYY-MM-DD-routed-XYZ.md
```

### 5. OCR bench

```pwsh
# Default — writes to tests\runs\ocr_bench\
pwsh -File scripts/run-ocr-bench.ps1

# Wipe previous output:
pwsh -File scripts/run-ocr-bench.ps1 -Clean

# Refresh the committed baseline (after reviewing the diff):
pwsh -File scripts/run-ocr-bench.ps1 -OutputDir tests/snapshots/ocr_bench
```

### 6. Throughput bench

```pwsh
cd src-tauri
# Pick whichever is relevant; all accept --samples and --write-report.
cargo run --release --example bench_all_tiers `
          --features embedded-llm-vulkan -- --samples 5 `
          --write-report ..\docs\benchmarks\YYYY-MM-DD-all-tiers.md
cargo run --release --example bench_multilang_tiers `
          --features embedded-llm-vulkan -- ...
cargo run --release --example bench_specialist_translation `
          --features embedded-llm-vulkan -- ...
```

## Snapshot workflow (golden file testing)

1. Make code change.
2. Run the relevant bench / eval against `tests/runs/<category>/`.
3. Diff `tests/runs/<category>/summary.md` against
   `tests/snapshots/<category>/summary.md`.
4. If the change is intended and the regression review passes,
   refresh the baseline by overwriting `tests/snapshots/<category>/`
   with the new run output. **Commit the new baseline along with
   the code change** so reviewers see both.
5. If the change is unintended, fix the code and re-run.

Quick PowerShell snippet to bless a fresh ocr_bench run as the new
baseline:

```pwsh
Remove-Item -Recurse -Force tests\snapshots\ocr_bench
Move-Item tests\runs\ocr_bench tests\snapshots\ocr_bench
git add tests/snapshots/ocr_bench
```

## Gitignore conventions

- `tests/runs/**` is gitignored entirely (regenerable from inputs).
- `tests/snapshots/**/*.png` and `tests/archive/**/*.png` are
  gitignored. Only the JSON metrics and the markdown summaries are
  committed — the side-by-side comparison PNGs are large and re-
  derivable from the inputs.
- `tests/models/` is gitignored (large binary models). The download
  scripts (`tests/models/download_*.ps1`) are committed so anyone
  can repopulate the cache.

## Ground truth coverage

`tests/ocr_gt/` carries one JSON per input image. As of the
2026-05-27 audit, the following input images do **not** yet have
ground truth and the OCR bench will silently skip them:

- `synth_long_paragraph.png`
- `synth_short_lines.png`
- `synth_small_body.png`
- `synth_title_body.png`
- `synth_two_columns.png`
- `synth_ui_buttons.png`

Adding GT for these is task #182 — they are layout-pattern fixtures
useful for catching regressions in card sizing, text alignment, and
column detection.

## Historical context

`docs/benchmarks/` collects human-readable reports from individual
bench runs (one markdown file per dated run). The 52 reports there
are kept as a decision-trail; consult them when "why did we switch
from X to Y at the time" matters. The 2026-05-26 / 2026-05-27
batch ("routed eval + Hy-MT2 adoption") contains an INVALID DATA
WARNING block on the entries that were measured before the
`filename_remote → model_path` routing bug was fixed. The current
trusted baseline is `2026-05-27-routed-after-filename-fix.md`.

## When this README is wrong

Code paths are the source of truth. If `lens_eval.rs` ever points
its output somewhere this README does not mention, the example
takes precedence — file an issue and the README should be
updated to match.
