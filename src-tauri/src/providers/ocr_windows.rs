//! OCR via `Windows.Media.Ocr` (WinRT).
//!
//! Windows 10 ships a native OCR engine with built-in Japanese /
//! Chinese / Korean language packs — exactly what `ocrs` lacks. When
//! the user has the relevant language pack installed, this provider
//! returns proper kana / kanji strings instead of the Latin gibberish
//! ocrs emits on CJK source. On machines without the pack,
//! `TryCreateFromLanguage` returns `None` and the chain falls through
//! to the `ocrs` fallback automatically.
//!
//! Not built on non-Windows targets — the module is gated at the
//! `providers::mod` level.

use async_trait::async_trait;
use parking_lot::RwLock;
use windows::core::{Interface, HSTRING};
use windows::Foundation::IAsyncOperation;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapDecoder, SoftwareBitmap};
use windows::Media::Ocr::{OcrEngine, OcrResult};
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

use super::{Capability, OcrLine, Provider, ProviderError, TaskInput, TaskOutput};

pub struct WindowsOcrProvider {
    id: String,
    /// Cached language hint (BCP-47 tag). `None` = engine created
    /// from the installed user profile's preferred language. Read
    /// on every `run` so a settings hot-reload picks it up without
    /// a provider rebuild.
    preferred_language: RwLock<Option<String>>,
}

impl WindowsOcrProvider {
    pub fn new(id: impl Into<String>, config: &serde_json::Value) -> Self {
        let lang = config
            .get("language")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Self {
            id: id.into(),
            preferred_language: RwLock::new(lang),
        }
    }
}

#[async_trait]
impl Provider for WindowsOcrProvider {
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
            return Err(ProviderError::Other(
                "WindowsOcrProvider handles Ocr only".into(),
            ));
        };
        let preferred = self.preferred_language.read().clone();
        // WinRT calls are blocking w.r.t. the Rust thread (they go
        // through COM apartments), so hop off the async runtime.
        let lines = tokio::task::spawn_blocking(
            move || -> Result<Vec<OcrLine>, ProviderError> {
                run_ocr(&image_png, preferred.as_deref())
            },
        )
        .await
        .map_err(|e| ProviderError::Other(format!("join error: {e}")))??;

        if lines.is_empty() {
            // Let the chain try the next provider (ocrs) instead of
            // returning an empty success — `Windows.Media.Ocr` returns
            // zero lines on some solid-color backgrounds that ocrs
            // still manages to parse.
            return Err(ProviderError::LowConfidence);
        }
        Ok(TaskOutput::Ocr { lines })
    }
}

/// Run OCR end-to-end for one PNG blob. Returns the detected lines
/// as axis-aligned bboxes in **image pixel coordinates**, matching
/// the shape `ocrs` produces so downstream code is engine-agnostic.
fn run_ocr(image_png: &[u8], preferred: Option<&str>) -> Result<Vec<OcrLine>, ProviderError> {
    // WinRT runtime initialisation is idempotent and cheap; calling
    // per-run is simpler than threading an init through the whole
    // provider lifecycle and the perf cost is negligible vs. the OCR
    // itself (tens of microseconds).
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let engine = try_create_engine(preferred)?;
    let bitmap = decode_png_to_software_bitmap(image_png)?;
    let op: IAsyncOperation<OcrResult> = engine
        .RecognizeAsync(&bitmap)
        .map_err(|e| ProviderError::Other(format!("RecognizeAsync: {e}")))?;
    let result = op
        .get()
        .map_err(|e| ProviderError::Other(format!("OCR wait: {e}")))?;

    let mut out: Vec<OcrLine> = Vec::new();
    let lines = result
        .Lines()
        .map_err(|e| ProviderError::Other(format!("Lines: {e}")))?;
    let line_count = lines
        .Size()
        .map_err(|e| ProviderError::Other(format!("Size: {e}")))? as u32;
    for i in 0..line_count {
        let line = match lines.GetAt(i) {
            Ok(l) => l,
            Err(_) => continue,
        };
        let text = line
            .Text()
            .map(|h| h.to_string_lossy())
            .unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        // Per-line bbox = union of every word's BoundingRect. The
        // OcrLine API doesn't expose a line-level rect directly, so
        // we aggregate its word rects.
        let words = match line.Words() {
            Ok(w) => w,
            Err(_) => continue,
        };
        let word_count = words.Size().unwrap_or(0) as u32;
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for wi in 0..word_count {
            if let Ok(word) = words.GetAt(wi) {
                if let Ok(rect) = word.BoundingRect() {
                    min_x = min_x.min(rect.X);
                    min_y = min_y.min(rect.Y);
                    max_x = max_x.max(rect.X + rect.Width);
                    max_y = max_y.max(rect.Y + rect.Height);
                }
            }
        }
        if min_x.is_finite() && max_x > min_x && max_y > min_y {
            out.push(OcrLine {
                text,
                bbox: [min_x, min_y, max_x - min_x, max_y - min_y],
                // WinRT does not expose per-line confidence; 1.0 keeps
                // the downstream surface happy (same as ocrs today).
                confidence: 1.0,
            });
        }
    }
    Ok(out)
}

