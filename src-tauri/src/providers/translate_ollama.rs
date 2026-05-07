//! Ollama HTTP client for local LLM translation (default: gemma4).

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput};

#[derive(Debug, Clone)]
pub struct FewShotExample {
    pub source: String,
    pub target: String,
}

pub struct OllamaTranslator {
    id: String,
    endpoint: String,
    model: String,
    temperature: f32,
    top_p: f32,
    num_ctx: u32,
    num_predict: Option<u32>,
    /// Few-shot example pairs prepended to every translate call.
    /// Supplying 2-4 high-quality examples biases Gemma toward the
    /// target style (idiomatic phrasing, proper counter words,
    /// omitted subjects, natural sentence order) without
    /// constraining the output to exact words in the examples.
    /// Configure via `"examples": [{"source": "...", "target": "..."}]`.
    examples: Vec<FewShotExample>,
    /// Reasoning-capable models (gemma4, qwen3, deepseek, ...) run a
    /// silent "thinking" pass before emitting visible tokens, adding
    /// seconds to first-token latency. We default to `false` because
    /// translation does not benefit from reasoning and the latency hit
    /// is unacceptable for Lens streaming (bench 2026-04-20: 10x slower
    /// with thinking on). Users can opt in via settings for models that
    /// benefit from it (Explain / Summarize capabilities may want true).
    think: bool,
    system_prompt: String,
    http: reqwest::Client,
    /// Callback invoked for each streaming chunk received from Ollama.
    /// `(delta, done)` — both forwarded verbatim to the UI as a
    /// `TaskOutput::TextChunk` event. `None` disables streaming and the
    /// provider returns the aggregated `Text` output synchronously.
    chunk_sink: Option<Arc<dyn Fn(String, bool) + Send + Sync>>,
}

impl OllamaTranslator {
    pub fn new(id: impl Into<String>, config: &serde_json::Value) -> Self {
        let get_str = |k: &str, d: &str| {
            config
                .get(k)
                .and_then(|v| v.as_str())
                .unwrap_or(d)
                .to_string()
        };
        let get_f = |k: &str, d: f64| config.get(k).and_then(|v| v.as_f64()).unwrap_or(d) as f32;
        let get_u = |k: &str, d: u64| config.get(k).and_then(|v| v.as_u64()).unwrap_or(d) as u32;
        let get_bool =
            |k: &str, d: bool| config.get(k).and_then(|v| v.as_bool()).unwrap_or(d);
        let get_opt_u = |k: &str| config.get(k).and_then(|v| v.as_u64()).map(|v| v as u32);
        let examples: Vec<FewShotExample> = config
            .get("examples")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|ex| {
                        let src = ex.get("source").and_then(|v| v.as_str())?;
                        let tgt = ex.get("target").and_then(|v| v.as_str())?;
                        Some(FewShotExample {
                            source: src.to_string(),
                            target: tgt.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            id: id.into(),
            endpoint: get_str("endpoint", "http://127.0.0.1:11434"),
            // Default model changed 2026-04-20 from 7B quantised to E2B
            // (effective 2B, MatFormer). Bench shows ~373ms first-token on
            // RTX 4080 versus several seconds for a 7B class model.
            model: get_str("model", "gemma4:e2b"),
            // 0.4 favours natural, idiomatic phrasing without the
            // semantic drift 0.6+ would introduce. See
            // `settings/default.toml` comment for the full rationale.
            temperature: get_f("temperature", 0.4),
            top_p: get_f("top_p", 0.95),
            // Translation rarely benefits from >4K context; prefill time
            // scales with num_ctx so keep it tight unless user overrides.
            num_ctx: get_u("num_ctx", 4096),
            num_predict: get_opt_u("num_predict").or(Some(256)),
            think: get_bool("think", false),
            system_prompt: get_str(
                "system_prompt",
                "You are a professional translator. Render the user's text naturally and idiomatically in the target language — match the meaning, tone, and register a native reader would expect, not the word order of the source. Preserve newlines exactly. Output ONLY the translated text. Never include commentary, explanations, romanizations, alternative translations, notes in parentheses, surrounding quotes, or bracketed markers like `[1]` / `[2]`.",
            ),
            examples,
            http: reqwest::Client::new(),
            chunk_sink: None,
        }
    }

    /// Attach a per-chunk sink so the UI can render streaming tokens as
    /// they arrive. The aggregated text is still returned from `run`.
    pub fn with_chunk_sink(mut self, sink: Arc<dyn Fn(String, bool) + Send + Sync>) -> Self {
        self.chunk_sink = Some(sink);
        self
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage>,
    stream: bool,
    /// Disables reasoning pass on models that support it. Serialised
    /// unconditionally — older Ollama versions ignore unknown fields.
    think: bool,
    options: Options,
}

#[derive(Serialize)]
struct Options {
    temperature: f32,
    top_p: f32,
    num_ctx: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Deserialize)]
struct ChatStreamChunk {
    #[serde(default)]
    message: Option<ChatMessageContent>,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize, Default)]
struct ChatMessageContent {
    #[serde(default)]
    content: String,
}

/// Build the ordered `messages` list that `/api/chat` expects.
///
/// Few-shot examples are serialised as alternating `user` /
/// `assistant` turns instead of being dumped into a single giant
/// user prompt. That matters because the previous `/api/generate`
/// approach mixed examples + instructions + source into one blob,
/// and Gemma occasionally "translated" the instruction block itself
/// when the source text was trivially short (regression: english0004
/// produced the entire prompt as Japanese output for a stray "0").
/// Splitting on role turns each example into a closed conversation
/// turn the model can attend to without confusing it with live
/// input.
pub fn build_chat_messages(
    system_prompt: &str,
    examples: &[FewShotExample],
    source_language: &str,
    target_language: &str,
    text: &str,
) -> Vec<ChatMessage> {
    let mut messages = Vec::with_capacity(2 + examples.len() * 2);
    messages.push(ChatMessage {
        role: "system".into(),
        content: system_prompt.to_string(),
    });
    let from_clause = if source_language.trim().is_empty()
        || source_language.eq_ignore_ascii_case("auto")
    {
        String::new()
    } else {
        format!(" from {source_language}")
    };
    for ex in examples {
        messages.push(ChatMessage {
            role: "user".into(),
            content: format!(
                "Target language: {tgt}. Rewrite the following{from_clause} naturally in {tgt}:\n\n{src}",
                tgt = target_language,
                from_clause = from_clause,
                src = ex.source,
            ),
        });
        messages.push(ChatMessage {
            role: "assistant".into(),
            content: ex.target.clone(),
        });
    }
    messages.push(ChatMessage {
        role: "user".into(),
        content: format!(
            "Target language: {tgt}. Rewrite the following{from_clause} naturally in {tgt}:\n\n{txt}",
            tgt = target_language,
            from_clause = from_clause,
            txt = text,
        ),
    });
    messages
}

/// Iteratively fold streaming JSON chunks from `/api/chat` into a
/// single aggregated response. Extracted so the parsing can be
/// unit-tested without an HTTP stack.
pub fn merge_chat_stream_chunks<I: IntoIterator<Item = String>>(lines: I) -> String {
    let mut out = String::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(chunk) = serde_json::from_str::<ChatStreamChunk>(trimmed) {
            if let Some(msg) = chunk.message {
                out.push_str(&msg.content);
            }
            if chunk.done {
                break;
            }
        }
    }
    out
}

