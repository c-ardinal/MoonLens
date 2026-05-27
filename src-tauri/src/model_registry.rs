//! Curated registry of embedded translation models.
//!
//! The registry is the single source of truth for "which models can a
//! user pick from the settings UI." A flat const list (no runtime
//! plug-in mechanism) is intentional: every entry here ships with the
//! app, the licence has been audited, and the HuggingFace repo has
//! been verified to work with `mistral.rs::GgufModelBuilder`. New
//! entries get a code review where the licence + runtime + size are
//! sanity-checked, instead of trusting a runtime-fetched manifest.
//!
//! Three tiers cover the realistic hardware spread for the offline /
//! restricted-network deployments this tool targets:
//!
//! * **Compact** — old office laptops / corporate VMs with 4 GB RAM.
//!   Quality is noticeably below the larger tiers but the tool still
//!   produces usable Japanese output, and the GGUF fits on tight
//!   disks (< 500 MB).
//! * **Standard** — modern office laptops with 8 GB RAM. The default
//!   tier the tool ships with — the install-and-go target.
//! * **Quality** — workstations with 16 GB+ RAM. Translation quality
//!   competitive with cloud APIs at the cost of disk + memory.
//!
//! All shipped tiers are **Apache 2.0** so corporate redistribution
//! does not need a license-compatibility review.

use serde::{Deserialize, Serialize};

/// Hardware bucket the tier targets. Used by the onboarding flow to
/// recommend a default based on the host's installed RAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTierClass {
    /// 4 GB RAM hosts. Smallest GGUF; lower translation quality but
    /// still usable for short Lens lines.
    Compact,
    /// 8 GB RAM hosts. Default — balances quality and speed.
    Standard,
    /// 16 GB+ RAM hosts. Larger model, higher quality, slower
    /// first-token latency.
    Quality,
}

/// Chat template family the model expects. Different model families
/// use different prompt markers — using the wrong template degrades
/// quality silently (the model still produces text, but it ignores
/// the system prompt and generates lower-quality translations).
///
/// Concrete formats currently supported:
/// - **ChatML**: `<|im_start|>role\ncontent<|im_end|>` markers,
///   used by Qwen 2.5 / many open-weight models.
/// - **ChatML (Qwen 3 no-think)**: same markers as ChatML but the
///   assistant primer pre-emits empty `<think>...</think>` tags so
///   Qwen 3's deliberative-reasoning mode short-circuits and the
///   model proceeds straight to the answer. Without this, Qwen 3
///   wastes the entire 64-token bench budget thinking out loud and
///   never reaches the translation. See HF Qwen 3 release notes for
///   the `enable_thinking=False` flag (this is the manual-render
///   equivalent).
/// - **Gemma**: `<start_of_turn>role\ncontent<end_of_turn>`, used by
///   Gemma 2 / Gemma 3 / Gemma 4 (incl. nano E2B/E4B). No system role
///   in the on-the-wire format — system prompt is inlined into the
///   first user turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatTemplate {
    /// `<|im_start|>` / `<|im_end|>` (Qwen 2.5 / open-weight models).
    ChatMl,
    /// ChatML with Qwen 3's reasoning mode disabled (empty think tags).
    ChatMlQwen3NoThink,
    /// `<start_of_turn>` / `<end_of_turn>` (Gemma family).
    Gemma,
}

/// Canonical descriptor for a single picker entry. Matches the JSON
/// shape `list_model_tiers` returns to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct ModelTier {
    /// Stable identifier the UI passes back to `set_model_tier`. Picked
    /// to be readable in `user.toml` so power users can edit by hand.
    pub id: &'static str,
    /// Human-readable name the picker displays (Japanese — this is a
    /// JP-first product).
    pub display_name: &'static str,
    /// Short blurb shown beneath the name.
    pub description: &'static str,
    /// HuggingFace repo. Must be verified to host the file(s) under
    /// `resolve/main/<filename>` without authentication; the download
    /// IPC fetches there.
    pub repo: &'static str,
    /// Exact GGUF filename(s) in the repo. Almost every tier has a
    /// single file, but HuggingFace splits large GGUFs across LFS
    /// objects (e.g. Qwen 2.5 7B Q4_K_M ships as
    /// `*-00001-of-00002.gguf` + `*-00002-of-00002.gguf`).
    /// `llama-cpp-2` natively loads the primary file and follows the
    /// part chain via mmap; the downloader iterates over the slice
    /// and fetches each part.
    /// Callers that need a single "primary" reference (UI labels,
    /// path resolution for status display, package manifest's
    /// legacy single-file field) use `primary_filename()`.
    pub filenames: &'static [&'static str],
    /// Approximate on-disk size in MB across **all** parts. Used
    /// by the picker to render "~1.1 GB" without having to query HF.
    pub size_mb: u32,
    /// Recommended minimum installed RAM in GB. The HW detector uses
    /// this to bucket the host into a tier class.
    pub min_ram_gb: u32,
    /// SPDX-style license advert. Settings UI surfaces this so IT
    /// audits do not leave the app.
    pub license: &'static str,
    /// Canonical license URL.
    pub license_url: &'static str,
    /// Tier class for HW recommendation.
    pub class: ModelTierClass,
    /// Chat template family the model expects. Determines which
    /// prompt markers `EmbeddedLlmTranslator` uses when assembling
    /// the (system + user) → (rendered prompt) mapping. Defaults to
    /// `ChatMl` for Qwen / SmolLM family models.
    #[serde(default = "default_chat_template")]
    pub chat_template: ChatTemplate,
    /// Optional speculative-decoding drafter. When present, the
    /// embedded translator loads BOTH this tier's main GGUF AND the
    /// drafter alongside it, and runs every translation through a
    /// spec-decode loop: the drafter predicts N tokens
    /// autoregressively, the main model batch-verifies them in
    /// parallel, and matching prefixes are accepted in a single
    /// forward pass. Google's Gemma 4 family ships official
    /// drafters (`*-it-assistant`) under Apache 2.0; community
    /// quantisations are on HuggingFace.
    ///
    /// Vocabulary compatibility is mandatory — drafter and main
    /// model MUST share the tokeniser, otherwise spec decode
    /// produces garbage. The runtime trusts this invariant (no
    /// per-token vocab check) so the registry is the right place
    /// to encode it.
    #[serde(default)]
    pub drafter: Option<DrafterSpec>,
}

/// Optional speculative-decoding drafter paired with a main tier.
/// The drafter is a smaller version of the main model that shares
/// its tokeniser; spec decode uses the drafter to predict candidate
/// tokens and the main model to verify them, accepting a run of
/// matching tokens in a single batch decode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DrafterSpec {
    /// HuggingFace repo hosting the drafter GGUF.
    pub repo: &'static str,
    /// Drafter GGUF filename in `repo`.
    pub filename: &'static str,
    /// Approximate on-disk size in MB. Used by the picker / status
    /// UI to show "+ 95 MB drafter" alongside the main model size.
    pub size_mb: u32,
    /// Default n_draft (tokens predicted per spec round). Typical
    /// values are 4-8 — too small means no speedup, too large means
    /// wasted draft work when acceptance rate is low. Per-model
    /// because some pairings tolerate longer drafts than others.
    pub n_draft: u32,
}

