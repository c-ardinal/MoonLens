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
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapDecoder, SoftwareBitmap};
use windows::Media::Ocr::{OcrEngine, OcrResult};
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
use windows_future::IAsyncOperation;

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
        let started = std::time::Instant::now();
        tracing::info!(
            "WindowsOcr: run start (language={:?}, image {} bytes)",
            preferred,
            image_png.len()
        );
        // WinRT calls are blocking w.r.t. the Rust thread (they go
        // through COM apartments), so hop off the async runtime.
        let lines = tokio::task::spawn_blocking(move || -> Result<Vec<OcrLine>, ProviderError> {
            run_ocr(&image_png, preferred.as_deref())
        })
        .await
        .map_err(|e| ProviderError::Other(format!("join error: {e}")))?;

        match lines {
            Ok(lines) => {
                tracing::info!(
                    "WindowsOcr: run ok ({} lines, {}ms)",
                    lines.len(),
                    started.elapsed().as_millis()
                );
                if lines.is_empty() {
                    return Err(ProviderError::LowConfidence);
                }
                Ok(TaskOutput::Ocr { lines })
            }
            Err(e) => {
                tracing::warn!(
                    "WindowsOcr: run failed in {}ms: {e}",
                    started.elapsed().as_millis()
                );
                Err(e)
            }
        }
    }
}

/// Run OCR end-to-end for one PNG blob. Returns the detected lines
/// as axis-aligned bboxes in **image pixel coordinates**, matching
/// the shape `ocrs` produces so downstream code is engine-agnostic.
pub fn run_ocr(image_png: &[u8], preferred: Option<&str>) -> Result<Vec<OcrLine>, ProviderError> {
    // WinRT runtime initialisation is idempotent and cheap; calling
    // per-run is simpler than threading an init through the whole
    // provider lifecycle and the perf cost is negligible vs. the OCR
    // itself (tens of microseconds).
    let mut lap = Lap::start();
    let hr = unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        CoInitializeEx(None, COINIT_MULTITHREADED)
    };
    lap.mark(&format!("CoInitializeEx {hr:?}"));

    let engine = try_create_engine(preferred)?;
    lap.mark("engine");
    let bitmap = decode_png_to_software_bitmap(image_png, &mut lap)?;
    let op: IAsyncOperation<OcrResult> = engine
        .RecognizeAsync(&bitmap)
        .map_err(|e| ProviderError::Other(format!("RecognizeAsync: {e}")))?;
    lap.mark("RecognizeAsync started");
    let result = op
        .join()
        .map_err(|e| ProviderError::Other(format!("OCR wait: {e}")))?;
    lap.mark("RecognizeAsync done");

    let mut out: Vec<OcrLine> = Vec::new();
    let lines = result
        .Lines()
        .map_err(|e| ProviderError::Other(format!("Lines: {e}")))?;
    let line_count = lines
        .Size()
        .map_err(|e| ProviderError::Other(format!("Size: {e}")))?;
    for i in 0..line_count {
        let line = match lines.GetAt(i) {
            Ok(l) => l,
            Err(_) => continue,
        };
        let line_text = line.Text().map(|h| h.to_string_lossy()).unwrap_or_default();
        if line_text.trim().is_empty() {
            continue;
        }
        let words = match line.Words() {
            Ok(w) => w,
            Err(_) => continue,
        };
        let word_count = words.Size().unwrap_or(0);
        if word_count == 0 {
            continue;
        }

        // Collect (text, rect, class) per word so we can split the
        // visual line into contiguous same-language runs. Without
        // this, a mixed "日本語 Hello" line becomes one card that
        // covers the Japanese glyphs with a translation — hiding the
        // source text the user explicitly wanted left alone.
        let mut word_infos: Vec<(String, windows::Foundation::Rect, WordClass)> =
            Vec::with_capacity(word_count as usize);
        for wi in 0..word_count {
            if let Ok(word) = words.GetAt(wi) {
                let wt = word.Text().map(|h| h.to_string_lossy()).unwrap_or_default();
                if wt.is_empty() {
                    continue;
                }
                let Ok(rect) = word.BoundingRect() else {
                    continue;
                };
                let class = classify_word(&wt);
                word_infos.push((wt, rect, class));
            }
        }
        if word_infos.is_empty() {
            continue;
        }

        // Resolve pure-symbol words to the class of their nearest
        // non-symbol neighbour before grouping. Without this step,
        // `(日本語)` splits into three separate runs (`(` Other,
        // `日本語` Japanese, `)` Other) and the parens each spawn
        // their own micro-card; with it, the parens inherit the
        // Japanese class and tuck into the run that skips cleanly.
        resolve_symbol_classes(&mut word_infos);

        // Group contiguous words of the same language class. Each
        // group becomes its own `OcrLine` — downstream filtering
        // can then drop the Japanese runs while keeping English
        // runs for translation.
        let mut i_run = 0;
        while i_run < word_infos.len() {
            let class = word_infos[i_run].2;
            let mut j = i_run + 1;
            while j < word_infos.len() && word_infos[j].2 == class {
                j += 1;
            }
            let run = &word_infos[i_run..j];
            let texts: Vec<&str> = run.iter().map(|(t, _, _)| t.as_str()).collect();
            let text = texts.join(" ");
            if !text.trim().is_empty() {
                let mut min_x = f32::INFINITY;
                let mut min_y = f32::INFINITY;
                let mut max_x = f32::NEG_INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                for (_, rect, _) in run {
                    min_x = min_x.min(rect.X);
                    min_y = min_y.min(rect.Y);
                    max_x = max_x.max(rect.X + rect.Width);
                    max_y = max_y.max(rect.Y + rect.Height);
                }
                if min_x.is_finite() && max_x > min_x && max_y > min_y {
                    out.push(OcrLine {
                        text,
                        bbox: [min_x, min_y, max_x - min_x, max_y - min_y],
                        confidence: 1.0,
                    });
                }
            }
            i_run = j;
        }
    }
    Ok(out)
}