#[async_trait]
impl Provider for OllamaTranslator {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[
            Capability::Translate,
            Capability::Explain,
            Capability::Summarize,
        ]
    }
    fn is_local(&self) -> bool {
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let messages = match input {
            TaskInput::Translate {
                text,
                source_language,
                target_language,
            } => build_chat_messages(
                &self.system_prompt,
                &self.examples,
                &source_language,
                &target_language,
                &text,
            ),
            TaskInput::Explain { prompt, .. } => vec![
                ChatMessage {
                    role: "system".into(),
                    content: self.system_prompt.clone(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: prompt,
                },
            ],
            TaskInput::Summarize { text } => vec![
                ChatMessage {
                    role: "system".into(),
                    content: self.system_prompt.clone(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: format!("Summarize concisely:\n\n{text}"),
                },
            ],
            _ => {
                return Err(ProviderError::Other(
                    "OllamaTranslator cannot handle this input".into(),
                ))
            }
        };

        let url = format!("{}/api/chat", self.endpoint.trim_end_matches('/'));
        let req = ChatRequest {
            model: &self.model,
            messages,
            stream: true,
            think: self.think,
            options: Options {
                temperature: self.temperature,
                top_p: self.top_p,
                num_ctx: self.num_ctx,
                num_predict: self.num_predict,
            },
        };

        let resp = self.http.post(&url).json(&req).send().await.map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout(0)
            } else if e.is_connect() {
                ProviderError::Offline
            } else {
                ProviderError::Other(e.to_string())
            }
        })?;

        // Parse line-delimited JSON chunks; `/api/chat` wraps each
        // delta under `message.content`. The sink fires per chunk so
        // the UI can render tokens live, while we aggregate into a
        // single Text output so existing call sites keep working
        // unchanged.
        let mut aggregated = String::new();
        let mut buffer = Vec::<u8>::new();
        let mut bytes = resp.bytes_stream();
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(|e| ProviderError::Other(e.to_string()))?;
            buffer.extend_from_slice(&chunk);
            while let Some(nl) = buffer.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buffer.drain(..=nl).collect();
                let line_str = match std::str::from_utf8(&line) {
                    Ok(s) => s.trim().to_string(),
                    Err(_) => continue,
                };
                if line_str.is_empty() {
                    continue;
                }
                if let Ok(c) = serde_json::from_str::<ChatStreamChunk>(&line_str) {
                    let delta = c
                        .message
                        .map(|m| m.content)
                        .unwrap_or_default();
                    if !delta.is_empty() {
                        aggregated.push_str(&delta);
                        if let Some(sink) = &self.chunk_sink {
                            sink(delta, c.done);
                        }
                    } else if c.done {
                        if let Some(sink) = &self.chunk_sink {
                            sink(String::new(), true);
                        }
                    }
                }
            }
        }
        if !buffer.is_empty() {
            if let Ok(line) = std::str::from_utf8(&buffer) {
                if let Ok(c) = serde_json::from_str::<ChatStreamChunk>(line.trim()) {
                    if let Some(msg) = c.message {
                        aggregated.push_str(&msg.content);
                    }
                }
            }
        }
        Ok(TaskOutput::Text(aggregated))
    }
}

