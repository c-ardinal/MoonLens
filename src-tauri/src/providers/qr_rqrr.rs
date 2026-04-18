//! QR decoder using `rqrr`.

use async_trait::async_trait;
use image::GenericImageView;

use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput};

pub struct RqrrProvider {
    id: String,
}

impl RqrrProvider {
    pub fn new(id: impl Into<String>, _config: &serde_json::Value) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Provider for RqrrProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::QrHandler]
    }
    fn is_local(&self) -> bool {
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::IntentClassifier { image_png } = input else {
            return Err(ProviderError::Other("expected image".into()));
        };
        let img =
            image::load_from_memory(&image_png).map_err(|e| ProviderError::Other(e.to_string()))?;
        let (w, h) = img.dimensions();
        let luma = img.to_luma8();
        let mut prepared =
            rqrr::PreparedImage::prepare_from_greyscale(w as usize, h as usize, |x, y| {
                luma.get_pixel(x as u32, y as u32)[0]
            });
        let grids = prepared.detect_grids();
        if let Some(grid) = grids.first() {
            let (_meta, content) = grid
                .decode()
                .map_err(|e| ProviderError::Other(e.to_string()))?;
            return Ok(TaskOutput::Text(content));
        }
        Err(ProviderError::LowConfidence)
    }
}
