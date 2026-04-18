//! Reverse image search: opens a browser URL by default. SerpAPI-backed
//! in-app results are reserved for a later iteration.

use async_trait::async_trait;

use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput};

pub struct BrowserSearchProvider {
    id: String,
    url_template: String,
}

impl BrowserSearchProvider {
    pub fn new(id: impl Into<String>, config: &serde_json::Value) -> Self {
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
            &url_encode(&image_url),
        );
        Ok(TaskOutput::OpenedExternalUrl(url))
    }
}

/// Percent-encode according to RFC 3986 unreserved characters only. Kept
/// as a top-level function so templates and tests can share it.
pub fn url_encode(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unreserved_characters_are_preserved() {
        assert_eq!(url_encode("AZaz09-_.~"), "AZaz09-_.~");
    }

    #[test]
    fn reserved_characters_are_percent_encoded() {
        assert_eq!(url_encode("https://example.com/a b"), "https%3A%2F%2Fexample.com%2Fa%20b");
    }

    #[test]
    fn multibyte_japanese_becomes_percent_sequence() {
        // "猫" is 0xE7 0x8C 0xAB in UTF-8.
        assert_eq!(url_encode("猫"), "%E7%8C%AB");
    }

    #[tokio::test]
    async fn browser_search_fills_template_and_returns_url() {
        let cfg = json!({
            "url_template": "https://search.example/?q={image_url}"
        });
        let p = BrowserSearchProvider::new("browser.example", &cfg);
        let out = p
            .run(TaskInput::ReverseImageSearch {
                image_url: "https://cdn.example/img?x=1".into(),
            })
            .await
            .expect("provider ok");
        match out {
            TaskOutput::OpenedExternalUrl(u) => {
                assert!(u.starts_with("https://search.example/?q="));
                assert!(u.contains("%3A%2F%2F"));
                assert!(u.contains("cdn.example"));
            }
            _ => panic!("expected OpenedExternalUrl"),
        }
    }
}