// Referenced by `#[serde(default = "default_chat_template")]` on
// `ModelTier::chat_template` via a string lookup, which the
// compiler's dead-code analyser cannot see. The `#[allow]` opts out
// of the resulting false positive; removing this function silently
// breaks deserialisation of partial tier records (older snapshots
// or user-edited TOMLs that omit `chat_template`).
#[allow(dead_code)]
fn default_chat_template() -> ChatTemplate {
    ChatTemplate::ChatMl
}

impl ModelTier {
    /// First filename in `filenames`. The IPC layer uses this for
    /// "model_id" labels and for legacy single-file path resolution
    /// so existing `user.toml` files (which only carried
    /// `filename_remote = "..."`) keep working unchanged.
    pub fn primary_filename(&self) -> &'static str {
        self.filenames.first().copied().unwrap_or("")
    }

    /// Display a stable "<repo>:<primary>" identifier the UI shows
    /// next to the license (and that the export package records as
    /// `model_id`'s source coordinate). Multi-part tiers identify
    /// themselves by their first file because that is the
    /// stable "primary" name (subsequent parts share its prefix).
    pub fn coord(&self) -> String {
        format!("{}:{}", self.repo, self.primary_filename())
    }
}

/// All shipped tiers, ordered Compact → Standard → Quality. UI
/// consumers (settings dropdown, onboarding modal) iterate this slice
/// directly so tier ordering is determined here, not at every call
/// site.
pub const TIERS: &[ModelTier] = &[
    // Ordered smallest → largest by Q4_K_M GGUF size_mb. The UI
    // dropdown iterates this slice directly and the regression test
    // `tiers_are_ordered_smallest_to_largest` enforces the
    // monotonicity. Phase 21 dropped the SmolLM 2 1.7B candidate
    // after benchmarking confirmed it produces gibberish on en→ja
    // (English-only training data); see docs/EMBEDDED_LLM_PERF.md
    // for the full rejection record.
    //
    // Phase 26 evaluation: llm-jp 3 small variants (150M / 440M)
    // were locally-converted (HF safetensors → GGUF via synthesised
    // tokenizer.model + llama.cpp's convert_hf_to_gguf.py) and
    // bench-tested but REJECTED at every size below 980M:
    //   - 150M base/Instruct2: instruction-following completely
    //     absent. Base copies the last few-shot example verbatim
    //     for every input; Instruct2 emits repeat-loops, English
    //     pass-throughs, or unrelated instruction snippets.
    //   - 440M base/Instruct2: marginally better but still 5/5
    //     translation breaks (repeat loops, English pass-through,
    //     meta-commentary instead of translation).
    //   - 980M base: 1/5 English pass-through case; Instruct
    //     variants required for reliable translation.
    //   - 980M Instruct2 (locally-converted): equivalent quality
    //     to mmnga's Instruct3 GGUF, but the locally-converted
    //     path adds tokenizer.model synthesis risk; opted to keep
    //     only the mmnga Instruct3 mirror.
    // See docs/benchmarks/2026-05-12-phase26-llmjp3-small-
    // quality-review.md for the full evidence.
    ModelTier {
        id: "qwen3-0.6b-q4_k_m",
        display_name: "Compact+ (Qwen 3 0.6B)",
        description: "Qwen 2.5 0.5B 後継。多言語品質向上、特に日本語で上回る (Apache 2.0)。",
        // The original Qwen org does not ship GGUF quants for the
        // small Qwen 3 sizes; unsloth's mirror is the canonical
        // community quantisation for these.
        repo: "unsloth/Qwen3-0.6B-GGUF",
        filenames: &["Qwen3-0.6B-Q4_K_M.gguf"],
        size_mb: 397,
        min_ram_gb: 4,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/blob/main/LICENSE",
        class: ModelTierClass::Compact,
        // Qwen 3 ships with deliberative-reasoning mode on by default,
        // emitting `<think>...</think>` blocks before answering. For
        // short-output translation that wastes the entire token budget
        // on internal monologue. The no-think template primes the
        // assistant turn with empty `<think></think>` to bypass it.
        chat_template: ChatTemplate::ChatMlQwen3NoThink,
        drafter: None,
    },
    ModelTier {
        id: "qwen2.5-0.5b-instruct-q4_k_m",
        display_name: "Compact (Qwen 2.5 0.5B)",
        description: "低スペックPC向け。最小サイズ・最速起動、翻訳品質は控えめ。",
        repo: "Qwen/Qwen2.5-0.5B-Instruct-GGUF",
        filenames: &["qwen2.5-0.5b-instruct-q4_k_m.gguf"],
        size_mb: 400,
        min_ram_gb: 4,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/blob/main/LICENSE",
        class: ModelTierClass::Compact,
        chat_template: ChatTemplate::ChatMl,
        drafter: None,
    },
    // Qwen 3.5 0.8B was rejected in Phase 23 v23c (Chinese leakage:
    // "欢迎来到未来翻译的殿堂"). Re-added in Phase 25 to validate
    // whether the new few-shot prompt + strengthened "no other
    // scripts" system directive (see `build_translation_few_shots`
    // and the system_prompt default in translate_embedded_llm.rs)
    // can rescue this candidate. Keep or drop based on the v25
    // bench evidence.
    ModelTier {
        id: "qwen3.5-0.8b-q4_k_m",
        display_name: "Compact++ (Qwen 3.5 0.8B)",
        description: "Qwen 3.5 系最小。Compact と Standard の間、Phase 25 few-shot で言語ロック検証中 (Apache 2.0)。",
        repo: "unsloth/Qwen3.5-0.8B-GGUF",
        filenames: &["Qwen3.5-0.8B-Q4_K_M.gguf"],
        size_mb: 508,
        min_ram_gb: 4,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/blob/main/LICENSE",
        class: ModelTierClass::Compact,
        // See registry comment on qwen3.5-4b: model card claims
        // thinking is default OFF but the GGUF still emits
        // `<think>` blocks. Same no-think primer applies.
        chat_template: ChatTemplate::ChatMlQwen3NoThink,
        drafter: None,
    },
    // llm-jp 3 980m instruct3 — sub-1B JA-specialised tier
    // (Phase 26 v1). mmnga's Q4_K_M is the canonical GGUF mirror;
    // upstream llm-jp ships only safetensors for this size. base
    // and instruct2 variants were locally-converted and bench-
    // tested in Phase 26 v2 but rejected (see registry comment
    // above for evidence).
    ModelTier {
        id: "llm-jp-3-980m-instruct3-q4_k_m",
        display_name: "JA-Compact (llm-jp 3 980M)",
        description: "llm-jp 3 系の 980M 日本語特化モデル (Apache 2.0)。Compact 帯に JA-first 選択肢を追加。",
        repo: "mmnga/llm-jp-3-980m-instruct3-gguf",
        filenames: &["llm-jp-3-980m-instruct3-Q4_K_M.gguf"],
        size_mb: 597,
        min_ram_gb: 4,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/llm-jp/llm-jp-3-980m-instruct3/blob/main/LICENSE",
        class: ModelTierClass::Compact,
        chat_template: ChatTemplate::ChatMl,
        drafter: None,
    },
    // RakutenAI 2.0 mini-instruct (1.53B Apache 2.0) was evaluated
    // in Phase 23 v1 bench but produced broken translations (echoes
    // system prompt, mixes EN/JA fragments, emits raw <|im_end|>
    // before content). Chat template incompatibility suspected;
    // removed pending JA-quality investigation.
    //
    // EuroLLM 1.7B-Instruct (utter-project, Apache 2.0) was also
    // evaluated and rejected: outputs random European languages
    // (Tamil/Dutch/Norwegian) instead of Japanese, despite the
    // 35-language model card claim. The translation directive in
    // our prompt is apparently outside EuroLLM's training
    // distribution.
    ModelTier {
        id: "qwen3-1.7b-q4_k_m",
        display_name: "Standard+ (Qwen 3 1.7B)",
        description: "Qwen 2.5 1.5B 後継。標準ティアの正統進化版 (Apache 2.0)。",
        repo: "unsloth/Qwen3-1.7B-GGUF",
        filenames: &["Qwen3-1.7B-Q4_K_M.gguf"],
        size_mb: 1056,
        min_ram_gb: 8,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/blob/main/LICENSE",
        class: ModelTierClass::Standard,
        chat_template: ChatTemplate::ChatMlQwen3NoThink,
        drafter: None,
    },
    ModelTier {
        id: "qwen2.5-1.5b-instruct-q4_k_m",
        display_name: "Standard (Qwen 2.5 1.5B)",
        description: "標準。一般的なオフィスPCでバランスよく動作する既定ティア。",
        repo: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
        filenames: &["qwen2.5-1.5b-instruct-q4_k_m.gguf"],
        size_mb: 1100,
        min_ram_gb: 8,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/blob/main/LICENSE",
        class: ModelTierClass::Standard,
        chat_template: ChatTemplate::ChatMl,
        drafter: None,
    },
    ModelTier {
        id: "llm-jp-3.1-1.8b-instruct4-q4_k_m",
        display_name: "JA-Standard+ (llm-jp 3.1 1.8B)",
        description: "日本政府系 LLM-JP プロジェクトの 1.8B JA-EN モデル (Apache 2.0)。日本語学習データ重視。",
        repo: "mmnga/llm-jp-3.1-1.8b-instruct4-gguf",
        filenames: &["llm-jp-3.1-1.8b-instruct4-Q4_K_M.gguf"],
        size_mb: 1110,
        min_ram_gb: 8,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/llm-jp/llm-jp-3.1-1.8b/blob/main/LICENSE",
        class: ModelTierClass::Standard,
        chat_template: ChatTemplate::ChatMl,
        drafter: None,
    },
    // Tencent Hunyuan Hy-MT2 1.8B — a translation-specialist model
    // released 2026-05-21. The 2026-05-26 standalone bench
    // (`bench_specialist_translation`) measured ws=0.93 / id=0.98
    // against llm-jp-3.1 1.8B's ws=0.88 / id=0.89 across 12
    // translation fixtures, a +5 / +9 win on both metrics. The
    // routed eval (`translate_quality_eval`) confirmed Standard
    // tier's residual `log-rust-tracing-multi` ws violation
    // (Standard 0.31 < Compact 0.49) is specifically caused by
    // llm-jp-3.1's tendency to collapse multi-line log input into
    // a single output line — a regression Hy-MT2 does not share.
    //
    // Chat template: ChatML. The GGUF emits `<|im_end|>` byte-
    // level (special-token flag dropped at conversion); the
    // runtime's `strip_chat_template_garbage` catches the
    // resulting malformed variants (`<|im_end(){`, `<|im_end>>`).
    ModelTier {
        id: "hy-mt2-1.8b-q4_k_m",
        display_name: "JA-Standard+ (Hy-MT2 1.8B)",
        description:
            "Tencent Hy-MT2 1.8B 翻訳特化モデル (2026-05-21)。33 言語の翻訳に対応。Standard / Quality JA-bridge として llm-jp-3.1 を置き換え (Tencent License)。",
        repo: "tencent/Hy-MT2-1.8B-GGUF",
        filenames: &["Hy-MT2-1.8B-Q4_K_M.gguf"],
        // Actual on-disk size: 1133 MB (1133080448 bytes ÷ 1024²).
        size_mb: 1133,
        min_ram_gb: 8,
        license: "Tencent License",
        license_url:
            "https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF/blob/main/LICENSE.txt",
        class: ModelTierClass::Standard,
        chat_template: ChatTemplate::ChatMl,
        drafter: None,
    },
    // Qwen 3.5 2B was rejected in Phase 23 v23c (semantic breaks:
    // "急げんこつなうとんぼが lazy なぬまを越す"). Re-added in
    // Phase 25 on the same basis as the 0.8B re-introduction —
    // we want to confirm whether few-shot anchoring fixes the
    // 狐→とんぼ kind of substitution noise.
    ModelTier {
        id: "qwen3.5-2b-q4_k_m",
        display_name: "Standard++ (Qwen 3.5 2B)",
        description: "Qwen 3.5 2B。Standard と Quality- の間、Phase 25 few-shot で品質再評価中 (Apache 2.0)。",
        repo: "unsloth/Qwen3.5-2B-GGUF",
        filenames: &["Qwen3.5-2B-Q4_K_M.gguf"],
        size_mb: 1221,
        min_ram_gb: 8,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/blob/main/LICENSE",
        class: ModelTierClass::Standard,
        chat_template: ChatTemplate::ChatMlQwen3NoThink,
        drafter: None,
    },
    ModelTier {
        id: "qwen3-4b-instruct-2507-q4_k_m",
        display_name: "Quality- (Qwen 3 4B Instruct 2507)",
        description: "中量級 (Standard と Quality の中間)。8-16GB RAM PC で実用的な品質と速度のバランス (Apache 2.0)。",
        repo: "unsloth/Qwen3-4B-Instruct-2507-GGUF",
        filenames: &["Qwen3-4B-Instruct-2507-Q4_K_M.gguf"],
        size_mb: 2497,
        min_ram_gb: 12,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/blob/main/LICENSE",
        class: ModelTierClass::Standard,
        // Phase 21 v2 finding: the "Instruct-2507" variant has its
        // reasoning mode removed at the checkpoint level — the
        // empty-think primer that works for Qwen 3 0.6B / 1.7B
        // makes 2507 emit EOS immediately (5 samples → 3 produced
        // 0 tokens, 2 echoed the source verbatim). Plain ChatML
        // (no primer) is the right template here.
        chat_template: ChatTemplate::ChatMl,
        drafter: None,
    },
    ModelTier {
        id: "qwen3.5-4b-q4_k_m",
        display_name: "Quality-- (Qwen 3.5 4B)",
        description: "Qwen 3.5 4B 中量級。Qwen 3 4B 2507 後継。201言語、thinking デフォルトOFF (Apache 2.0)。",
        repo: "unsloth/Qwen3.5-4B-GGUF",
        filenames: &["Qwen3.5-4B-Q4_K_M.gguf"],
        size_mb: 2614,
        min_ram_gb: 12,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/blob/main/LICENSE",
        class: ModelTierClass::Standard,
        // See Qwen 3.5 0.8B note: same no-think primer needed.
        chat_template: ChatTemplate::ChatMlQwen3NoThink,
        drafter: None,
    },
    // Gemma 4 nano (E2B / E4B) — Google's edge-optimised family with
    // sparse activation (effective param count smaller than total).
    // **License caveat**: Gemma Terms of Use, NOT Apache 2.0. Commercial
    // use is permitted but redistribution requires shipping the license
    // terms + Prohibited Use Policy. Selecting this tier surfaces a
    // license-acknowledgement step in the settings UI before download.
    // Google does not ship GGUF; unsloth's mirror is the canonical
    // community quantisation for these.
    ModelTier {
        id: "gemma-4-e2b-it-q4_k_m",
        display_name: "Edge (Gemma 4 E2B)",
        description: "Google 製エッジモデル。effective 2B 活性化 (total ~5B)、Gemma TOS — 配布時にライセンス同梱必須。",
        repo: "unsloth/gemma-4-E2B-it-GGUF",
        filenames: &["gemma-4-E2B-it-Q4_K_M.gguf"],
        size_mb: 3107,
        min_ram_gb: 8,
        license: "Gemma-Terms-of-Use",
        license_url: "https://ai.google.dev/gemma/terms",
        class: ModelTierClass::Standard,
        chat_template: ChatTemplate::Gemma,
        // Official Google MTP drafter (Apache 2.0). The Google org
        // ships `google/gemma-4-E2B-it-assistant` as raw safetensors
        // (78M params, ~158 MB); community Q8_0 quant is the
        // canonical GGUF for embedded use.
        drafter: Some(DrafterSpec {
            repo: "Radamanthys11/Gemma-4-E2B-it-assistant-GGUF",
            filename: "gemma-4-E2B-it-assistant-Q8_0.gguf",
            size_mb: 95,
            n_draft: 5,
        }),
    },
    ModelTier {
        id: "qwen2.5-7b-instruct-q4_k_m",
        display_name: "Quality (Qwen 2.5 7B)",
        description:
            "高品質ティア。16GB+ RAM の高性能PC向け、翻訳品質はクラウドAPIに迫る。",
        repo: "Qwen/Qwen2.5-7B-Instruct-GGUF",
        // Qwen 2.5 7B Q4_K_M is split across two LFS files in the
        // upstream repo. Listing both parts here is what fixes the
        // 9.x-era 404: the pre-9.x registry recorded a single
        // synthetic filename `qwen2.5-7b-instruct-q4_k_m.gguf`
        // that does not exist on HuggingFace. llama-cpp-2 follows
        // the part chain via mmap, so loading transparently joins
        // them at runtime.
        filenames: &[
            "qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf",
            "qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf",
        ],
        size_mb: 4700,
        min_ram_gb: 16,
        license: "Apache-2.0",
        license_url:
            "https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF/blob/main/LICENSE",
        class: ModelTierClass::Quality,
        chat_template: ChatTemplate::ChatMl,
        drafter: None,
    },
    ModelTier {
        id: "gemma-4-e4b-it-q4_k_m",
        display_name: "Edge+ (Gemma 4 E4B)",
        description: "Gemma 4 nano E4B 版。effective 4B 活性化 (total ~8B)。Gemma TOS。",
        repo: "unsloth/gemma-4-E4B-it-GGUF",
        filenames: &["gemma-4-E4B-it-Q4_K_M.gguf"],
        size_mb: 4977,
        min_ram_gb: 12,
        license: "Gemma-Terms-of-Use",
        license_url: "https://ai.google.dev/gemma/terms",
        class: ModelTierClass::Quality,
        chat_template: ChatTemplate::Gemma,
        drafter: Some(DrafterSpec {
            repo: "AtomicChat/gemma-4-E4B-it-assistant-GGUF",
            filename: "gemma-4-E4B-it-assistant.Q8_0.gguf",
            size_mb: 96,
            n_draft: 5,
        }),
    },
];