#[cfg(test)]
impl OllamaTranslator {
    pub(crate) fn think_for_test(&self) -> bool {
        self.think
    }
    pub(crate) fn model_for_test(&self) -> &str {
        &self.model
    }
    pub(crate) fn num_ctx_for_test(&self) -> u32 {
        self.num_ctx
    }
    pub(crate) fn num_predict_for_test(&self) -> Option<u32> {
        self.num_predict
    }
    pub(crate) fn temperature_for_test(&self) -> f32 {
        self.temperature
    }
    pub(crate) fn top_p_for_test(&self) -> f32 {
        self.top_p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_disables_thinking_and_picks_e2b_model() {
        // Bench 2026-04-20 verdict: thinking mode costs 10x first-token
        // latency on gemma4 class models. Default MUST be false — if a
        // future refactor flips this, Lens streaming goes from ~370ms to
        // ~3700ms first-token.
        let t = OllamaTranslator::new("ollama.gemma4", &serde_json::Value::Null);
        assert!(!t.think_for_test(), "default `think` must be false");
        assert_eq!(t.model_for_test(), "gemma4:e2b");
        assert_eq!(t.num_ctx_for_test(), 4096);
        assert_eq!(t.num_predict_for_test(), Some(256));
    }

    #[test]
    fn default_sampling_favours_natural_output() {
        // 2026-04-22: earlier 0.2 / 0.9 defaults produced word-for-word
        // translations that read like machine output. 0.4 / 0.95 give
        // Gemma enough room to pick idiomatic phrasings without
        // hallucinating. Guard the defaults so a future tuning pass
        // doesn't silently regress to rigid output.
        let t = OllamaTranslator::new("ollama.gemma4", &serde_json::Value::Null);
        assert!(
            (t.temperature_for_test() - 0.4).abs() < 1e-4,
            "default temperature should be 0.4, got {}",
            t.temperature_for_test()
        );
        assert!(
            (t.top_p_for_test() - 0.95).abs() < 1e-4,
            "default top_p should be 0.95, got {}",
            t.top_p_for_test()
        );
    }

    #[test]
    fn config_can_override_think_and_num_predict() {
        // Explain / Summarize capabilities may genuinely want thinking
        // on. The override path must honour that rather than forcing
        // `false` at the call site.
        let cfg = serde_json::json!({
            "think": true,
            "num_predict": 512,
            "num_ctx": 8192,
            "model": "gemma4:e4b",
        });
        let t = OllamaTranslator::new("x", &cfg);
        assert!(t.think_for_test());
        assert_eq!(t.num_predict_for_test(), Some(512));
        assert_eq!(t.num_ctx_for_test(), 8192);
        assert_eq!(t.model_for_test(), "gemma4:e4b");
    }

    #[test]
    fn merge_chat_stream_chunks_concatenates_content_fields() {
        let lines = vec![
            "{\"message\":{\"role\":\"assistant\",\"content\":\"Hel\"},\"done\":false}".to_string(),
            "{\"message\":{\"role\":\"assistant\",\"content\":\"lo\"},\"done\":false}".to_string(),
            "{\"message\":{\"role\":\"assistant\",\"content\":\" world\"},\"done\":true}".to_string(),
        ];
        assert_eq!(merge_chat_stream_chunks(lines), "Hello world");
    }

    #[test]
    fn merge_chat_stream_chunks_stops_at_done_marker() {
        // Late chunks after `done:true` are ignored to match Ollama
        // semantics.
        let lines = vec![
            "{\"message\":{\"role\":\"assistant\",\"content\":\"abc\"},\"done\":true}".to_string(),
            "{\"message\":{\"role\":\"assistant\",\"content\":\"should not appear\"},\"done\":false}".to_string(),
        ];
        assert_eq!(merge_chat_stream_chunks(lines), "abc");
    }

    #[test]
    fn merge_chat_stream_chunks_ignores_blank_and_invalid_lines() {
        let lines = vec![
            "".to_string(),
            "   ".to_string(),
            "not json".to_string(),
            "{\"message\":{\"role\":\"assistant\",\"content\":\"ok\"},\"done\":true}".to_string(),
        ];
        assert_eq!(merge_chat_stream_chunks(lines), "ok");
    }

    #[test]
    fn merge_chat_stream_chunks_tolerates_missing_message_field() {
        // Final "done" chunks from Ollama arrive without a `message`
        // body in some versions — treat that as empty delta.
        let lines = vec![
            "{\"done\":false}".to_string(),
            "{\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":false}".to_string(),
            "{\"message\":{\"role\":\"assistant\",\"content\":\"x\"},\"done\":true}".to_string(),
        ];
        assert_eq!(merge_chat_stream_chunks(lines), "x");
    }

