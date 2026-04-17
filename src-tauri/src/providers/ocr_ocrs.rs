//! ocrs-based OCR. The heavy model loading is deferred to first use so start
//! up stays fast. When the `ocrs-engine` feature is disabled this becomes an
//! inert stub that returns `LowConfidence`, allowing chains to fall through.

use async_trait::async_trait;

use super::{Capability, OcrLine, Provider, ProviderError, TaskInput, TaskOutput};

pub struct OcrsProvider {
    id: String,
}

impl OcrsProvider {
    pub fn new(id: impl Into<String>, _config: &toml::Value) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Provider for OcrsProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Ocr]
    }
    fn is_local(&self) -> bool {
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Ocr { image_png, .. } = input else {
            return Err(ProviderError::Other("OcrsProvider handles Ocr only".into()));
        };
        #[cfg(feature = "ocrs-engine")]
        {
            let _ = image_png;
            // TODO: load ocrs detection+recognition models from the model
            // directory on first call and run them here. Returning empty for
            // now so the chain can fall through to paddle/plugin OCR.
            Ok(TaskOutput::Ocr { lines: Vec::<OcrLine>::new() })
        }
        #[cfg(not(feature = "ocrs-engine"))]
        {
            let _ = image_png;
            Err(ProviderError::LowConfidence)
        }
    }
}
