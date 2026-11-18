//! Ollama HTTP client for local LLM translation.
//!
//! Latency notes (measured, see `docs/translation-models.md`):
//! - `think: false` is sent on every request. Ollama turns reasoning *on*
//!   by default for thinking-capable models (Gemma 4, Qwen 3.5, ...),
//!   which streams hundreds of hidden tokens before the first translated
//!   one and blew straight through the translate timeout.
//! - `keep_alive` keeps the weights resident between captures; Ollama's
//!   5-minute default meant the next capture after a short break paid a
//!   multi-second cold load.
//! - [`OllamaTranslator::preload`] loads the model at app start so even
//!   the first capture is warm.

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::glossary::{pfml3_form, pfml3_glossary, TranslateOptions};
use super::lang::translation_prompt_with;
use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput};

/// Default local model. Chosen by the benchmark in
/// `docs/translation-models.md`; keep the two in sync.
pub const DEFAULT_MODEL: &str = "gemma4:e4b";
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434";
pub const DEFAULT_KEEP_ALIVE: &str = "30m";

pub struct OllamaTranslator {
    id: String,
    endpoint: String,
    model: String,
    temperature: f32,
    top_p: f32,
    num_ctx: u32,
    keep_alive: String,
    think: bool,
    /// Optional override. When empty, the prompt from
    /// [`translation_prompt`] is used for translate requests.
    system_prompt: Option<String>,
    http: reqwest::Client,
    /// Callback invoked for each streaming chunk received from Ollama.
    /// `(delta, done)` — both forwarded verbatim to the UI as a
    /// `TaskOutput::TextChunk` event. `None` disables streaming and the
    /// provider returns the aggregated `Text` output synchronously.
    chunk_sink: Option<Arc<dyn Fn(String, bool) + Send + Sync>>,
    /// User glossary / Japanese style (Settings → 翻訳).
    options: Arc<TranslateOptions>,
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
        let system_prompt = config
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        Self {
            id: id.into(),
            endpoint: get_str("endpoint", DEFAULT_ENDPOINT),
            model: get_str("model", DEFAULT_MODEL),
            temperature: get_f("temperature", 0.0),
            top_p: get_f("top_p", 0.9),
            num_ctx: get_u("num_ctx", 4096),
            keep_alive: get_str("keep_alive", DEFAULT_KEEP_ALIVE),
            think: config
                .get("think")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            system_prompt,
            http: reqwest::Client::new(),
            chunk_sink: None,
            options: Arc::default(),
        }
    }

    pub fn with_options(mut self, options: Arc<TranslateOptions>) -> Self {
        self.options = options;
        self
    }

    /// Attach a per-chunk sink so the UI can render streaming tokens as
    /// they arrive. The aggregated text is still returned from `run`.
    pub fn with_chunk_sink(mut self, sink: Arc<dyn Fn(String, bool) + Send + Sync>) -> Self {
        self.chunk_sink = Some(sink);
        self
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.endpoint.trim_end_matches('/'))
    }

    /// Ask Ollama to load the model without generating anything (an empty
    /// prompt is Ollama's documented "preload" request). Errors are
    /// returned so the caller can log them, but nothing depends on this
    /// succeeding: a later `run` simply pays the load itself.
    pub async fn preload(&self) -> Result<(), ProviderError> {
        let body = serde_json::json!({
            "model": self.model,
            "prompt": "",
            "stream": false,
            "keep_alive": self.keep_alive,
        });
        let resp = self
            .http
            .post(self.url("/api/generate"))
            .json(&body)
            .send()
            .await
            .map_err(map_reqwest_err)?;
        check_status(&self.model, resp).await.map(|_| ())
    }
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    system: String,
    /// `raw` bypasses the model's chat template; used for PLaMo's PFML3.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    raw: bool,
    stream: bool,
    think: bool,
    keep_alive: &'a str,
    options: Options,
}

#[derive(Serialize)]
struct Options {
    temperature: f32,
    top_p: f32,
    num_ctx: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<&'static str>,
}

/// PLaMo 3 Translate is not a chat model: it expects a PFML3 prompt and
/// stops on its own tag tokens (see the model card on Hugging Face).
pub fn is_plamo_translate(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("plamo-3-translate") || m.contains("plamo3-translate")
}