/// Stage timer for [`run_ocr`]: one debug line per finished stage, so a
/// stalled call shows the stage it never left.
struct Lap(std::time::Instant);

impl Lap {
    fn start() -> Self {
        Self(std::time::Instant::now())
    }
    fn mark(&mut self, stage: &str) {
        tracing::debug!(
            "WindowsOcr[{:?}]: {stage}: {} ms",
            std::thread::current().id(),
            self.0.elapsed().as_millis()
        );
        self.0 = std::time::Instant::now();
    }
}

/// Per-word language class for grouping. A run of `Japanese` words
/// stays together so the filter can drop the whole run; a run of
/// `Other` (Latin, digits) gets translated as a unit. `Symbol`
/// (pure punctuation / brackets / dashes with no letters or digits)
/// has no language on its own — it inherits the class of its
/// nearest non-Symbol neighbour, so `(日本語)` groups with the
/// Japanese run and `(English)` groups with the Latin run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordClass {
    Japanese,
    Other,
    Symbol,
}

fn classify_word(text: &str) -> WordClass {
    // Any Japanese-specific character (hiragana / katakana / kanji /
    // halfwidth katakana) wins — kanji-bearing text is always
    // Japanese regardless of surrounding punctuation.
    for c in text.chars() {
        let u = c as u32;
        if (0x3040..=0x309F).contains(&u)
            || (0x30A0..=0x30FF).contains(&u)
            || (0xFF65..=0xFF9F).contains(&u)
            || (0x3400..=0x4DBF).contains(&u)
            || (0x4E00..=0x9FFF).contains(&u)
        {
            return WordClass::Japanese;
        }
    }
    // Pure-symbol word (every char is non-alphanumeric). This covers
    // "(" ")" "「」" "。" "—" and the like. A post-processing pass
    // promotes these to the class of whatever non-Symbol word sits
    // next to them — left-first with right-fallback — so parens
    // inherit the language they contain rather than always defaulting
    // to Other and carving out stray single-char cards.
    if !text.is_empty() && text.chars().all(|c| !c.is_alphanumeric()) {
        return WordClass::Symbol;
    }
    WordClass::Other
}

/// Replace every `Symbol` entry in `words` with the class of its
/// nearest non-Symbol neighbour — left first, right as a fallback.
/// When the entire line is symbols (no language context), keep them
/// as `Other` so downstream grouping still produces a card.
fn resolve_symbol_classes(words: &mut [(String, windows::Foundation::Rect, WordClass)]) {
    for i in 0..words.len() {
        if words[i].2 != WordClass::Symbol {
            continue;
        }
        let is_word =
            |w: &&(String, windows::Foundation::Rect, WordClass)| w.2 != WordClass::Symbol;
        let resolved = words[..i]
            .iter()
            .rev()
            .find(is_word)
            .or_else(|| words[i + 1..].iter().find(is_word))
            .map(|w| w.2);
        words[i].2 = resolved.unwrap_or(WordClass::Other);
    }
}