/// The tier the tool defaults to on a fresh install. Reads as
/// "qwen2.5-1.5b-instruct-q4_k_m" — must stay in sync with
/// `DEFAULT_MODEL_FILENAME_REMOTE` in `translate_embedded_llm.rs`
/// (the constants there are the legacy single-model surface; this
/// registry is its multi-tier replacement).
pub const DEFAULT_TIER_ID: &str = "qwen2.5-1.5b-instruct-q4_k_m";

/* ---------------- Translation router anchor tier IDs --------------- */
//
// The translation router (`translation_router.rs`) consults the
// matrix in `docs/TRANSLATION_ROUTING_MATRIX.md` and resolves each
// `(UserTier, Hardware, src, tgt)` triple to one or two of the IDs
// below. Lifting the literals out of the router into named
// constants here gives two benefits:
//
// 1. Type-safe single source of truth — every router decision goes
//    through these constants, so renaming a tier id in TIERS only
//    requires editing the constant, and the
//    `router_anchor_ids_are_registered` regression test below
//    catches any disconnect at compile-time (well, test-run-time).
// 2. Grep-ability — a future maintainer searching for "which paths
//    use the JA bridge?" finds every caller of
//    `STANDARD_JA_BRIDGE_TIER_ID` instantly, without parsing the
//    routing match arms.

