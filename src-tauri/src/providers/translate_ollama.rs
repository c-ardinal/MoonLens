//! Ollama HTTP client for local LLM translation (default: gemma4).

use async_trait::async_trait;
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
        let get_f = |k: &str, d: f64| {
            config.get(k).and_then(|v| v.as_f64()).unwrap_or(d) as f32
        };
        let get_u = |k: &str, d: u64| {
            config.get(k).and_then(|v| v.as_u64()).unwrap_or(d) as u32
        };
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
        }
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
struct GenerateResponse {
    response: String,
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
        let req = GenerateRequest {
            model: &self.model,
            prompt,
            system: &self.system_prompt,
            stream: false,
            options: Options {
                temperature: self.temperature,
                top_p: self.top_p,
                num_ctx: self.num_ctx,
            },
        };

        let resp = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout(0)
                } else if e.is_connect() {
                    ProviderError::Offline
                } else {
                    ProviderError::Other(e.to_string())
                }
            })?;

        let body: GenerateResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        Ok(TaskOutput::Text(body.response))
    }
}