pub const PLAMO_STOP: [&str; 2] = ["<|plamo:tag|>", "<|plamo:bos|>"];

/// PFML3 prompt for one translation. The source language is detected by
/// the model; only the target language code is given.
pub fn plamo_prompt(text: &str, target: &str) -> String {
    plamo_prompt_with(text, target, &TranslateOptions::default())
}

/// [`plamo_prompt`] with the user's glossary and Japanese style.
pub fn plamo_prompt_with(text: &str, target: &str, opts: &TranslateOptions) -> String {
    let glossary = pfml3_glossary(&opts.terms_for(text, target));
    let form = pfml3_form(opts.form_for(target));
    let lang = target.trim().to_ascii_lowercase();
    // PFML3 wants plain language codes without region subtags, except the
    // Chinese script variants.
    let lang = match lang.as_str() {
        "zh-tw" | "zh-hk" | "zh-hant" => "zh-hant".to_string(),
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" => "zh".to_string(),
        other => other.split(['-', '_']).next().unwrap_or(other).to_string(),
    };
    // Reserved tokens inside OCR text would corrupt the prompt structure.
    let text = text.replace("<|plamo:", "<|plamo :");
    format!(
        "<|plamo:tag|>dataset<|plamo:msg|>translation{glossary}<|plamo:tag|>input<|plamo:msg|>{text}\
         <|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>{lang}{form}<|plamo:msg|>"
    )
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    response: String,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

fn map_reqwest_err(e: reqwest::Error) -> ProviderError {
    if e.is_timeout() {
        ProviderError::Timeout(0)
    } else if e.is_connect() {
        ProviderError::Offline
    } else {
        ProviderError::Other(e.to_string())
    }
}

/// Turn a non-2xx Ollama response into a readable error. The most common
/// one in practice is a model that was never pulled, which Ollama reports
/// as a 404 with `{"error":"model '...' not found"}`; before this check
/// the JSON parsed as an empty chunk and the user got a blank translation.
async fn check_status(
    model: &str,
    resp: reqwest::Response,
) -> Result<reqwest::Response, ProviderError> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let text = resp.text().await.unwrap_or_default();
    let msg = serde_json::from_str::<ErrorBody>(&text)
        .map(|b| b.error)
        .unwrap_or(text);
    Err(ProviderError::Other(describe_error(
        model,
        status.as_u16(),
        &msg,
    )))
}

pub(crate) fn describe_error(model: &str, status: u16, msg: &str) -> String {
    if status == 404 || msg.contains("not found") {
        format!("ollama model '{model}' is not installed; run `ollama pull {model}` ({msg})")
    } else {
        format!("ollama HTTP {status}: {msg}")
    }
}

/// Iteratively fold streaming JSON chunks into a single response.
/// Extracted so the parsing can be unit-tested without an HTTP stack.
pub fn merge_stream_chunks<I: IntoIterator<Item = String>>(lines: I) -> String {
    let mut out = String::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(trimmed) {
            out.push_str(&chunk.response);
            if chunk.done {
                break;
            }
        }
    }
    out
}

