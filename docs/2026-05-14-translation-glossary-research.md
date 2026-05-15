# Translation Glossary & Review-pass Research

**Date**: 2026-05-14
**Source**: 7shi, "ローカル LLM による翻訳実験" — <https://zenn.dev/7shi/scraps/da63f326aed58d>
**Repo**: <https://github.com/7shi/multilingual-reader>
**Status**: research notes — implementation deferred to **Phase 29**

## Context

While auditing the iter-by-iter OCR migration work in Phase 28, we
read a Zenn scrap from 7shi on multilingual translation evaluation
using local LLMs. The author runs Podcast-length corpora through
67 target languages and reports per-language quality scores
(90+ = high quality, 80–89 = usable). The pipeline is interesting
because it uses **three distinct LLM passes** with different roles,
rather than the single-pass we currently do in MoonLens.

This document captures the relevant techniques and our triage
decisions: which apply to MoonLens, which do not, and what would
land where.

## The 7shi pipeline

Three sub-commands, distinct prompts, potentially distinct models:

1. **`trtools term`** — terminology pre-extraction.
   Scans the full input text once and emits a `{source → target}`
   glossary for proper nouns, technical terms, and recurring phrases.
   The glossary is injected as a system-prompt fragment for the
   downstream translate pass.

2. **`trtools translate`** — base translation.
   Uses the glossary from step 1 to pin terminology. Keeps a
   "summary-compressed history" so each line's translation sees a
   condensed context window rather than the raw running transcript.
   Direct output, no chain-of-thought reasoning emitted.

3. **`trtools review`** — second-pass critique.
   A different model is asked to evaluate the translate-pass output
   ("does this read naturally?", "is the terminology consistent?",
   "is there language contamination?") and rewrite if needed.

Model selection in the article:

- **Gemma 4 (26B)** — robust generator. Used in the translate pass.
- **Qwen 3.6 (35B)** — sharp analyst. Used in the review pass.

Asymmetric on purpose: one model is hard to break in generation,
the other is hard to fool in evaluation. Improvement examples cited:
Czech 78 → 94 on the per-language score after adding the review
pass.

## Mapping to MoonLens

MoonLens has three translation flows:

- **Lens overlay** — single screenshot, latency-critical (<1 s target).
- **Lens watcher** — continuous monitoring, near-real-time.
- **Audio transcript** — long-form, latency-tolerant.

### Technique 1: terminology pre-extraction

**Applies to**: Lens overlay (multi-line images), audio transcripts.
**Skip for**: short single-line lookups.

The concrete pain: line-by-line translation today produces
**inconsistent terminology** across a single image. If the source
mentions "The Speech Accent Archive" three times, the embedded LLM
may render it three different ways without a shared glossary.

**Proposed shape** (Phase 29):

- Gate: trigger when OCR produces ≥ 5 detected lines on a single
  request.
- Stage 1 — glossary pass:
  Build a single prompt out of all line texts, ask the model for
  proper nouns / technical terms with target-language renderings.
  Parse the response into a `Vec<(String, String)>`.
