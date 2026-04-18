//! Ollama HTTP client for local LLM translation (default: gemma4).

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput};

pub struct OllamaTranslator {
    id: String,
    endpoint: String,
    model: String,
    temperature: f32,
    top_p: f32,
    num_ctx: u32,
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
        Self {
            id: id.into(),
            endpoint: get_str("endpoint", "http://127.0.0.1:11434"),
            model: get_str("model", "gemma4:7b-instruct-q4_K_M"),
            temperature: get_f("temperature", 0.2),
            top_p: get_f("top_p", 0.9),
            num_ctx: get_u("num_ctx", 8192),
            system_prompt: get_str(
                "system_prompt",
                "You are a careful translator. Preserve meaning, tone, and formatting. Output only the translation.",
            ),
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

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: String,
    system: &'a str,
    stream: bool,
    options: Options,
}

#[derive(Serialize)]
struct Options {
    temperature: f32,
    top_p: f32,
    num_ctx: u32,
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    response: String,
    #[serde(default)]
    done: bool,
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
        let prompt = match input {
            TaskInput::Translate {
                text,
                source_language,
                target_language,
            } => format!(
                "Translate the following text from {src} to {tgt}.\n\n{txt}",
                src = source_language,
                tgt = target_language,
                txt = text
            ),
            TaskInput::Explain { prompt, .. } => prompt,
            TaskInput::Summarize { text } => format!("Summarize concisely:\n\n{text}"),
            _ => {
                return Err(ProviderError::Other(
                    "OllamaTranslator cannot handle this input".into(),
                ))
            }
        };

        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));
        let stream = true;
        let req = GenerateRequest {
            model: &self.model,
            prompt,
            system: &self.system_prompt,
            stream,
            options: Options {
                temperature: self.temperature,
                top_p: self.top_p,
                num_ctx: self.num_ctx,
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

        // Parse line-delimited JSON chunks; each one contains `response`
        // plus a `done` flag. The sink fires per chunk so the UI can
        // render tokens live, while we aggregate into a single Text
        // output so existing call sites keep working unchanged.
        let mut aggregated = String::new();
        let mut buffer = Vec::<u8>::new();
        let mut bytes = resp.bytes_stream();
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(|e| ProviderError::Other(e.to_string()))?;
            buffer.extend_from_slice(&chunk);
            // Flush every complete line (newline-delimited JSON).
            while let Some(nl) = buffer.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buffer.drain(..=nl).collect();
                let line_str = match std::str::from_utf8(&line) {
                    Ok(s) => s.trim().to_string(),
                    Err(_) => continue,
                };
                if line_str.is_empty() {
                    continue;
                }
                if let Ok(c) = serde_json::from_str::<StreamChunk>(&line_str) {
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
            }
        }
        // Any trailing bytes without a newline (rare) can still parse.
        if !buffer.is_empty() {
            if let Ok(line) = std::str::from_utf8(&buffer) {
                if let Ok(c) = serde_json::from_str::<StreamChunk>(line.trim()) {
                    aggregated.push_str(&c.response);
                }
            }
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
}