/// Some small models wrap the answer in a label or quotes despite the
/// instructions ("Translation: ...", "「...」"). Strip only those exact
/// wrappers so legitimate content is never touched.
pub fn clean_translation(raw: &str) -> String {
    let mut s = raw.trim();
    for label in ["Translation:", "翻訳:", "翻訳："] {
        if let Some(rest) = s.strip_prefix(label) {
            s = rest.trim_start();
        }
    }
    s.to_string()
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

    async fn warm_up(&self) -> Result<(), ProviderError> {
        self.preload().await
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let is_translate = matches!(input, TaskInput::Translate { .. });
        let plamo = is_plamo_translate(&self.model);
        // Glossary / style lines, kept for a custom system prompt too.
        let extra = match &input {
            TaskInput::Translate {
                text,
                target_language,
                ..
            } if !plamo => self.options.chat_instructions(text, target_language),
            _ => None,
        };
        let (system, prompt) = match input {
            TaskInput::Translate {
                text,
                target_language,
                ..
            } if plamo => (
                String::new(),
                plamo_prompt_with(&text, &target_language, &self.options),
            ),
            TaskInput::Translate {
                text,
                source_language,
                target_language,
                ..
            } => translation_prompt_with(&text, &source_language, &target_language, &self.options),
            TaskInput::Explain { prompt, .. } => (String::new(), prompt),
            TaskInput::Summarize { text } => {
                (String::new(), format!("Summarize concisely:\n\n{text}"))
            }
            _ => {
                return Err(ProviderError::Other(
                    "OllamaTranslator cannot handle this input".into(),
                ))
            }
        };
        let raw = plamo && is_translate;
        let system = match &self.system_prompt {
            Some(custom) if !raw && is_translate => match &extra {
                Some(extra) => format!("{custom}\n\n{extra}"),
                None => custom.clone(),
            },
            Some(custom) if !raw => custom.clone(),
            _ => system,
        };

        let req = GenerateRequest {
            model: &self.model,
            prompt,
            system,
            raw,
            stream: true,
            think: self.think,
            keep_alive: &self.keep_alive,
            options: Options {
                temperature: self.temperature,
                top_p: self.top_p,
                num_ctx: self.num_ctx,
                stop: if raw { PLAMO_STOP.to_vec() } else { Vec::new() },
            },
        };

        let resp = self
            .http
            .post(self.url("/api/generate"))
            .json(&req)
            .send()
            .await
            .map_err(map_reqwest_err)?;
        let resp = check_status(&self.model, resp).await?;

        // Parse line-delimited JSON chunks; each one contains `response`
        // plus a `done` flag. The sink fires per chunk so the UI can
        // render tokens live, while we aggregate into a single Text
        // output so existing call sites keep working unchanged.
        let mut aggregated = String::new();
        let mut buffer = Vec::<u8>::new();
        let mut bytes = resp.bytes_stream();
        let handle_line = |line: &[u8], aggregated: &mut String| -> Result<(), ProviderError> {
            let Ok(line_str) = std::str::from_utf8(line) else {
                return Ok(());
            };
            let line_str = line_str.trim();
            if line_str.is_empty() {
                return Ok(());
            }
            if let Ok(c) = serde_json::from_str::<StreamChunk>(line_str) {
                if let Some(err) = c.error {
                    return Err(ProviderError::Other(describe_error(&self.model, 500, &err)));
                }
                if !c.response.is_empty() {
                    aggregated.push_str(&c.response);
                    if let Some(sink) = &self.chunk_sink {
                        sink(c.response, c.done);
                    }
                } else if c.done {
                    if let Some(sink) = &self.chunk_sink {
                        sink(String::new(), true);
                    }
                }
            }
            Ok(())
        };
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(|e| ProviderError::Other(e.to_string()))?;
            buffer.extend_from_slice(&chunk);
            // Flush every complete line (newline-delimited JSON).
            while let Some(nl) = buffer.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buffer.drain(..=nl).collect();
                handle_line(&line, &mut aggregated)?;
            }
        }
        // Any trailing bytes without a newline (rare) can still parse.
        if !buffer.is_empty() {
            let rest = std::mem::take(&mut buffer);
            handle_line(&rest, &mut aggregated)?;
        }
        if is_translate {
            let cleaned = clean_translation(&aggregated);
            if cleaned.is_empty() {
                return Err(ProviderError::Other(format!(
                    "ollama model '{}' returned an empty translation",
                    self.model
                )));
            }
            return Ok(TaskOutput::Text(cleaned));
        }
        Ok(TaskOutput::Text(aggregated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_stream_chunks_concatenates_response_fields() {
        let lines = vec![
            "{\"response\":\"Hel\",\"done\":false}".to_string(),
            "{\"response\":\"lo\",\"done\":false}".to_string(),
            "{\"response\":\" world\",\"done\":true}".to_string(),
        ];
        assert_eq!(merge_stream_chunks(lines), "Hello world");
    }

    #[test]
    fn merge_stream_chunks_stops_at_done_marker() {
        // Late chunks after `done:true` are ignored to match Ollama
        // semantics.
        let lines = vec![
            "{\"response\":\"abc\",\"done\":true}".to_string(),
            "{\"response\":\"should not appear\",\"done\":false}".to_string(),
        ];
        assert_eq!(merge_stream_chunks(lines), "abc");
    }

    #[test]
    fn merge_stream_chunks_ignores_blank_and_invalid_lines() {
        let lines = vec![
            "".to_string(),
            "   ".to_string(),
            "not json".to_string(),
            "{\"response\":\"ok\",\"done\":true}".to_string(),
        ];
        assert_eq!(merge_stream_chunks(lines), "ok");
    }

    #[test]
    fn merge_stream_chunks_tolerates_empty_response_field() {
        let lines = vec![
            "{\"done\":false}".to_string(),
            "{\"response\":\"\",\"done\":false}".to_string(),
            "{\"response\":\"x\",\"done\":true}".to_string(),
        ];
        assert_eq!(merge_stream_chunks(lines), "x");
    }

    #[test]
    fn clean_translation_strips_only_label_wrappers() {
        assert_eq!(clean_translation("Translation: Hello"), "Hello");
        assert_eq!(clean_translation("翻訳：こんにちは\n"), "こんにちは");
        assert_eq!(
            clean_translation("The Translation: section"),
            "The Translation: section"
        );
    }

    #[test]
    fn describe_error_points_at_ollama_pull_for_missing_models() {
        let m = describe_error("gemma4:e4b", 404, "model 'gemma4:e4b' not found");
        assert!(m.contains("ollama pull gemma4:e4b"), "{m}");
        let m = describe_error("x", 500, "boom");
        assert_eq!(m, "ollama HTTP 500: boom");
    }

    /// Minimal one-shot HTTP server: answers the first request with
    /// `status` + `body` and hands back the request body it received.
    async fn mock_ollama(
        status: &'static str,
        body: String,
    ) -> (String, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                let n = sock.read(&mut tmp).await.unwrap();
                buf.extend_from_slice(&tmp[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(idx) = text.find("\r\n\r\n") {
                    let len = text[..idx]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if buf.len() >= idx + 4 + len {
                        let resp = format!(
                            "HTTP/1.1 {status}\r\ncontent-type: application/x-ndjson\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        sock.write_all(resp.as_bytes()).await.unwrap();
                        return String::from_utf8_lossy(&buf[idx + 4..idx + 4 + len]).to_string();
                    }
                }
                if n == 0 {
                    return String::new();
                }
            }
        });
        (format!("http://{addr}"), handle)
    }

    fn translator(endpoint: &str) -> OllamaTranslator {
        OllamaTranslator::new(
            "ollama.local",
            &serde_json::json!({ "endpoint": endpoint, "model": "test-model" }),
        )
    }

    fn translate_input() -> TaskInput {
        TaskInput::Translate {
            text: "こんにちは".into(),
            source_language: "auto".into(),
            target_language: "en".into(),
            context: Default::default(),
        }
    }

    #[tokio::test]
    async fn run_sends_think_false_keep_alive_and_named_languages() {
        let body = "{\"response\":\"Hel\",\"done\":false}\n{\"response\":\"lo\",\"done\":false}\n{\"response\":\"\",\"done\":true}\n".to_string();
        let (url, req) = mock_ollama("200 OK", body).await;
        let out = translator(&url).run(translate_input()).await.unwrap();
        match out {
            TaskOutput::Text(t) => assert_eq!(t, "Hello"),
            other => panic!("unexpected {other:?}"),
        }
        let sent: serde_json::Value = serde_json::from_str(&req.await.unwrap()).unwrap();
        assert_eq!(sent["think"], serde_json::json!(false));
        assert_eq!(sent["keep_alive"], serde_json::json!(DEFAULT_KEEP_ALIVE));
        assert_eq!(sent["model"], serde_json::json!("test-model"));
        assert_eq!(sent["prompt"], serde_json::json!("こんにちは"));
        let system = sent["system"].as_str().unwrap();
        assert!(system.contains("into English"), "{system}");
        assert!(!system.contains("from auto"), "{system}");
    }

    #[tokio::test]
    async fn missing_model_is_an_error_not_an_empty_translation() {
        let (url, _req) = mock_ollama(
            "404 Not Found",
            "{\"error\":\"model 'test-model' not found\"}".into(),
        )
        .await;
        let err = translator(&url).run(translate_input()).await.unwrap_err();
        assert!(err.to_string().contains("ollama pull test-model"), "{err}");
    }

    #[tokio::test]
    async fn mid_stream_error_is_surfaced() {
        let (url, _req) = mock_ollama(
            "200 OK",
            "{\"response\":\"x\",\"done\":false}\n{\"error\":\"out of memory\"}\n".into(),
        )
        .await;
        let err = translator(&url).run(translate_input()).await.unwrap_err();
        assert!(err.to_string().contains("out of memory"), "{err}");
    }

    #[tokio::test]
    async fn empty_translation_is_an_error_so_the_chain_falls_back() {
        let (url, _req) = mock_ollama("200 OK", "{\"response\":\"\",\"done\":true}\n".into()).await;
        let err = translator(&url).run(translate_input()).await.unwrap_err();
        assert!(err.to_string().contains("empty translation"), "{err}");
    }

    #[tokio::test]
    async fn unreachable_server_maps_to_offline() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let err = translator(&format!("http://{addr}"))
            .run(translate_input())
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Offline), "{err:?}");
    }

    #[tokio::test]
    async fn warm_up_posts_an_empty_prompt() {
        let (url, req) = mock_ollama("200 OK", "{\"done\":true}".into()).await;
        translator(&url).preload().await.unwrap();
        let sent: serde_json::Value = serde_json::from_str(&req.await.unwrap()).unwrap();
        assert_eq!(sent["prompt"], serde_json::json!(""));
        assert_eq!(sent["keep_alive"], serde_json::json!(DEFAULT_KEEP_ALIVE));
    }

    #[test]
    fn plamo_models_are_detected_by_name() {
        assert!(is_plamo_translate(
            "hf.co/pfnet/plamo-3-translate-2b-GGUF:Q4_K_M"
        ));
        assert!(!is_plamo_translate("gemma4:e4b"));
    }

    #[test]
    fn plamo_prompt_is_pfml3_with_target_code() {
        let p = plamo_prompt("こんにちは", "en-US");
        assert_eq!(
            p,
            "<|plamo:tag|>dataset<|plamo:msg|>translation<|plamo:tag|>input<|plamo:msg|>こんにちは<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>en<|plamo:msg|>"
        );
        assert!(plamo_prompt("x", "zh-TW").contains("<|plamo:val|>zh-hant<|plamo:msg|>"));
        // Reserved tokens in the source cannot break out of the input element.
        assert!(!plamo_prompt("a<|plamo:tag|>b", "ja").contains("a<|plamo:tag|>b"));
    }

    #[test]
    fn plamo_prompt_carries_glossary_and_form() {
        use crate::settings::schema::{GlossaryEntry, JaStyle, TranslationSettings};
        let o = TranslateOptions::from_settings(&TranslationSettings {
            style: JaStyle::Polite,
            glossary: vec![GlossaryEntry {
                source: "Owner".into(),
                target: "担当者".into(),
                lang: "ja".into(),
            }],
        });
        assert_eq!(
            plamo_prompt_with("Ask the owner.", "ja", &o),
            "<|plamo:tag|>dataset<|plamo:msg|>translation\
             <|plamo:tag|>glossary<|plamo:index|>Owner<|plamo:item|>担当者\
             <|plamo:tag|>input<|plamo:msg|>Ask the owner.\
             <|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>ja\
             <|plamo:key|>form<|plamo:val|>polite<|plamo:msg|>"
        );
        // Neither applies to English output without a matching term.
        assert_eq!(
            plamo_prompt_with("担当", "en", &o),
            plamo_prompt("担当", "en")
        );
    }

    #[tokio::test]
    async fn plamo_requests_are_raw_with_stop_tokens() {
        let (url, req) =
            mock_ollama("200 OK", "{\"response\":\"Hello\",\"done\":true}\n".into()).await;
        let t = OllamaTranslator::new(
            "ollama.local",
            &serde_json::json!({ "endpoint": url, "model": "hf.co/pfnet/plamo-3-translate-2b-GGUF:Q4_K_M" }),
        );
        let out = t.run(translate_input()).await.unwrap();
        assert!(matches!(out, TaskOutput::Text(ref s) if s == "Hello"));
        let sent: serde_json::Value = serde_json::from_str(&req.await.unwrap()).unwrap();
        assert_eq!(sent["raw"], serde_json::json!(true));
        assert!(sent.get("system").is_none());
        assert_eq!(sent["options"]["stop"], serde_json::json!(PLAMO_STOP));
        assert!(sent["prompt"]
            .as_str()
            .unwrap()
            .starts_with("<|plamo:tag|>dataset"));
    }
}