- Stage 2 — per-line translate:
  Inject the glossary as a system-prompt fragment ("Use these
  exact renderings: …") and translate each line as today.

Cost: one extra forward pass per request (200–500 ms on GPU). For
Lens overlay this is acceptable because we are saving the user the
pain of fixing inconsistent translations.

### Technique 2: review / critique pass

**Applies to**: Quality tier as an opt-in mode.
**Skip for**: Compact / Standard tiers, Lens overlay default.

A second LLM pass roughly doubles latency. Worth it when the user
explicitly chose Quality tier *and* the input is long enough that
the quality lift dominates the latency cost (audio transcripts).

**Proposed shape** (Phase 29, opt-in):

- Settings flag `providers.embedded_llm.enable_review_pass`
  (default `false`).
- When on, after the translate pass, run a review pass with the
  *same* model but a critique-style prompt. The article suggests
  using an asymmetric model — we do not currently ship two
  instruction-tuned models with different strengths, so this
  refinement waits for Phase 30.

### Technique 3: asymmetric model usage

**Applies to**: only if we add a second model family.

Our current ladder (Compact JA / Standard JA-bridge / Standard
multi / Quality multi) is all instruction-tuned LMs from one of two
families (llm-jp, Gemma). The article's asymmetry argument suggests
that adding e.g. Qwen as a Reviewer-only model could lift Quality
tier without changing the translate path. Out of scope for Phase
29; revisit when adding new model tiers.

### Technique 4: summary-compressed KV cache

**Skip for MoonLens.**

The article's technique compresses the running transcript context
to fit inside a fixed KV-cache budget. We do per-line translation
with no running context — there is nothing to compress. If we
later add a context-aware mode (e.g. translate paragraphs as a
unit), this technique returns to the discussion.

### Technique 5: CoT-less direct output

**Already doing this.**

Our greedy-decode prompts ask for the translation only; no
"chain-of-thought" emission. The article's measurement that direct
output is both faster *and* higher quality matches our own
observation from Phase 25 (few-shot prompts beat reasoning prompts
on Lens-style short inputs).

## Decision summary

| Technique | Land in? | Phase | Status |
|---|---|---|---|
| Terminology pre-extraction | yes, multi-line gate | **29** | **shipped** (2026-05-15) |
| Review pass | yes, Quality-tier opt-in | 30+ | deferred — Phase 29 scope creep, latency 2× makes it Quality-only |
| Asymmetric reviewer model | maybe, after new model lands | 30+ | depends on model choice |
| Summary-compressed KV cache | no | — | n/a in current architecture |
| CoT-less prompts | already doing | — | — |

## Phase 29 implementation note (2026-05-15)

The pre-extraction pass landed in commit `<tbd>`. Touch list:

- `src-tauri/src/providers/glossary.rs` (new) — prompt builder,
  response parser, 13 unit tests covering decoration stripping,
  duplicate handling, anti-hallucination guard.
- `src-tauri/src/providers/mod.rs` — `TaskInput::Translate` gained
  a `glossary: Vec<(String, String)>` field (serde-default empty for
  backward-compat). `Provider` trait gained `extract_glossary`
  with a `None` default; only `EmbeddedLlmTranslator` overrides.
- `src-tauri/src/providers/translate_embedded_llm.rs` —
  `system_prompt_with_glossary` helper splices the agreed-on
  entries into the system prompt at inference time. Greedy decode
  with `temperature=0` for the extraction pass itself.
- `src-tauri/src/pipeline.rs` — `Pipeline::extract_glossary` walks
  the translate chain and returns the first non-None reply.
- `src-tauri/src/lens_pipeline.rs` — `translate_cards` now calls
  `extract_glossary` before kicking off the per-line spawn loop,
  gated on `cards.len() >= glossary_min_lines`. Each per-line
  spawn forwards the glossary via the new
  `LensTranslator::translate_with_glossary` method.
- Settings: `providers.glossary_min_lines: u32` (default 5),
  surfaced in `default.toml`. Set to `u32::MAX` to disable.

A formal end-to-end CER / consistency bench was deferred — running
it requires the embedded LLM at a specific tier with a stable
golden set, and the existing Phase 27 bench harness already covers
the per-tier baselines we care about. The expected user-visible
delta is "same proper noun rendered the same way every time" on
multi-line Lens captures.

## Open questions

- Does the glossary pass need to be per-target-language, or can it
  be language-agnostic ("here are proper nouns in the source —
  render them appropriately")? The article uses target-aware
  glossaries. We probably should too, but the cost is one extra
  pass per target.
- For Lens watcher, glossary stability across frames matters
  more than per-frame accuracy. A cached glossary keyed by
  capture region would let us amortise the cost — open for design.
- Review-pass prompt design: does the model need to see the source
  + initial translation + tier-specific style guide? Worth running
  a small bench before shipping.

## See also

- `docs/TRANSLATION_ROUTING_MATRIX.md` — current translation
  tier / routing design.
- `docs/benchmarks/2026-05-12-phase27-*` — most recent translation
  benches; relevant baselines for any Phase 29 quality measurement.
