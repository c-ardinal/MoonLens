# Model Runtimes — Design Notes

This doc tracks which inference runtimes MoonLens ships, and how we plan
to extend support beyond the decoder-only LLMs we have today. It exists
because picking a runtime locks in the architecture (encoder-decoder
support, tokenizer format, GPU acceleration) for years; adding a runtime
mid-product is far more disruptive than adding a model.

## Status (2026-05-09)

| Runtime | Used for | Status |
|---|---|---|
| `mistral.rs::GgufModelBuilder` | Decoder-only LLMs (Qwen 2.5, Gemma, Llama) | **Shipped** — feature-gated by `embedded-llm` |
| ONNX (encoder-decoder) | Helsinki Opus-MT, MADLAD-400 | **Designed only** — see below |

The shipped runtime covers the Standard / Compact / Quality tiers in
`model_registry::TIERS`, all on Apache 2.0 weights. No Helsinki / MADLAD
support yet; those are tracked here as a future tier.

## Why a second runtime is on the roadmap

The current decoder-only runtime can serve hosts down to ~4 GB RAM via
the Compact tier (Qwen 2.5 0.5B Q4_K_M, ~400 MB), but two real use cases
push past what that envelope can do well:

1. **Truly low-spec PCs** (Atom-class CPUs, 2 GB RAM laptops). At the
   400 MB Compact tier the model still needs ~1 GB working set during
   inference. Helsinki Opus-MT models are ~80 MB **per language pair**
   and run comfortably on these hosts because they are dedicated MT
   models, not general-purpose chat LLMs.
2. **License purity at the smallest tier**. Opus-MT is **MIT** (the
   cleanest possible OSS license), useful for the most paranoid
   corporate procurement reviews where even Apache-2.0 raises questions.

## Design — proposed `embedded.opusmt.<src>-<tgt>` provider

Provider id pattern: `embedded.opusmt.ja-en`, `embedded.opusmt.en-ja`,
`embedded.opusmt.zh-ja`, etc. One provider instance per language pair
because Helsinki publishes one model per pair (the architecture
choice — they did not train an n-to-n model in the original release).

Pipeline integration mirrors `EmbeddedLlmTranslator`:

```
EmbeddedOpusMtTranslator
├── id: "embedded.opusmt.ja-en"
├── source_lang / target_lang  (parsed from id)
├── model_dir: <app-data>/moonlens/models/opusmt-ja-en/
│   ├── encoder_model.onnx
│   ├── decoder_model.onnx
│   ├── decoder_with_past_model.onnx
│   ├── tokenizer.json   (HuggingFace tokenizers format)
│   ├── vocab.json
│   └── source.spm / target.spm  (SentencePiece artifacts)
└── engine: tokio::sync::Mutex<Option<Arc<OpusMtEngine>>>  (lazy load)
```

`Pipeline::run` / `run_with_sink` already routes by capability, so as
long as the new provider implements the existing `Provider` trait with
`run_streaming`, the lens / capture flows pick it up via chain config.

## Required dependencies (not yet added)

| Crate | Purpose | License | Notes |
|---|---|---|---|
| `tract-onnx` | ONNX inference | MIT/Apache-2.0 | Already a dev-dep for PaddleOCR; promoting to a runtime dep adds CPU-only inference at ~zero binary cost |
| `tokenizers` | SentencePiece + BPE tokenization | Apache-2.0 | HuggingFace's pure-Rust tokenizer; required for Opus-MT vocab |
| `ndarray` | Tensor manipulation around tract | MIT/Apache-2.0 | tract already depends on it |

Estimated added compile time: ~20 % on `cargo build --features embedded-llm-onnx`.
Binary size impact: ~3-5 MB (tract is small).

## Required code (rough work breakdown)

1. **Add provider scaffold** — `providers/translate_opusmt.rs` with a
   stub `EmbeddedOpusMtTranslator` whose `run` returns
   `ProviderError::Other("opusmt not yet implemented")`. Wire into
   `pipeline::rebuild_providers` for `embedded.opusmt.*` ids. This step
   alone is ~2 hours and unblocks the rest of the plan.
2. **Tokenizer pipeline** — load `tokenizer.json` via `tokenizers`,
   handle the source/target SentencePiece pair, encode/decode round trip
   tests. ~1-2 days.
3. **Encoder pass** — load encoder ONNX, run inference, cache the
   encoder output across decoder steps. ~2 days.
4. **Decoder loop** — iterative decoder ONNX call with past-key-value
   caching (`decoder_with_past_model.onnx`), greedy decoding plus an
   optional length-normalised beam (Opus-MT is sensitive to length
   penalties). ~3-4 days.
5. **Streaming integration** — emit per-token deltas through
   `StreamingChunkSink` so the Lens UI lights up identically for Opus-MT
   and the LLM tier. ~1 day.
6. **Multi-pair model registry** — extend `model_registry::TIERS` with
   a `RuntimeKind` enum (`Llm` | `OpusMt`) so the picker shows pair
   selectors for Opus-MT, and wire downloads to fetch the multi-file
   bundle (4-6 files per pair) instead of a single GGUF. ~1-2 days.
7. **Packaging compatibility** — `embedded_model_package` currently
   bundles a single GGUF. For Opus-MT it needs to bundle a directory.
   Either (a) ship a v2 manifest with multi-file support, or (b) tar
   the directory before zipping. ~1 day.

Total: ~2-3 weeks for a working prototype, plus an additional 1 week
for the QA we would want before declaring it shipped (hand-translated
test set, BLEU regression vs the LLM tier, perf bench on weak HW).

## Out of scope for this design pass

* **MADLAD-400** — same architecture family as Opus-MT (T5-style
  encoder-decoder) so the runtime work above covers it once the
  scaffolding lands. Adding MADLAD is then "register a new tier in
  the registry and ship the GGUF/ONNX bundles."
* **GPU acceleration for ONNX** — defer until CPU perf on the target
  weak hardware is measured; ORT-with-CUDA is not a hard requirement
  because Opus-MT is small enough that CPU is competitive.
* **Encoder-decoder LLMs** (e.g. T5-XXL, Flan-T5) — runtime work
  overlaps but the model files are huge and the use case (research /
  experimentation) does not match the offline-corporate-deployment
  brief.

## Decision log

* **2026-05-09**: Defer Opus-MT to a separate workstream. Standard /
  Compact / Quality tiers via Qwen 2.5 are sufficient to ship the
  current v0.1 milestone (offline-first, license-clean, multi-tier).
  Opus-MT support targets v0.2 once the LLM-tier path is battle-tested
  in restricted-network deployments.