/// Compact tier's sole model. JA-Compact (llm-jp 3 980M Instruct3).
/// Used for `en↔ja` plus the explicitly-degraded multilingual
/// routes when the user opted into the smallest footprint.
pub const COMPACT_JA_TIER_ID: &str = "llm-jp-3-980m-instruct3-q4_k_m";

/// Standard / Quality tiers' Japanese bridge. JA-Standard+
/// (Hy-MT2 1.8B, Tencent 2026-05-21). 2026-05-26 bench: ws=0.93,
/// id=0.98 across 12 fixtures — +5 / +9 over the previous bridge
/// (llm-jp 3.1 1.8B, ws=0.88 id=0.89). Replaces llm-jp-3.1 to
/// resolve the residual `log-rust-tracing-multi` strict-ordering
/// violation that llm-jp's tendency to collapse multi-line input
/// caused under Standard tier.
pub const STANDARD_JA_BRIDGE_TIER_ID: &str = "hy-mt2-1.8b-q4_k_m";

/// Standard tier's multilingual model. Edge (Gemma 4 E2B).
/// Direct single-step on CPU (2 GB-class effective) and pivot
/// partner on GPU.
pub const STANDARD_MULTI_TIER_ID: &str = "gemma-4-e2b-it-q4_k_m";

/// Quality tier's multilingual model. Edge+ (Gemma 4 E4B).
/// Highest measured multilingual safety (96% across 28 pairs in
/// Phase 27); direct on CPU, pivot partner on GPU.
pub const QUALITY_MULTI_TIER_ID: &str = "gemma-4-e4b-it-q4_k_m";

