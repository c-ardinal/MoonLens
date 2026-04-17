//! Reverse image search: opens a browser URL by default. SerpAPI-backed
//! in-app results are reserved for a later iteration.

use async_trait::async_trait;

use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput};

pub struct BrowserSearchProvider {
    id: String,
    url_template: String,
}

impl BrowserSearchProvider {
    pub fn new(id: impl Into<String>, config: &toml::Value) -> Self {
        Self {
            id: id.into(),
            url_template: config
                .get("url_template")
                .and_then(|v| v.as_str())
                .unwrap_or("https://www.google.com/searchbyimage?image_url={image_url}")
                .to_string(),
        }
    }
}

#[async_trait]
impl Provider for BrowserSearchProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::ReverseImageSearch]
    }
    fn is_local(&self) -> bool {
        false
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::ReverseImageSearch { image_url } = input else {
            return Err(ProviderError::Other("expected ReverseImageSearch".into()));
        };
        let url = self.url_template.replace(
            "{image_url}",
            &urlencoding_encode(&image_url),
        );
        Ok(TaskOutput::OpenedExternalUrl(url))
    }
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