    #[test]
    fn build_chat_messages_places_system_first_then_user_for_simple_input() {
        let msgs = build_chat_messages(
            "You are a translator.",
            &[],
            "en",
            "ja",
            "Hello world",
        );
        assert_eq!(msgs.len(), 2, "no examples → [system, user]");
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs[0].content, "You are a translator.");
        assert_eq!(msgs[1].role, "user");
        assert!(msgs[1].content.contains("Target language: ja"));
        assert!(msgs[1].content.contains("from en"));
        assert!(msgs[1].content.ends_with("Hello world"));
    }

    #[test]
    fn build_chat_messages_interleaves_examples_as_user_assistant_turns() {
        // CRITICAL: the regression that motivated the /api/chat move
        // was Gemma echoing the entire few-shot block as the
        // "translation" of a trivially short source ("0"). Splitting
        // examples into distinct turns prevents that — the model sees
        // each example as a closed conversation, not as live input to
        // echo back.
        let examples = vec![
            FewShotExample {
                source: "Hello".into(),
                target: "こんにちは".into(),
            },
            FewShotExample {
                source: "Thank you".into(),
                target: "ありがとう".into(),
            },
        ];
        let msgs = build_chat_messages(
            "SYS",
            &examples,
            "en",
            "ja",
            "Goodbye",
        );
        assert_eq!(msgs.len(), 6, "system + 2*(user+assistant) + final user");
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs[1].role, "user");
        assert!(msgs[1].content.contains("Hello"));
        assert_eq!(msgs[2].role, "assistant");
        assert_eq!(msgs[2].content, "こんにちは");
        assert_eq!(msgs[3].role, "user");
        assert!(msgs[3].content.contains("Thank you"));
        assert_eq!(msgs[4].role, "assistant");
        assert_eq!(msgs[4].content, "ありがとう");
        assert_eq!(msgs[5].role, "user");
        assert!(msgs[5].content.contains("Goodbye"));
    }

    #[test]
    fn build_chat_messages_omits_from_clause_when_language_is_auto() {
        let msgs = build_chat_messages("SYS", &[], "auto", "ja", "hi");
        let user_turn = msgs.last().expect("final user turn");
        assert!(
            !user_turn.content.contains("from "),
            "auto source → no `from <lang>` hint"
        );
    }

    #[test]
    fn build_chat_messages_trivial_source_still_gets_scoped_prompt() {
        // Regression guard: previously "0" caused the model to
        // translate the instruction block because the instructions
        // were inline. With role separation, even a trivial source
        // lands as a dedicated user turn whose content is ONLY the
        // target-language hint + the source text.
        let examples = vec![FewShotExample {
            source: "Thank you".into(),
            target: "ありがとう".into(),
        }];
        let msgs = build_chat_messages("SYS", &examples, "en", "ja", "0");
        let final_user = msgs.last().expect("final user turn");
        assert_eq!(final_user.role, "user");
        assert!(final_user.content.ends_with("0"));
        assert!(
            !final_user.content.contains("SOURCE:"),
            "examples must not leak into the final user turn"
        );
        assert!(
            !final_user.content.contains("Thank you"),
            "example source must stay scoped to its own turn"
        );
    }
}