/// Look up a tier by its stable id. Returns `None` for unknown ids
/// — callers that need a fallback should pair this with [`default_tier`].
pub fn tier_by_id(id: &str) -> Option<&'static ModelTier> {
    TIERS.iter().find(|t| t.id == id)
}

/// Resolve the tier matching `repo` + `filename` (the coordinates the
/// download IPC stores in `[providers.configs."embedded.llm"]`). This
/// is how the UI maps user.toml back to a tier id without having to
/// stash the tier id separately. For multi-part tiers the lookup
/// matches against ANY of the part filenames so a user.toml that
/// records the second part still resolves correctly.
pub fn tier_by_coord(repo: &str, filename: &str) -> Option<&'static ModelTier> {
    TIERS
        .iter()
        .find(|t| t.repo == repo && t.filenames.contains(&filename))
}

/// Resolve the tier matching `repo` only — used by the legacy-config
/// migration to fix user.toml entries whose `filename_remote` points
/// at a synthetic name we shipped pre-9.x (e.g.
/// `qwen2.5-7b-instruct-q4_k_m.gguf`, which never existed on HF
/// because the actual upstream artefact is split into two parts).
/// Returns the FIRST registered tier with the matching repo so the
/// migration is deterministic.
pub fn tier_by_repo(repo: &str) -> Option<&'static ModelTier> {
    TIERS.iter().find(|t| t.repo == repo)
}

/// One-shot legacy-config migration. Applied at app startup before
/// any download IPC runs. Detects the pre-9.x embedded config that
/// recorded a non-existent synthetic filename for the multi-part
/// 7B Q4_K_M tier and rewrites the entry to the registered shape
/// (primary + `filenames_remote_extra`). Idempotent: returns
/// `false` when the config already matches a known tier or the
/// repo is unrecognised.
pub fn migrate_legacy_embedded_config(cfg: &mut serde_json::Value) -> bool {
    let Some(obj) = cfg.as_object_mut() else {
        return false;
    };
    let Some(repo) = obj.get("repo").and_then(|v| v.as_str()).map(str::to_string) else {
        return false;
    };
    let Some(filename_remote) = obj
        .get("filename_remote")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    else {
        return false;
    };

    // Already lines up with a registered tier — nothing to do.
    if tier_by_coord(&repo, &filename_remote).is_some() {
        return false;
    }

    // Repo recognised but filename does not match any part. Upgrade
    // to the registered tier's primary filename + extras list.
    let Some(tier) = tier_by_repo(&repo) else {
        return false;
    };

    obj.insert(
        "filename_remote".into(),
        serde_json::Value::String(tier.primary_filename().into()),
    );
    if tier.filenames.len() > 1 {
        let extras: Vec<serde_json::Value> = tier
            .filenames
            .iter()
            .skip(1)
            .map(|f| serde_json::Value::String((*f).to_string()))
            .collect();
        obj.insert(
            "filenames_remote_extra".into(),
            serde_json::Value::Array(extras),
        );
    } else {
        obj.remove("filenames_remote_extra");
    }
    // Drop any stale `model_path` override so the resolver picks
    // the new filename's natural location under <app-data>/models/.
    obj.remove("model_path");
    true
}

/// Legacy provider id we shipped before the 12.x rename. Stays as a
/// constant so the migration's intent is explicit at every call
/// site and grep-able when (if) we add a third id rename later.
pub const LEGACY_EMBEDDED_PROVIDER_ID: &str = "embedded.gemma";
/// Current provider id. The default chain points here; the
/// migration below rewrites legacy entries to match.
pub const CURRENT_EMBEDDED_PROVIDER_ID: &str = "embedded.llm";

/// Whole-`Settings` migration that combines:
///
/// 1. The 12.x `embedded.gemma` → `embedded.llm` rename. The
///    embedded provider was originally named after the Gemma model
///    family it loaded; the 2026-05-09 switch to Qwen 2.5 made the
///    name actively misleading, so the id was generalised to
///    `embedded.llm` (model-family-neutral).
/// 2. The 9.x synthetic-filename rewrite implemented in
///    [`migrate_legacy_embedded_config`]. The two are bundled because
///    a single user.toml typically needs both — a user who had
///    `embedded.gemma` in their chain pre-9.x almost certainly also
///    had the 7B synthetic filename if they picked Quality.
///
/// Returns `true` when the settings were modified. The caller is
/// expected to persist the snapshot via `SettingsStore::save` if
/// `true` is returned.
pub fn migrate_legacy_embedded_settings(s: &mut crate::settings::schema::Settings) -> bool {
    let mut changed = false;

    // Rename chain entries across every task chain. The translate
    // chain is the obvious one but the pre-12.x default pre-loaded
    // `embedded.gemma` into explain / summarize too via the Ollama
    // preset, so we sweep all four LLM-shaped chains.
    let chains: [&mut Vec<String>; 5] = [
        &mut s.providers.translate.chain,
        &mut s.providers.explain.chain,
        &mut s.providers.summarize.chain,
        &mut s.providers.ocr.chain,
        &mut s.providers.asr.chain,
    ];
    for chain in chains {
        for entry in chain.iter_mut() {
            if entry == LEGACY_EMBEDDED_PROVIDER_ID {
                *entry = CURRENT_EMBEDDED_PROVIDER_ID.to_string();
                changed = true;
            }
        }
    }

    // Move the per-provider config block. Done as a remove-then-
    // insert to avoid a clone when the entry is large.
    if let Some(legacy) = s.providers.configs.remove(LEGACY_EMBEDDED_PROVIDER_ID) {
        // CRITICAL: the merge layer (`layer_user_over_defaults`)
        // produces a Settings whose `configs` map contains BOTH
        // `embedded.gemma` (from user.toml, with the user's last
        // tier choice) AND `embedded.llm` (from default.toml,
        // bundled defaults). The legacy entry is the user's
        // authoritative state; the new-key entry is just whatever
        // the bundled defaults shipped with. Use `insert` (not
        // `or_insert`) so the legacy data overrides — the previous
        // `or_insert` was silently rolling user's tier selection
        // back to the bundled Standard tier on every restart that
        // did a rename migration.
        s.providers
            .configs
            .insert(CURRENT_EMBEDDED_PROVIDER_ID.to_string(), legacy);
        changed = true;
    }

    // Now apply the synthetic-filename migration on the (possibly
    // just-renamed) embedded config. Done last so the function
    // operates on the canonical key.
    if let Some(cfg) = s.providers.configs.get_mut(CURRENT_EMBEDDED_PROVIDER_ID) {
        if migrate_legacy_embedded_config(cfg) {
            changed = true;
        }
    }

    // Derive the user-facing translation tier (Compact / Standard /
    // Quality) from the legacy single-model coordinates if the
    // user.toml predates the routing-tier picker. A pre-routing
    // install had `user_tier = None`; the user picked their model
    // via the now-deprecated "モデルティア" dropdown which wrote
    // `[providers.configs."embedded.llm"]` repo + filename. We
    // recover their intent by:
    //
    //   1. Reading `repo` + `filename_remote` from the config block.
    //   2. Resolving the matching `ModelTier` via `tier_by_coord`.
    //   3. Mapping `ModelTier::class` (Compact / Standard / Quality)
    //      to the same-named `UserTier` variant.
    //
    // When no match is found (the user hand-edited user.toml to
    // point at a custom GGUF), we leave `user_tier = None`. The
    // `Providers::default` impl already returns `Some(Standard)`
    // for *fresh* installs, but `migrate_legacy_embedded_settings`
    // runs over a *parsed* Settings struct after layering — the
    // user.toml side is authoritative, and a missing field there
    // means we have to derive a value or fall back.
    if s.providers.user_tier.is_none() {
        let derived = s
            .providers
            .configs
            .get(CURRENT_EMBEDDED_PROVIDER_ID)
            .and_then(|cfg| {
                let repo = cfg.get("repo").and_then(|v| v.as_str())?;
                let filename = cfg.get("filename_remote").and_then(|v| v.as_str())?;
                tier_by_coord(repo, filename)
            })
            .map(|t| match t.class {
                ModelTierClass::Compact => crate::translation_router::UserTier::Compact,
                ModelTierClass::Standard => crate::translation_router::UserTier::Standard,
                ModelTierClass::Quality => crate::translation_router::UserTier::Quality,
            })
            // Anything we cannot resolve falls back to Standard —
            // matching the bundled default. Using `unwrap_or_else`
            // (not `or_else(|| Some(...))`) so this branch always
            // produces a value when user_tier was absent.
            .unwrap_or(crate::translation_router::UserTier::Standard);
        s.providers.user_tier = Some(derived);
        changed = true;
    }

    changed
}