/// Construct an `OcrEngine` for `preferred`, falling back to the
/// user-profile default when the caller asked for a language the
/// machine doesn't have a pack for. The profile-default path itself
/// can also be `None` on locked-down systems — surface that as a
/// `LowConfidence` error so the chain falls through to `ocrs`.
fn try_create_engine(preferred: Option<&str>) -> Result<OcrEngine, ProviderError> {
    // Log once per process which languages WinRT is willing to OCR.
    // Helps users diagnose "Japanese installed as input method but not
    // as language pack" — a real and silent failure mode.
    log_available_languages_once();

    if let Some(tag) = preferred {
        let lang = Language::CreateLanguage(&HSTRING::from(tag))
            .map_err(|e| ProviderError::Other(format!("Language create {tag}: {e}")))?;
        // Check availability explicitly — some WinRT builds return
        // a null COM interface from `TryCreateFromLanguage` when the
        // pack is missing, which the `if let Ok` path alone misses.
        let supported = OcrEngine::IsLanguageSupported(&lang).unwrap_or(false);
        if supported {
            if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&lang) {
                if !engine.as_raw().is_null() {
                    tracing::info!("WindowsOcr: engine created for language '{tag}'");
                    return Ok(engine);
                }
            }
        }
        tracing::warn!(
            "WindowsOcr: language '{tag}' NOT supported on this machine — \
             OCR for that language requires installing the '{tag}' language pack \
             via Settings > Time & Language > Language. Falling back to profile default."
        );
    }
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| ProviderError::Other(format!("TryCreateFromUserProfileLanguages: {e}")))?;
    if engine.as_raw().is_null() {
        tracing::warn!(
            "WindowsOcr: profile-default engine is null, no OCR language pack installed"
        );
        return Err(ProviderError::LowConfidence);
    }
    tracing::info!("WindowsOcr: engine created from profile default");
    Ok(engine)
}

/// One-time snapshot of the OCR language packs WinRT reports as
/// installed. Uses a Once latch so the log line fires only on the
/// first call per process.
fn log_available_languages_once() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| match OcrEngine::AvailableRecognizerLanguages() {
        Ok(langs) => {
            let count = langs.Size().unwrap_or(0);
            let mut names: Vec<String> = Vec::with_capacity(count as usize);
            for i in 0..count {
                if let Ok(lang) = langs.GetAt(i) {
                    if let Ok(tag) = lang.LanguageTag() {
                        names.push(tag.to_string_lossy());
                    }
                }
            }
            tracing::info!(
                "WindowsOcr: {} OCR language pack(s) available: {:?}",
                count,
                names
            );
        }
        Err(e) => {
            tracing::warn!("WindowsOcr: AvailableRecognizerLanguages failed: {e}");
        }
    });
}

