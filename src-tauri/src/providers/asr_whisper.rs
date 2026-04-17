//! whisper.cpp server ASR provider. The actual server is expected to run as
//! a sidecar; this module only speaks to its HTTP API.

use async_trait::async_trait;
use serde::Deserialize;

use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput, TranscriptSegment};

pub struct WhisperCppProvider {
    id: String,
    endpoint: String,
    http: reqwest::Client,
}

impl WhisperCppProvider {
    pub fn new(id: impl Into<String>, config: &toml::Value) -> Self {
        Self {
            id: id.into(),
            endpoint: config
                .get("endpoint")
                .and_then(|v| v.as_str())
                .unwrap_or("http://127.0.0.1:9002")
                .to_string(),
            http: reqwest::Client::new(),
        }
    }
}

#[derive(Deserialize)]
struct ApiResponse {
    segments: Vec<Seg>,
}
#[derive(Deserialize)]
struct Seg {
    start: f32,
    end: f32,
    text: String,
}

#[async_trait]
impl Provider for WhisperCppProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Asr]
    }
    fn is_local(&self) -> bool {
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Asr {
            audio_pcm16_mono_16k,
            language_hint,
        } = input
        else {
            return Err(ProviderError::Other("expected Asr input".into()));
        };
        let url = format!("{}/inference", self.endpoint.trim_end_matches('/'));
        let mut req = self.http.post(&url).body(audio_pcm16_mono_16k);
        if let Some(lang) = language_hint {
            req = req.query(&[("language", lang)]);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    ProviderError::Offline
                } else {
                    ProviderError::Other(e.to_string())
                }
            })?;
        let body: ApiResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        let segments = body
            .segments
            .into_iter()
            .map(|s| TranscriptSegment {
                start_s: s.start,
                end_s: s.end,
                text: s.text,
            })
            .collect();
        Ok(TaskOutput::Transcript { segments })
    }
}
