//! DeepL API fallback translator.

use async_trait::async_trait;
use serde::Deserialize;

use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput};

pub struct DeepLTranslator {
    id: String,
    endpoint: String,
    api_key: Option<String>,
    formality: String,
    http: reqwest::Client,
}

impl DeepLTranslator {
    pub fn new(id: impl Into<String>, config: &toml::Value) -> Self {
        let key_env = config
            .get("api_key_env")
            .and_then(|v| v.as_str())
            .unwrap_or("DEEPL_API_KEY");
        let endpoint = config
            .get("endpoint")
            .and_then(|v| v.as_str())
            .unwrap_or("https://api-free.deepl.com/v2/translate")
            .to_string();
        let formality = config
            .get("formality")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();
        Self {
            id: id.into(),
            endpoint,
            api_key: std::env::var(key_env).ok(),
            formality,
            http: reqwest::Client::new(),
        }
    }
}

#[derive(Deserialize)]
struct Response {
    translations: Vec<Translation>,
}
#[derive(Deserialize)]
struct Translation {
    text: String,
}

#[async_trait]
impl Provider for DeepLTranslator {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Translate]
    }
    fn is_local(&self) -> bool {
        false
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Translate {
            text,
            target_language,
            ..
        } = input
        else {
            return Err(ProviderError::Other("DeepL handles Translate only".into()));
        };
        let key = self
            .api_key
            .as_ref()
            .ok_or_else(|| ProviderError::Other("DEEPL_API_KEY not set".into()))?;

        let params = [
            ("text", text.as_str()),
            ("target_lang", target_language.as_str()),
            ("formality", self.formality.as_str()),
        ];

        let resp = self
            .http
            .post(&self.endpoint)
            .header("Authorization", format!("DeepL-Auth-Key {key}"))
            .form(&params)
            .send()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        let body: Response = resp
            .json()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        let out = body
            .translations
            .into_iter()
            .next()
            .map(|t| t.text)
            .unwrap_or_default();
        Ok(TaskOutput::Text(out))
    }
}
