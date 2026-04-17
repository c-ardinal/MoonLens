//! Gemini API fallback translator / explainer.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput};

pub struct GeminiProvider {
    id: String,
    model: String,
    api_key: Option<String>,
    temperature: f32,
    http: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(id: impl Into<String>, config: &serde_json::Value) -> Self {
        let key_env = config
            .get("api_key_env")
            .and_then(|v| v.as_str())
            .unwrap_or("GEMINI_API_KEY");
        Self {
            id: id.into(),
            model: config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("gemini-2.5-flash")
                .to_string(),
            api_key: std::env::var(key_env).ok(),
            temperature: config
                .get("temperature")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.2) as f32,
            http: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct Request {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}
#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}
#[derive(Serialize)]
struct Part {
    text: String,
}
#[derive(Serialize)]
struct GenerationConfig {
    temperature: f32,
}
#[derive(Deserialize)]
struct Response {
    candidates: Option<Vec<Candidate>>,
}
#[derive(Deserialize)]
struct Candidate {
    content: Option<RespContent>,
}
#[derive(Deserialize)]
struct RespContent {
    parts: Option<Vec<RespPart>>,
}
#[derive(Deserialize)]
struct RespPart {
    text: Option<String>,
}

#[async_trait]
impl Provider for GeminiProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Translate, Capability::Explain, Capability::Summarize]
    }
    fn is_local(&self) -> bool {
        false
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let key = self
            .api_key
            .as_ref()
            .ok_or_else(|| ProviderError::Other("GEMINI_API_KEY not set".into()))?;

        let prompt = match input {
            TaskInput::Translate {
                text,
                source_language,
                target_language,
            } => format!("Translate from {source_language} to {target_language}:\n{text}"),
            TaskInput::Explain { prompt, .. } => prompt,
            TaskInput::Summarize { text } => format!("Summarize concisely:\n{text}"),
            _ => return Err(ProviderError::Other("unsupported input".into())),
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, key
        );
        let req = Request {
            contents: vec![Content {
                parts: vec![Part { text: prompt }],
            }],
            generation_config: GenerationConfig {
                temperature: self.temperature,
            },
        };
        let resp = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        let body: Response = resp
            .json()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        let out = body
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .and_then(|p| p.into_iter().next())
            .and_then(|p| p.text)
            .unwrap_or_default();
        Ok(TaskOutput::Text(out))
    }
}