/// The fallback when the user's settings reference a coord we no
/// longer ship. Always succeeds because `DEFAULT_TIER_ID` is checked
/// at compile time via the unit tests below.
pub fn default_tier() -> &'static ModelTier {
    tier_by_id(DEFAULT_TIER_ID).expect("DEFAULT_TIER_ID must point at a real entry in TIERS")
}

/// Map an installed-RAM measurement (GB) to the tier class we
/// recommend. The buckets are deliberately conservative — picking
/// "Quality" on an 8 GB host leaves no headroom for the rest of the
/// app, the OS, and the user's other software, even though the GGUF
/// itself fits.
pub fn recommended_class_for_ram(total_ram_gb: f64) -> ModelTierClass {
    if total_ram_gb < 6.0 {
        ModelTierClass::Compact
    } else if total_ram_gb < 12.0 {
        ModelTierClass::Standard
    } else {
        ModelTierClass::Quality
    }
}

/// Resolve the recommended tier for a given RAM measurement. Walks
/// `TIERS` for the first entry of the matching class — keeps the
/// recommendation stable even if multiple variants per class get
/// added later (the first listed wins).
pub fn recommended_tier_for_ram(total_ram_gb: f64) -> &'static ModelTier {
    let class = recommended_class_for_ram(total_ram_gb);
    TIERS
        .iter()
        .find(|t| t.class == class)
        .unwrap_or_else(|| default_tier())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_anchor_ids_are_registered() {
        // `translation_router::route` returns these IDs verbatim
        // when it selects a model. If a tier is dropped or renamed
        // in TIERS without updating the matching constant, the
        // router would later look up a non-existent ID at runtime
        // and silently degrade to the legacy fallback chain. This
        // guard catches the disconnect at test-run-time.
        for id in [
            COMPACT_JA_TIER_ID,
            STANDARD_JA_BRIDGE_TIER_ID,
            STANDARD_MULTI_TIER_ID,
            QUALITY_MULTI_TIER_ID,
        ] {
            assert!(
                tier_by_id(id).is_some(),
                "router anchor `{id}` not in TIERS",
            );
        }
    }

    #[test]
    fn default_tier_id_resolves_to_a_registered_entry() {
        // Compile-time-ish guard: if a future PR drops the default
        // tier without updating the constant, this test fails before
        // a release ships a tool that crashes on first launch.
        assert!(
            tier_by_id(DEFAULT_TIER_ID).is_some(),
            "DEFAULT_TIER_ID `{DEFAULT_TIER_ID}` is missing from TIERS"
        );
    }

    #[test]
    fn every_tier_uses_an_allowed_license() {
        // Allow-list approach: any tier that lands here MUST use a
        // license we have legal sign-off on for commercial offline
        // distribution. Apache-2.0 is the preferred default; Gemma
        // TOS is permitted under the documented redistribution
        // workflow (license + Prohibited Use Policy bundled in the
        // package). Tencent License is permitted for the Hy-MT2
        // translation specialist (2026-05-26 adoption); the license
        // file ships alongside the GGUF in the model directory.
        // New entries with other licenses would silently re-introduce
        // the contamination this allow-list prevents.
        const ALLOWED: &[&str] = &["Apache-2.0", "Gemma-Terms-of-Use", "Tencent License"];
        for t in TIERS {
            assert!(
                ALLOWED.contains(&t.license),
                "tier `{}` license `{}` is not in the allow-list (allowed: {:?})",
                t.id,
                t.license,
                ALLOWED,
            );
        }
    }

    #[test]
    fn chat_template_matches_known_model_family() {
        // Cross-check: the registered chat_template for each tier
        // must align with the model family (inferred from id /
        // repo). A future PR that registers a Qwen 3 tier WITHOUT
        // the no-think variant (or a Qwen 2.5 tier WITH it) would
        // silently wreck translation quality — this guard catches
        // it pre-merge.
        for t in TIERS {
            let id_lc = t.id.to_lowercase();
            let expected = if id_lc.starts_with("qwen3.5") {
                // Qwen 3.5: model card claims thinking is default
                // OFF, but bench confirmed the GGUF still emits
                // `<think>` blocks. Use the same no-think primer
                // as Qwen 3 base. (See registry comment on
                // qwen3.5-0.8b for the Phase 23 evidence.)
                ChatTemplate::ChatMlQwen3NoThink
            } else if id_lc.contains("instruct-2507") {
                // Qwen 3 "Instruct-2507" is the post-thinking
                // checkpoint — reasoning mode was removed at the
                // weights level. Plain ChatML is correct; the
                // no-think primer makes the model emit EOS
                // immediately (Phase 21 v2 finding).
                ChatTemplate::ChatMl
            } else if id_lc.starts_with("qwen3") {
                // Base Qwen 3 ships with reasoning mode on by
                // default; the no-think render is required for
                // short-output translation.
                ChatTemplate::ChatMlQwen3NoThink
            } else if id_lc.starts_with("qwen") {
                ChatTemplate::ChatMl
            } else if id_lc.starts_with("gemma") {
                ChatTemplate::Gemma
            } else if id_lc.starts_with("llm-jp") {
                // llm-jp 3.1 1.8B — JA-specialised, ChatML compatible.
                // (RakutenAI / EuroLLM were dropped — see registry
                // comments at their former position for rationale.)
                ChatTemplate::ChatMl
            } else {
                continue;
            };
            assert_eq!(
                t.chat_template, expected,
                "tier `{}` should use {:?} but is registered as {:?}",
                t.id, expected, t.chat_template,
            );
        }
    }

    #[test]
    fn every_tier_has_a_unique_id() {
        let mut seen = std::collections::HashSet::new();
        for t in TIERS {
            assert!(
                seen.insert(t.id),
                "duplicate tier id `{}`; the lookup helpers depend on uniqueness",
                t.id
            );
        }
    }

    #[test]
    fn every_tier_has_a_unique_repo_primary_filename_pair() {
        // `tier_by_coord` matches against any part filename, but
        // the primary (first) filename must still be unique within
        // a repo so the "current tier" lookup from a saved
        // user.toml's `filename_remote` is unambiguous.
        let mut seen = std::collections::HashSet::new();
        for t in TIERS {
            assert!(
                seen.insert((t.repo, t.primary_filename())),
                "duplicate coord `{}:{}`",
                t.repo,
                t.primary_filename()
            );
        }
    }

    #[test]
    fn every_tier_has_at_least_one_filename() {
        for t in TIERS {
            assert!(
                !t.filenames.is_empty(),
                "tier `{}` has no filenames — downloader cannot run",
                t.id
            );
        }
    }

    #[test]
    fn tier_by_coord_matches_any_part_of_split_tiers() {
        // Regression guard for the 9.x 404 bug: when the user's
        // `filename_remote` recorded the SECOND part of a multi-
        // file GGUF (because they edited user.toml by hand or
        // because a future migration tweaked the canonical name),
        // the tier lookup MUST still resolve so the picker shows
        // the right entry rather than "カスタム".
        let quality = tier_by_id("qwen2.5-7b-instruct-q4_k_m").unwrap();
        if quality.filenames.len() < 2 {
            return; // single-file tier — invariant trivially holds.
        }
        for f in quality.filenames {
            let resolved = tier_by_coord(quality.repo, f);
            assert!(
                resolved.is_some(),
                "tier_by_coord({}, {}) failed to resolve a known part",
                quality.repo,
                f,
            );
            assert_eq!(resolved.unwrap().id, quality.id);
        }
    }

    #[test]
    fn tiers_are_ordered_smallest_to_largest() {
        // The UI dropdown displays them in order. A Quality entry
        // sandwiched between two Compacts would surprise users.
        for window in TIERS.windows(2) {
            assert!(
                window[0].size_mb <= window[1].size_mb,
                "tier `{}` ({} MB) precedes `{}` ({} MB)",
                window[0].id,
                window[0].size_mb,
                window[1].id,
                window[1].size_mb,
            );
        }
    }

    #[test]
    fn ram_recommendation_buckets_are_monotonic() {
        // 2 GB → Compact, 8 GB → Standard, 32 GB → Quality.
        assert_eq!(recommended_class_for_ram(2.0), ModelTierClass::Compact);
        assert_eq!(recommended_class_for_ram(4.0), ModelTierClass::Compact);
        assert_eq!(recommended_class_for_ram(8.0), ModelTierClass::Standard);
        assert_eq!(recommended_class_for_ram(12.0), ModelTierClass::Quality);
        assert_eq!(recommended_class_for_ram(64.0), ModelTierClass::Quality);
    }

    #[test]
    fn recommended_tier_resolves_to_a_concrete_entry_for_every_ram() {
        // Each bucket has at least one tier registered, so the
        // recommendation is never an Option — the picker always has
        // a default to highlight.
        for ram in [1.0, 4.0, 8.0, 16.0, 64.0] {
            let t = recommended_tier_for_ram(ram);
            assert!(
                tier_by_id(t.id).is_some(),
                "recommended tier id `{}` not registered (ram={})",
                t.id,
                ram
            );
        }
    }

    #[test]
    fn coord_format_round_trips_through_tier_by_coord() {
        for t in TIERS {
            let coord = t.coord();
            let (repo, filename) = coord.split_once(':').expect("coord has colon");
            let resolved = tier_by_coord(repo, filename).expect("coord resolves");
            assert_eq!(resolved.id, t.id);
            assert_eq!(filename, t.primary_filename());
        }
    }

    #[test]
    fn migration_rewrites_pre_9x_quality_synthetic_filename() {
        // Repro of the user-reported 9.x 404: pre-9.x recorded a
        // synthetic single-file name that does not exist on HF.
        let mut cfg = serde_json::json!({
            "repo": "Qwen/Qwen2.5-7B-Instruct-GGUF",
            "filename_remote": "qwen2.5-7b-instruct-q4_k_m.gguf",
            "license": "Apache-2.0",
        });
        let migrated = migrate_legacy_embedded_config(&mut cfg);
        assert!(migrated, "stale 7B config must trigger migration");
        let obj = cfg.as_object().unwrap();
        assert_eq!(
            obj.get("filename_remote").and_then(|v| v.as_str()),
            Some("qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf")
        );
        let extras = obj
            .get("filenames_remote_extra")
            .and_then(|v| v.as_array())
            .expect("extras populated");
        assert_eq!(extras.len(), 1);
        assert_eq!(
            extras[0].as_str(),
            Some("qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf"),
        );
    }

    #[test]
    fn migration_is_a_noop_for_already_correct_configs() {
        let mut cfg = serde_json::json!({
            "repo": "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
            "filename_remote": "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        });
        let migrated = migrate_legacy_embedded_config(&mut cfg);
        assert!(!migrated, "matching config must not be touched");
    }

    #[test]
    fn settings_migration_renames_legacy_embedded_id_in_chains_and_configs() {
        // Repro of the 12.x rename: user.toml carries the legacy
        // `embedded.gemma` id in both the translate chain and the
        // per-provider configs map. After migration both must be
        // `embedded.llm`.
        let toml = r#"
            [providers.translate]
            chain = ["embedded.gemma", "ollama.gemma4"]

            [providers.explain]
            chain = ["embedded.gemma"]

            [providers.configs."embedded.gemma"]
            repo = "Qwen/Qwen2.5-1.5B-Instruct-GGUF"
            filename_remote = "qwen2.5-1.5b-instruct-q4_k_m.gguf"
            license = "Apache-2.0"
        "#;
        let defaults = include_str!("settings/default.toml");
        let mut s: crate::settings::schema::Settings =
            crate::settings::store::layer_user_over_defaults_for_test(defaults, toml)
                .expect("layered settings parse");
        let migrated = migrate_legacy_embedded_settings(&mut s);
        assert!(migrated, "legacy id must trigger migration");

        assert!(
            s.providers
                .translate
                .chain
                .iter()
                .all(|e| e != "embedded.gemma"),
            "no chain entry may remain as the legacy id"
        );
        assert!(
            s.providers
                .translate
                .chain
                .iter()
                .any(|e| e == "embedded.llm"),
            "translate chain must reference the new id"
        );
        assert!(
            s.providers
                .explain
                .chain
                .iter()
                .any(|e| e == "embedded.llm"),
            "explain chain must also be migrated"
        );
        assert!(
            !s.providers.configs.contains_key("embedded.gemma"),
            "legacy configs entry must be removed"
        );
        assert!(
            s.providers.configs.contains_key("embedded.llm"),
            "new configs entry must exist after migration"
        );
    }

    #[test]
    fn settings_migration_preserves_user_data_when_both_keys_exist() {
        // Regression for the 13.x bug: before the fix, the migration
        // used `or_insert(legacy)` to move the per-provider config
        // block. But because `default.toml` already provides an
        // `embedded.llm` block, the merged Settings ALWAYS had both
        // `embedded.gemma` (from user.toml — the user's authoritative
        // state) AND `embedded.llm` (from defaults — Standard 1.5B).
        // `or_insert` kept the default-derived entry and silently
        // discarded the user's tier selection, rolling Quality 7B
        // back to Standard 1.5B on every restart.
        let user_toml = r#"
            [providers.translate]
            chain = ["embedded.gemma"]

            [providers.configs."embedded.gemma"]
            repo = "Qwen/Qwen2.5-7B-Instruct-GGUF"
            filename_remote = "qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf"
            filenames_remote_extra = ["qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf"]
            license = "Apache-2.0"
        "#;
        let defaults = include_str!("settings/default.toml");
        let mut s = crate::settings::store::layer_user_over_defaults_for_test(defaults, user_toml)
            .expect("layered settings parse");

        // Sanity: before migration, BOTH keys must coexist (this is
        // exactly the merge state the bug was triggered by — without
        // both keys present the bug does not surface).
        assert!(
            s.providers.configs.contains_key("embedded.gemma"),
            "user.toml's legacy entry must survive the layer merge"
        );
        assert!(
            s.providers.configs.contains_key("embedded.llm"),
            "default.toml's new entry must survive the layer merge"
        );

        let migrated = migrate_legacy_embedded_settings(&mut s);
        assert!(migrated, "migration must report a change");

        // After migration: legacy key gone, embedded.llm carries
        // the USER'S Quality 7B selection (NOT the default Standard
        // 1.5B).
        assert!(!s.providers.configs.contains_key("embedded.gemma"));
        let llm = s
            .providers
            .configs
            .get("embedded.llm")
            .expect("embedded.llm must exist post-migration");
        assert_eq!(
            llm.get("repo").and_then(|v| v.as_str()),
            Some("Qwen/Qwen2.5-7B-Instruct-GGUF"),
            "user's Quality 7B repo MUST survive migration; \
             a default-derived `embedded.llm` value here means the \
             rollback bug has regressed"
        );
        assert_eq!(
            llm.get("filename_remote").and_then(|v| v.as_str()),
            Some("qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf"),
            "user's primary filename MUST survive"
        );
        let extras = llm
            .get("filenames_remote_extra")
            .and_then(|v| v.as_array())
            .expect("multi-part extras MUST survive");
        assert_eq!(extras.len(), 1);
        assert_eq!(
            extras[0].as_str(),
            Some("qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf")
        );
    }

    #[test]
    fn settings_migration_is_idempotent() {
        // Running migration twice on a fresh-default settings file
        // must be a no-op the second time. Guards against partially
        // applying migrations and dirtying user.toml on every launch.
        let defaults = include_str!("settings/default.toml");
        let mut s: crate::settings::schema::Settings =
            toml::from_str(defaults).expect("default.toml parses");
        let first = migrate_legacy_embedded_settings(&mut s);
        let second = migrate_legacy_embedded_settings(&mut s);
        assert!(
            !first,
            "default.toml is already canonical; first run must be a no-op"
        );
        assert!(!second, "second run must always be a no-op");
    }

    #[test]
    fn migration_is_a_noop_for_unknown_repos() {
        // User has a custom plugin model — leave alone.
        let mut cfg = serde_json::json!({
            "repo": "myorg/my-custom-model",
            "filename_remote": "anything.gguf",
        });
        let migrated = migrate_legacy_embedded_config(&mut cfg);
        assert!(!migrated, "unknown repo must not be migrated");
    }

    #[test]
    fn settings_migration_derives_user_tier_from_legacy_embedded_config() {
        // Pre-routing install: user.toml has the legacy embedded
        // config block (with the user's picked tier_id encoded as
        // repo + filename) but no `providers.user_tier`. The
        // migration must derive a `UserTier` that matches the
        // saved model's class so the user does not see their tier
        // visibly "reset" on the first launch after upgrade.
        let user_toml = r#"
            [providers.translate]
            chain = ["embedded.llm"]

            [providers.configs."embedded.llm"]
            repo = "Qwen/Qwen2.5-7B-Instruct-GGUF"
            filename_remote = "qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf"
            filenames_remote_extra = ["qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf"]
            license = "Apache-2.0"
        "#;
        let defaults = include_str!("settings/default.toml");
        let mut s = crate::settings::store::layer_user_over_defaults_for_test(defaults, user_toml)
            .expect("layered settings parse");
        // Pre-migration: user_tier should still default to whatever
        // the layered default produced (Some(Standard) from
        // `Providers::default`). Force it to None so the migration
        // path can do its derivation work — we are simulating an
        // even-older user.toml that predates the Default change.
        s.providers.user_tier = None;
        let migrated = migrate_legacy_embedded_settings(&mut s);
        assert!(migrated, "user_tier derivation must report a change");
        // The Qwen 2.5 7B tier's class is Quality → UserTier::Quality.
        assert_eq!(
            s.providers.user_tier,
            Some(crate::translation_router::UserTier::Quality),
        );
    }

    #[test]
    fn settings_migration_falls_back_to_standard_for_custom_models() {
        // Hand-edited user.toml points at a model we do not ship.
        // The migration cannot match it to a tier class, so it must
        // fall back to Standard rather than leaving `user_tier =
        // None` (the routing-aware embedded provider would then
        // refuse to load).
        let user_toml = r#"
            [providers.translate]
            chain = ["embedded.llm"]

            [providers.configs."embedded.llm"]
            repo = "myorg/my-custom-finetune"
            filename_remote = "custom.gguf"
        "#;
        let defaults = include_str!("settings/default.toml");
        let mut s = crate::settings::store::layer_user_over_defaults_for_test(defaults, user_toml)
            .expect("layered settings parse");
        s.providers.user_tier = None;
        let migrated = migrate_legacy_embedded_settings(&mut s);
        assert!(migrated);
        assert_eq!(
            s.providers.user_tier,
            Some(crate::translation_router::UserTier::Standard),
            "unknown coord must fall back to Standard",
        );
    }

    #[test]
    fn default_tier_is_the_standard_class() {
        // The install-and-go default targets typical office hardware
        // (8 GB RAM). If a future tuning pass swaps the default to a
        // Compact, this test fails so we re-evaluate the on-boarding
        // copy ("約 1GB のモデルをダウンロードします") which assumes
        // the Standard tier.
        assert_eq!(default_tier().class, ModelTierClass::Standard);
    }
}