/// Construct an `OcrEngine` for `preferred`, falling back to the
/// user-profile default when the caller asked for a language the
/// machine doesn't have a pack for. The profile-default path itself
/// can also be `None` on locked-down systems — surface that as a
/// `LowConfidence` error so the chain falls through to `ocrs`.
fn try_create_engine(preferred: Option<&str>) -> Result<OcrEngine, ProviderError> {
    if let Some(tag) = preferred {
        let lang = Language::CreateLanguage(&HSTRING::from(tag))
            .map_err(|e| ProviderError::Other(format!("Language create {tag}: {e}")))?;
        if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&lang) {
            // Some WinRT builds return a null COM interface instead
            // of an error when the language pack is missing. The
            // `.as_raw().is_null()` check covers that path too.
            if !engine.as_raw().is_null() {
                return Ok(engine);
            }
        }
        tracing::debug!(
            "WindowsOcr: language '{tag}' not available, falling back to user profile default"
        );
    }
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|e| {
        ProviderError::Other(format!(
            "TryCreateFromUserProfileLanguages: {e}"
        ))
    })?;
    if engine.as_raw().is_null() {
        return Err(ProviderError::LowConfidence);
    }
    Ok(engine)
}

/// Decode PNG bytes into a `SoftwareBitmap` in the 8-bpc BGRA
/// pixel format that `OcrEngine::RecognizeAsync` accepts.
fn decode_png_to_software_bitmap(png: &[u8]) -> Result<SoftwareBitmap, ProviderError> {
    // In-memory stream so we don't touch the filesystem just to hand
    // bytes to the decoder.
    let stream = InMemoryRandomAccessStream::new()
        .map_err(|e| ProviderError::Other(format!("InMemoryRandomAccessStream: {e}")))?;
    let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0).map_err(|e| {
        ProviderError::Other(format!("GetOutputStreamAt: {e}"))
    })?)
    .map_err(|e| ProviderError::Other(format!("DataWriter::Create: {e}")))?;
    writer
        .WriteBytes(png)
        .map_err(|e| ProviderError::Other(format!("WriteBytes: {e}")))?;
    writer
        .StoreAsync()
        .and_then(|op| op.get())
        .map_err(|e| ProviderError::Other(format!("StoreAsync: {e}")))?;
    writer
        .FlushAsync()
        .and_then(|op| op.get())
        .map_err(|e| ProviderError::Other(format!("FlushAsync: {e}")))?;
    // Detaching the stream from the writer prevents it from closing
    // when the writer drops.
    let _ = writer.DetachStream();
    stream
        .Seek(0)
        .map_err(|e| ProviderError::Other(format!("Seek: {e}")))?;

    let decoder = BitmapDecoder::CreateAsync(&stream)
        .and_then(|op| op.get())
        .map_err(|e| ProviderError::Other(format!("BitmapDecoder::Create: {e}")))?;
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .and_then(|op| op.get())
        .map_err(|e| ProviderError::Other(format!("GetSoftwareBitmap: {e}")))?;
    Ok(bitmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_picks_up_language_config() {
        let cfg = serde_json::json!({ "language": "ja" });
        let p = WindowsOcrProvider::new("windows", &cfg);
        assert_eq!(p.preferred_language.read().as_deref(), Some("ja"));
    }

    #[test]
    fn new_without_language_defaults_to_profile() {
        let p = WindowsOcrProvider::new("windows", &serde_json::Value::Null);
        assert!(p.preferred_language.read().is_none());
    }

    #[test]
    fn advertises_only_ocr_capability() {
        let p = WindowsOcrProvider::new("windows", &serde_json::Value::Null);
        assert_eq!(p.capabilities(), &[Capability::Ocr]);
        assert!(p.is_local());
    }
}