/// Decode PNG bytes into a `SoftwareBitmap` in the 8-bpc BGRA
/// pixel format that `OcrEngine::RecognizeAsync` accepts.
fn decode_png_to_software_bitmap(
    png: &[u8],
    lap: &mut Lap,
) -> Result<SoftwareBitmap, ProviderError> {
    // In-memory stream so we don't touch the filesystem just to hand
    // bytes to the decoder.
    let stream = InMemoryRandomAccessStream::new()
        .map_err(|e| ProviderError::Other(format!("InMemoryRandomAccessStream: {e}")))?;
    let writer = DataWriter::CreateDataWriter(
        &stream
            .GetOutputStreamAt(0)
            .map_err(|e| ProviderError::Other(format!("GetOutputStreamAt: {e}")))?,
    )
    .map_err(|e| ProviderError::Other(format!("DataWriter::Create: {e}")))?;
    writer
        .WriteBytes(png)
        .map_err(|e| ProviderError::Other(format!("WriteBytes: {e}")))?;
    writer
        .StoreAsync()
        .and_then(|op| op.join())
        .map_err(|e| ProviderError::Other(format!("StoreAsync: {e}")))?;
    lap.mark("StoreAsync");
    writer
        .FlushAsync()
        .and_then(|op| op.join())
        .map_err(|e| ProviderError::Other(format!("FlushAsync: {e}")))?;
    lap.mark("FlushAsync");
    // Detaching the stream from the writer prevents it from closing
    // when the writer drops.
    let _ = writer.DetachStream();
    stream
        .Seek(0)
        .map_err(|e| ProviderError::Other(format!("Seek: {e}")))?;

    let decoder = BitmapDecoder::CreateAsync(&stream)
        .and_then(|op| op.join())
        .map_err(|e| ProviderError::Other(format!("BitmapDecoder::Create: {e}")))?;
    lap.mark("BitmapDecoder::CreateAsync");
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .and_then(|op| op.join())
        .map_err(|e| ProviderError::Other(format!("GetSoftwareBitmap: {e}")))?;
    lap.mark("GetSoftwareBitmapAsync");
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

    #[test]
    fn classify_word_tags_japanese_strings() {
        assert_eq!(classify_word("こんにちは"), WordClass::Japanese);
        assert_eq!(classify_word("テスト"), WordClass::Japanese);
        assert_eq!(classify_word("日本語"), WordClass::Japanese);
        assert_eq!(classify_word("ｶﾀｶﾅ"), WordClass::Japanese);
        assert_eq!(classify_word("「日本語」"), WordClass::Japanese);
    }

    #[test]
    fn classify_word_tags_latin_and_numbers_as_other() {
        assert_eq!(classify_word("Hello"), WordClass::Other);
        assert_eq!(classify_word("world"), WordClass::Other);
        assert_eq!(classify_word("123"), WordClass::Other);
        assert_eq!(classify_word("OK"), WordClass::Other);
    }

    #[test]
    fn classify_word_tags_any_japanese_char_as_japanese() {
        // Mixed single-token (unlikely from a real OCR but defensive):
        // the presence of any Japanese char classifies it as JA.
        assert_eq!(classify_word("Hi日"), WordClass::Japanese);
    }

    #[test]
    fn classify_word_tags_pure_punctuation_as_symbol() {
        assert_eq!(classify_word("("), WordClass::Symbol);
        assert_eq!(classify_word(")"), WordClass::Symbol);
        assert_eq!(classify_word("—"), WordClass::Symbol);
        assert_eq!(classify_word("!!!"), WordClass::Symbol);
        assert_eq!(classify_word("、。"), WordClass::Symbol);
        assert_eq!(classify_word("「」"), WordClass::Symbol);
    }

    #[test]
    fn classify_word_does_not_tag_alphanumeric_as_symbol() {
        // Digits / letters are Other, never Symbol.
        assert_eq!(classify_word("123"), WordClass::Other);
        assert_eq!(classify_word("A1"), WordClass::Other);
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> windows::Foundation::Rect {
        windows::Foundation::Rect {
            X: x,
            Y: y,
            Width: w,
            Height: h,
        }
    }

    #[test]
    fn resolve_symbol_inherits_from_left_neighbour() {
        let mut words = vec![
            (
                "本文ページ".into(),
                rect(0.0, 0.0, 10.0, 10.0),
                WordClass::Japanese,
            ),
            ("(".into(), rect(10.0, 0.0, 2.0, 10.0), WordClass::Symbol),
            (
                "ページ最下部".into(),
                rect(12.0, 0.0, 20.0, 10.0),
                WordClass::Japanese,
            ),
            (")".into(), rect(32.0, 0.0, 2.0, 10.0), WordClass::Symbol),
        ];
        resolve_symbol_classes(&mut words);
        assert_eq!(words[1].2, WordClass::Japanese);
        assert_eq!(words[3].2, WordClass::Japanese);
    }

    #[test]
    fn resolve_symbol_inherits_from_english_neighbour() {
        let mut words = vec![
            ("(".into(), rect(0.0, 0.0, 2.0, 10.0), WordClass::Symbol),
            ("Hello".into(), rect(2.0, 0.0, 10.0, 10.0), WordClass::Other),
            (")".into(), rect(12.0, 0.0, 2.0, 10.0), WordClass::Symbol),
        ];
        resolve_symbol_classes(&mut words);
        assert_eq!(words[0].2, WordClass::Other);
        assert_eq!(words[2].2, WordClass::Other);
    }

    #[test]
    fn resolve_symbol_prefers_left_when_both_sides_available() {
        // "English (日本語)" — left of the paren is English, right is
        // Japanese. Left-first policy keeps the paren in the Latin run,
        // so "English (" is translated but "日本語 )" is skipped.
        let mut words = vec![
            (
                "English".into(),
                rect(0.0, 0.0, 10.0, 10.0),
                WordClass::Other,
            ),
            ("(".into(), rect(10.0, 0.0, 2.0, 10.0), WordClass::Symbol),
            (
                "日本語".into(),
                rect(12.0, 0.0, 10.0, 10.0),
                WordClass::Japanese,
            ),
            (")".into(), rect(22.0, 0.0, 2.0, 10.0), WordClass::Symbol),
        ];
        resolve_symbol_classes(&mut words);
        assert_eq!(words[1].2, WordClass::Other);
        assert_eq!(words[3].2, WordClass::Japanese);
    }

    #[test]
    fn resolve_symbol_defaults_to_other_when_no_neighbour() {
        let mut words = vec![("---".into(), rect(0.0, 0.0, 10.0, 10.0), WordClass::Symbol)];
        resolve_symbol_classes(&mut words);
        assert_eq!(words[0].2, WordClass::Other);
    }
}
