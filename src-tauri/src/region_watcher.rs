//! Change-driven reading of a screen region (continuous window
//! translation).
//!
//! A window-translation window (`lens`) set to continuous reading feeds
//! its region to a [`Tracker`] at every tick
//! (`performance.region_watcher_poll_ms`):
//!
//! 1. Capture the region and reduce it to a small greyscale thumbnail.
//!    Unchanged pixels cost nothing more.
//! 2. When pixels change, translations whose source area changed are
//!    hidden at once ([`LiveEvent::Stale`]), so an outdated translation
//!    never sits over new text while OCR runs.
//! 3. OCR the region. When the text is what is already shown (a video
//!    behind a subtitle, a blinking cursor), nothing is re-translated.
//!    New text is translated once it reads the same on two consecutive
//!    ticks, so text that is still being typed out is not translated
//!    half-way; text that keeps changing is translated after
//!    [`SETTLE_LIMIT`] ticks anyway. When the second tick captures
//!    exactly the same picture the first reading stands: OCR is not
//!    run a second time on the same pixels.
//! 4. Blocks seen before come from the translation cache and are shown
//!    immediately; only new paragraphs go to the model. A long text is
//!    translated in parts, each shown as soon as it is ready.
//!
//! Steps 3 and 4 depend on the window's [`ReadMode`]: subtitles wait
//! for a line to be typed out in full and are translated with the lines
//! before them; interface text is translated reading by reading.
//!
//! The capture loop and the window live in `lens`.

use image::{DynamicImage, GrayImage};
use serde::{Deserialize, Serialize};

use crate::{
    capture,
    pipeline::{CaptureFlowResult, LensBlock, Pipeline, TranslationProgress},
    providers::{Capability, TaskOutput},
    translate_plan, user_error,
};

/// Longest side of the thumbnail used for change detection.
const THUMB_MAX_SIDE: u32 = 192;

/// Luma difference above which a thumbnail pixel counts as changed.
const PIXEL_DELTA: u8 = 24;

/// Share of changed thumbnail pixels for the region to count as changed.
/// Small enough for a one-word change in a subtitle line, large enough to
/// ignore compression noise in video.
const REGION_CHANGED: f32 = 0.002;

/// Share of changed pixels inside a block's box that hides its
/// translation before OCR confirms the change.
const BLOCK_CHANGED: f32 = 0.12;

/// Share of changed pixels inside a block's box above which the picture
/// behind its text counts as moving (a video): the text-only erase is a
/// still picture, so such a block goes back to the one-colour box.
const ERASE_MOVED: f32 = 0.02;

/// Ticks a changing text may take to settle before it is translated
/// anyway (a clock, a counter).
const SETTLE_LIMIT: u32 = 6;

/// Ticks a subtitle that is being typed out may keep growing before it
/// is translated anyway (text that scrolls without an end).
const GROW_LIMIT: u32 = 60;

/// Characters at the end of a reading that may differ in the next one
/// for it to still count as the same line growing: the last glyphs of a
/// line being typed out are often read half-drawn.
const GROW_SLACK: usize = 2;

/// Earlier readings given to the translator with a subtitle, in
/// [`translate_plan::cost`] units: a few lines of dialogue. Small, so
/// the prompt stays short and an old scene does not colour a new one.
const CARRY: usize = 800;

/// How a window-translation window reads its region.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadMode {
    /// Running text (subtitles, dialogue): a line that is typed out is
    /// translated once it is complete, with the lines read before it
    /// as context.
    #[default]
    Subtitle,
    /// Interface text (menus, labels): every reading is translated by
    /// itself, nothing is carried from one to the next, and text that
    /// keeps changing is not waited for beyond [`SETTLE_LIMIT`].
    Ui,
}

/// Messages for a window-translation window.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveEvent {
    /// Blocks to paint, positioned in the window's physical pixels.
    /// `translating` is true while new paragraphs are still with the
    /// model (blocks without a translation are drawn as pending).
    Blocks {
        blocks: Vec<LensBlock>,
        translating: bool,
        /// Engine that translated the new blocks, once known.
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
        /// The new translations came from a cloud provider.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        via_cloud: bool,
    },
    /// Hide these blocks (indices into the last `Blocks`): their source
    /// changed and a new reading is on its way.
    Stale { indices: Vec<usize> },
    /// Nothing to show (the region is empty).
    Clear,
    /// Text recognition of a changed region started (`on`) or ended; the
    /// window shows that it is working.
    Reading { on: bool },
    /// A QR code in the region was decoded; the window shows `text`.
    Decoded { text: String },
    /// Reading or translating failed (`code|message`, see `user_error`).
    /// The window shows it.
    Error { message: String },
}

/// Region geometry in screen pixels (`[x, y, w, h]`). A different value
/// means the region moved or was resized.
pub type Geometry = [i32; 4];

/// One reading of a watched region.
pub struct Frame {
    pub image: DynamicImage,
    pub geometry: Geometry,
}

/// What the caller supplies to [`Tracker::tick`].
pub struct TickEnv<'a> {
    pub pipeline: &'a Pipeline,
    pub target_language: &'a str,
    pub mode: ReadMode,
    /// History kind of new translations (`pipeline::history_kind`).
    pub history_kind: &'a str,
    pub emit: &'a (dyn Fn(LiveEvent) + Send + Sync),
    /// Region geometry now. A translation finished after the region moved
    /// is not shown at the old place; the next tick shows it at the new.
    pub geometry_now: &'a (dyn Fn() -> Option<Geometry> + Send + Sync),
}

/// Result of one [`Tracker::tick`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tick {
    /// Pixels unchanged: no OCR, no translation.
    Unchanged,
    /// The region moved or was resized: shown blocks hidden, reading
    /// waits until it holds still.
    Moved,
    /// New text read once; waiting for the same reading again.
    Settling,
    /// Same text as shown (only pixels around it changed): translations
    /// kept, nothing sent to the model.
    SameText,
    /// No text in the region.
    Empty,
    /// A QR code was decoded (no text blocks).
    Decoded { text: String },
    /// New text shown. `requested` is false when every block came from
    /// the cache or is already in the target language.
    Translated {
        source: String,
        translation: String,
        requested: bool,
    },
}

/// What the tracker has on screen.
#[derive(Default)]
struct Shown {
    thumb: Option<Thumb>,
    /// Target language and source text of the shown blocks (or the
    /// decoded QR payload): what a new reading is compared with.
    text: Option<String>,
    blocks: Vec<LensBlock>,
    /// Indices already reported stale since the last `Blocks`.
    stale: Vec<usize>,
    /// The frame the shown erase pictures were made from. Unlike
    /// `thumb` it does not follow ticks that emit nothing, so slow
    /// motion behind the text adds up against it.
    erase_base: Option<Thumb>,
}

/// Change-driven reading of one screen region, shared by live
/// translation and Lens windows. Feed it a [`Frame`] per tick; it runs
/// OCR only when pixels change and translates only text not shown yet
/// (see the module docs for the steps).
#[derive(Default)]
pub struct Tracker {
    shown: Shown,
    /// Last frame read.
    last_thumb: Option<Thumb>,
    last_geometry: Option<Geometry>,
    /// The reading that waits for the text to settle.
    pending: Option<Pending>,
    unsettled: u32,
    /// Ticks the pending reading has been growing ([`ReadMode::Subtitle`]).
    growing: u32,
    /// Mode and target language of the last tick: a change drops
    /// `carried` (its translations are in the old language).
    carried_for: (ReadMode, String),
    /// Source and translation of earlier readings, oldest first: the
    /// context of the next subtitle. Empty in [`ReadMode::Ui`].
    carried: Vec<(String, String)>,
}

/// A reading of new text, kept until the next tick confirms it.
struct Pending {
    /// Target language and text read (see `key` in [`Tracker::tick`]).
    key: String,
    /// [`fingerprint`] of the picture that was read.
    picture: u64,
    /// What OCR returned for it. Used as the next tick's reading when
    /// that tick captures the very same picture, instead of running
    /// OCR again.
    bundle: CaptureFlowResult,
}

/// Identifies a capture pixel for pixel. The thumbnail comparison is
/// approximate on purpose (video noise must not count as a change), so
/// it cannot tell whether OCR would read the same again: one more
/// character typed into a large region stays below its threshold.
fn fingerprint(image: &DynamicImage) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (image.width(), image.height(), image.color()).hash(&mut h);
    image.as_bytes().hash(&mut h);
    h.finish()
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// The region moved or was resized: hide every shown block (their
    /// boxes no longer line up) and wait for it to hold still.
    pub fn region_moved(&mut self, geometry: Geometry, env: &TickEnv<'_>) {
        self.last_geometry = Some(geometry);
        self.last_thumb = None;
        self.pending = None;
        self.unsettled = 0;
        self.growing = 0;
        // Another place may show another text.
        self.carried.clear();
        let newly: Vec<usize> = (0..self.shown.blocks.len())
            .filter(|i| !self.shown.stale.contains(i))
            .collect();
        if !newly.is_empty() {
            self.shown.stale.extend(&newly);
            (env.emit)(LiveEvent::Stale { indices: newly });
        }
    }

    /// Process one reading. `force` reads and shows the region even when
    /// nothing changed and skips the wait for text to settle (a manual
    /// refresh).
    pub async fn tick(
        &mut self,
        frame: Frame,
        force: bool,
        env: &TickEnv<'_>,
    ) -> Result<Tick, String> {
        if self.last_geometry.is_some_and(|g| g != frame.geometry) {
            self.region_moved(frame.geometry, env);
            return Ok(Tick::Moved);
        }
        self.last_geometry = Some(frame.geometry);
        if self.carried_for.0 != env.mode || self.carried_for.1 != env.target_language {
            self.carried_for = (env.mode, env.target_language.to_string());
            self.carried.clear();
        }
        let thumb = Thumb::of(&frame.image);
        let previous = self.last_thumb.replace(thumb.clone());
        let changed = previous
            .as_ref()
            .is_none_or(|t| t.changed_share(&thumb, None) >= REGION_CHANGED);
        // Nothing changed since the last tick and nothing waits to settle.
        // The target language changed in settings: read and translate again.
        let language_changed = self
            .shown
            .text
            .as_deref()
            .is_some_and(|k| k.split('\u{1}').next() != Some(env.target_language));
        if !changed
            && !force
            && !language_changed
            && self.pending.is_none()
            && self.shown.stale.is_empty()
        {
            return Ok(Tick::Unchanged);
        }

        // Hide translations whose source area changed, before OCR.
        if let Some(base) = &self.shown.thumb {
            let newly: Vec<usize> = self
                .shown
                .blocks
                .iter()
                .enumerate()
                .filter(|(i, b)| {
                    !self.shown.stale.contains(i)
                        && base.changed_share(&thumb, Some(b.bbox)) >= BLOCK_CHANGED
                })
                .map(|(i, _)| i)
                .collect();
            if !newly.is_empty() {
                self.shown.stale.extend(&newly);
                (env.emit)(LiveEvent::Stale { indices: newly });
            }
        }

        let picture = fingerprint(&frame.image);
        let (mut bundle, pending_key) = match self.pending.take() {
            // The very picture whose reading waits to settle: OCR would
            // return the same again, so that reading stands.
            Some(p) if p.picture == picture && !force => (p.bundle, Some(p.key)),
            waiting => {
                let png = capture::png_bytes(&frame.image).map_err(|e| e.to_string())?;
                (env.emit)(LiveEvent::Reading { on: true });
                let analyzed = env.pipeline.analyze_capture(png).await;
                (env.emit)(LiveEvent::Reading { on: false });
                match analyzed {
                    Ok(bundle) => (bundle, waiting.map(|p| p.key)),
                    Err(e) => {
                        // Still unread: the next tick tries again.
                        self.pending = waiting;
                        return Err(user_error::provider(&e, Capability::Ocr));
                    }
                }
            }
        };
        // The region moved while OCR ran: these boxes belong to the old
        // place. Hide what is shown and read again once it holds still.
        if let Some(now) = (env.geometry_now)().filter(|g| *g != frame.geometry) {
            self.region_moved(now, env);
            return Ok(Tick::Moved);
        }
        let text = source_key(&bundle.blocks);
        // A decoded QR code has no blocks; its payload is the reading.
        let decoded = match (&bundle.result.output, bundle.blocks.is_empty()) {
            (TaskOutput::Text(t), true) if !t.trim().is_empty() => Some(t.clone()),
            _ => None,
        };
        // What decides "same as shown": the text read, and the language
        // it is shown in (a changed target language translates again).
        let key = format!(
            "{}\u{1}{}",
            env.target_language,
            decoded.as_deref().map_or(text.as_str(), |d| d)
        );

        // A refresh retries blocks left untranslated (a failed request).
        let incomplete = self
            .shown
            .blocks
            .iter()
            .any(|b| !b.same_language && b.translation.is_none());
        if self.shown.text.as_deref() == Some(key.as_str()) && !(force && incomplete) {
            // Same text as on screen (only the background changed, or
            // the region moved over the same text): show what was hidden
            // again, at the new positions.
            self.unsettled = 0;
            self.growing = 0;
            if let (Some(text), true) = (&decoded, force) {
                self.shown.thumb = Some(thumb);
                (env.emit)(LiveEvent::Decoded { text: text.clone() });
                return Ok(Tick::SameText);
            }
            let mut blocks = carry_translations(&self.shown.blocks, bundle.blocks);
            // The picture behind the text moves: no still erase picture
            // over it, now or while this text stays. A refresh tries anew.
            if !force {
                keep_plain(&self.shown.blocks, &mut blocks);
                drop_moving_erase(&mut blocks, self.shown.erase_base.as_ref(), &thumb);
            }
            let erase_lost = erase_count(&blocks) < erase_count(&self.shown.blocks);
            if !self.shown.stale.is_empty() || force || erase_lost {
                self.shown.stale.clear();
                self.shown.erase_base = Some(thumb.clone());
                self.shown.blocks = blocks.clone();
                (env.emit)(LiveEvent::Blocks {
                    blocks,
                    translating: false,
                    provider: None,
                    via_cloud: false,
                });
            }
            self.shown.thumb = Some(thumb);
            return Ok(Tick::SameText);
        }

        // New text: wait for it to read the same twice (text being typed
        // out), unless it has been changing for too long.
        let settled = force || pending_key.as_deref() == Some(key.as_str());
        // A subtitle that is still being typed out: every reading holds
        // more of the same line. It is not text that never settles, so
        // it gets the time it needs.
        let grows = env.mode == ReadMode::Subtitle
            && self.growing < GROW_LIMIT
            && pending_key.as_deref().is_some_and(|p| key_extends(p, &key));
        if !settled && (grows || self.unsettled < SETTLE_LIMIT) {
            self.pending = Some(Pending {
                key,
                picture,
                bundle,
            });
            if grows {
                self.growing += 1;
            } else {
                self.unsettled += 1;
            }
            return Ok(Tick::Settling);
        }
        self.unsettled = 0;
        self.growing = 0;

        if bundle.blocks.is_empty() {
            self.shown = Shown {
                thumb: Some(thumb),
                text: Some(key),
                ..Shown::default()
            };
            if let Some(text) = decoded {
                // Take earlier boxes off before the decoded text.
                (env.emit)(LiveEvent::Clear);
                (env.emit)(LiveEvent::Decoded { text: text.clone() });
                return Ok(Tick::Decoded { text });
            }
            (env.emit)(LiveEvent::Clear);
            return Ok(Tick::Empty);
        }

        // The text-only erase is a still picture: only for blocks whose
        // area held still since the previous tick. A refresh shows it
        // at once; the following ticks take it back if the picture moves.
        if !force {
            drop_moving_erase(&mut bundle.blocks, previous.as_ref(), &thumb);
        }

        // Cached and same-language blocks are ready now; show them while
        // the rest is translated.
        env.pipeline
            .prepare_translation(&mut bundle, env.target_language);
        let requested =
            bundle.needs_translation() && bundle.blocks.iter().any(|b| b.needs_request());
        self.shown = Shown {
            erase_base: Some(thumb.clone()),
            thumb: Some(thumb),
            text: Some(key),
            blocks: bundle.blocks.clone(),
            stale: Vec::new(),
        };
        (env.emit)(LiveEvent::Blocks {
            blocks: bundle.blocks.clone(),
            translating: requested,
            provider: None,
            via_cloud: false,
        });
        if requested {
            // A long text is translated in parts: show each part as it
            // is done instead of all of them after the last.
            let ready =
                |blocks: &[LensBlock]| blocks.iter().filter(|b| b.translation.is_some()).count();
            let shown = std::sync::atomic::AtomicUsize::new(ready(&bundle.blocks));
            let progress = |p: &TranslationProgress| {
                let now = ready(&p.blocks);
                let more = shown.swap(now, std::sync::atomic::Ordering::Relaxed) < now;
                if more && (env.geometry_now)() == Some(frame.geometry) {
                    (env.emit)(LiveEvent::Blocks {
                        blocks: p.blocks.clone(),
                        translating: true,
                        provider: None,
                        via_cloud: false,
                    });
                }
            };
            // Empty in `ReadMode::Ui`. A reading that the new one
            // continues (or repeats) is the same line: as history it
            // would be translated twice.
            let earlier = match self.carried.split_last() {
                Some(((last, _), before)) if *last == text || extends(last, &text) => before,
                _ => &self.carried[..],
            };
            let result = env
                .pipeline
                .translate_capture_after(
                    &mut bundle,
                    env.target_language,
                    Some(&progress),
                    env.history_kind,
                    earlier,
                )
                .await;
            self.shown.blocks = bundle.blocks.clone();
            if let Err(e) = result {
                // Keep what is readable; the text is tried again once it
                // changes or on a manual refresh.
                self.shown.text = None;
                (env.emit)(LiveEvent::Blocks {
                    blocks: bundle.blocks,
                    translating: false,
                    provider: None,
                    via_cloud: false,
                });
                return Err(user_error::provider(&e, Capability::Translate));
            }
            // The region moved while the model ran: the boxes belong to
            // the old place. The next tick shows them at the new one.
            if (env.geometry_now)() == Some(frame.geometry) {
                (env.emit)(LiveEvent::Blocks {
                    blocks: bundle.blocks.clone(),
                    translating: false,
                    provider: Some(bundle.result.used_provider.clone()),
                    via_cloud: bundle.result.via_cloud,
                });
            }
        }
        let translation = bundle
            .blocks
            .iter()
            .map(|b| {
                b.translation
                    .as_deref()
                    .unwrap_or(if b.same_language { &b.source } else { "" })
                    .trim()
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if env.mode == ReadMode::Subtitle {
            self.carry(&text, &translation);
        }
        Ok(Tick::Translated {
            source: text,
            translation,
            requested,
        })
    }
}

impl Tracker {
    /// Keep a shown reading as context of the next ones.
    fn carry(&mut self, source: &str, translation: &str) {
        if source.trim().is_empty() || translation.trim().is_empty() {
            return;
        }
        // The line shown before, now complete: only the whole line stays.
        if self
            .carried
            .last()
            .is_some_and(|(s, _)| s == source || extends(s, source))
        {
            self.carried.pop();
        }
        self.carried.push((source.into(), translation.into()));
        translate_plan::trim_history(&mut self.carried, CARRY);
    }
}

/// Whether `new` is `old` with more text after it (a line being typed
/// out), allowing for [`GROW_SLACK`] misread characters at the end of
/// `old`.
fn extends(old: &str, new: &str) -> bool {
    let old_len = old.chars().count();
    let same = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .count();
    old_len > GROW_SLACK && same + GROW_SLACK >= old_len && new.chars().count() > old_len
}

/// [`extends`] for two reading keys (target language and text, see
/// `key` in [`Tracker::tick`]) of the same language.
fn key_extends(old: &str, new: &str) -> bool {
    match (old.split_once('\u{1}'), new.split_once('\u{1}')) {
        (Some((old_lang, old)), Some((new_lang, new))) => old_lang == new_lang && extends(old, new),
        _ => false,
    }
}

/// Text of all blocks in order: what decides whether the screen says
/// something new.
fn source_key(blocks: &[LensBlock]) -> String {
    blocks
        .iter()
        .map(|b| b.source.trim())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Take the erase picture off every block whose area differs between
/// `base` and `now` (or when there is nothing to compare with).
fn drop_moving_erase(blocks: &mut [LensBlock], base: Option<&Thumb>, now: &Thumb) {
    for b in blocks.iter_mut().filter(|b| b.erase.is_some()) {
        if base.is_none_or(|t| t.changed_share(now, Some(b.bbox)) >= ERASE_MOVED) {
            b.erase = None;
        }
    }
}

/// Blocks shown with the one-colour box keep it in a new reading of the
/// same text: the box and the erase picture do not swap back and forth.
fn keep_plain(old: &[LensBlock], new: &mut [LensBlock]) {
    for b in new.iter_mut() {
        if old
            .iter()
            .any(|o| o.source == b.source && o.erase.is_none())
        {
            b.erase = None;
        }
    }
}

fn erase_count(blocks: &[LensBlock]) -> usize {
    blocks.iter().filter(|b| b.erase.is_some()).count()
}

/// New readings of the same text: keep the translations, take the new
/// positions and colours.
fn carry_translations(old: &[LensBlock], new: Vec<LensBlock>) -> Vec<LensBlock> {
    new.into_iter()
        .map(|mut b| {
            if let Some(o) = old.iter().find(|o| o.source == b.source) {
                b.translation = o.translation.clone();
                b.same_language = o.same_language;
                b.from_cache = o.from_cache;
            }
            b
        })
        .collect()
}

/// Small greyscale copy of a capture for change detection.
#[derive(Clone)]
struct Thumb {
    img: GrayImage,
    /// Thumbnail pixels per capture pixel.
    scale: f32,
}

impl Thumb {
    fn of(image: &DynamicImage) -> Self {
        let (w, h) = (image.width().max(1), image.height().max(1));
        let scale = (THUMB_MAX_SIDE as f32 / w.max(h) as f32).min(1.0);
        let tw = ((w as f32 * scale).round() as u32).max(1);
        let th = ((h as f32 * scale).round() as u32).max(1);
        let img = image
            .resize_exact(tw, th, image::imageops::FilterType::Triangle)
            .to_luma8();
        Self { img, scale }
    }

    /// Share of pixels that differ from `other`, over the whole image or
    /// inside `bbox` (`[x, y, w, h]` in capture pixels). A different size
    /// (the region was resized) counts as fully changed.
    fn changed_share(&self, other: &Thumb, bbox: Option<[f32; 4]>) -> f32 {
        if self.img.dimensions() != other.img.dimensions() {
            return 1.0;
        }
        let (w, h) = self.img.dimensions();
        let (x0, y0, x1, y1) = match bbox {
            None => (0, 0, w, h),
            Some([x, y, bw, bh]) => {
                let s = self.scale;
                let x0 = ((x * s).floor().max(0.0) as u32).min(w);
                let y0 = ((y * s).floor().max(0.0) as u32).min(h);
                let x1 = (((x + bw) * s).ceil().max(0.0) as u32).clamp(x0, w);
                let y1 = (((y + bh) * s).ceil().max(0.0) as u32).clamp(y0, h);
                (x0, y0, x1, y1)
            }
        };
        let total = (x1 - x0) * (y1 - y0);
        if total == 0 {
            return 0.0;
        }
        let mut changed = 0u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let a = self.img.get_pixel(x, y)[0];
                let b = other.img.get_pixel(x, y)[0];
                if a.abs_diff(b) > PIXEL_DELTA {
                    changed += 1;
                }
            }
        }
        changed as f32 / total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn solid(w: u32, h: u32, v: u8) -> DynamicImage {
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(w, h, Rgba([v, v, v, 255])))
    }

    /// White image with a dark "word" drawn at `x`.
    fn with_word(x: u32) -> DynamicImage {
        let mut img = ImageBuffer::from_pixel(800, 60, Rgba([255, 255, 255, 255]));
        for yy in 20..40 {
            for xx in x..x + 60 {
                img.put_pixel(xx, yy, Rgba([0, 0, 0, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    fn block(source: &str, bbox: [f32; 4]) -> LensBlock {
        LensBlock {
            bbox,
            line_height: 20.0,
            source: source.into(),
            translation: None,
            colors: None,
            same_language: false,
            from_cache: false,
            align: Default::default(),
            bold: false,
            lines: vec![],
            row_limits: vec![],
            erase: None,
        }
    }

    fn erased(source: &str, bbox: [f32; 4]) -> LensBlock {
        LensBlock {
            erase: Some(crate::erase::ErasePatch {
                rect: [0, 0, 1, 1],
                image: String::new(),
                outline: false,
                foreground: [0, 0, 0],
            }),
            ..block(source, bbox)
        }
    }

    #[test]
    fn erase_picture_stays_while_the_picture_behind_holds_still() {
        let frame = Thumb::of(&with_word(100));
        let mut blocks = vec![erased("a", [80.0, 10.0, 400.0, 40.0])];
        drop_moving_erase(&mut blocks, Some(&frame), &Thumb::of(&with_word(100)));
        assert!(blocks[0].erase.is_some());
    }

    #[test]
    fn erase_picture_goes_when_the_picture_behind_moves() {
        let before = Thumb::of(&with_word(100));
        let after = Thumb::of(&with_word(400));
        // The block where the word was is dropped, the one elsewhere stays.
        let mut blocks = vec![
            erased("moved", [80.0, 10.0, 400.0, 40.0]),
            erased("still", [560.0, 0.0, 200.0, 60.0]),
        ];
        drop_moving_erase(&mut blocks, Some(&before), &after);
        assert!(blocks[0].erase.is_none());
        assert!(blocks[1].erase.is_some());
        // Nothing to compare with: not known to be still.
        drop_moving_erase(&mut blocks, None, &after);
        assert!(blocks[1].erase.is_none());
    }

    #[test]
    fn a_block_shown_with_the_box_keeps_it_for_the_same_text() {
        let old = vec![block("a", [0.0; 4]), erased("b", [0.0; 4])];
        let mut new = vec![
            erased("a", [0.0; 4]),
            erased("b", [0.0; 4]),
            erased("c", [0.0; 4]),
        ];
        keep_plain(&old, &mut new);
        assert!(new[0].erase.is_none());
        assert!(new[1].erase.is_some());
        assert!(new[2].erase.is_some());
    }

    #[test]
    fn identical_frames_do_not_count_as_changed() {
        let a = Thumb::of(&with_word(100));
        let b = Thumb::of(&with_word(100));
        assert_eq!(a.changed_share(&b, None), 0.0);
    }

    #[test]
    fn a_new_word_in_a_subtitle_line_is_a_change() {
        let a = Thumb::of(&with_word(100));
        let b = Thumb::of(&with_word(400));
        assert!(a.changed_share(&b, None) >= REGION_CHANGED);
    }

    #[test]
    fn faint_noise_is_not_a_change() {
        let a = Thumb::of(&solid(800, 60, 120));
        let b = Thumb::of(&solid(800, 60, 130));
        assert_eq!(a.changed_share(&b, None), 0.0);
    }

    #[test]
    fn change_is_measured_inside_a_block_only() {
        let a = Thumb::of(&with_word(100));
        let b = Thumb::of(&with_word(400));
        // The word moved away from x=100 and into x=400.
        assert!(a.changed_share(&b, Some([90.0, 15.0, 80.0, 30.0])) >= BLOCK_CHANGED);
        assert!(a.changed_share(&b, Some([600.0, 15.0, 80.0, 30.0])) < BLOCK_CHANGED);
    }

    #[test]
    fn a_resized_region_counts_as_fully_changed() {
        let a = Thumb::of(&solid(800, 60, 0));
        let b = Thumb::of(&solid(400, 60, 0));
        assert_eq!(a.changed_share(&b, None), 1.0);
    }

    #[test]
    fn rereading_the_same_text_keeps_translations_at_new_positions() {
        let mut old = vec![block("Hello", [0.0, 0.0, 50.0, 20.0])];
        old[0].translation = Some("こんにちは".into());
        let new = vec![block("Hello", [10.0, 40.0, 50.0, 20.0])];
        let out = carry_translations(&old, new);
        assert_eq!(out[0].translation.as_deref(), Some("こんにちは"));
        assert_eq!(out[0].bbox, [10.0, 40.0, 50.0, 20.0]);
    }

    #[test]
    fn a_line_being_typed_out_extends_the_reading_before_it() {
        assert!(extends("The north wind", "The north wind blows"));
        // The last glyph was read half-drawn.
        assert!(extends("The north wi", "The north wind blows"));
        assert!(!extends("The north wind", "The north wind"));
        assert!(!extends("The north wind", "A warm sun"));
        // Too short to tell a growing line from another text.
        assert!(!extends("Th", "The north wind"));
        assert!(key_extends("ja\u{1}The north", "ja\u{1}The north wind"));
        assert!(!key_extends("ja\u{1}The north", "en\u{1}The north wind"));
        assert!(!key_extends("ja\u{1}", "ja\u{1}The north wind"));
    }

    #[test]
    fn carried_context_keeps_whole_lines_within_its_budget() {
        let mut t = Tracker::new();
        t.carry("The north wind", "北風が");
        t.carry("The north wind blows.", "北風が吹く。");
        assert_eq!(
            t.carried,
            vec![(
                "The north wind blows.".to_string(),
                "北風が吹く。".to_string()
            )]
        );
        t.carry("", "x");
        t.carry("x", " ");
        assert_eq!(t.carried.len(), 1);
        for i in 0..200 {
            t.carry(&format!("Line number {i} of the dialogue."), "台詞。");
        }
        let total: usize = t
            .carried
            .iter()
            .map(|(s, tr)| translate_plan::cost(s) + translate_plan::cost(tr))
            .sum();
        assert!(total <= CARRY && t.carried.len() > 1, "{total}");
        assert_eq!(
            t.carried.last().unwrap().0,
            "Line number 199 of the dialogue."
        );
    }

    #[test]
    fn source_key_ignores_surrounding_whitespace() {
        let a = vec![block(" Hello ", [0.0; 4]), block("World", [0.0; 4])];
        let b = vec![block("Hello", [0.0; 4]), block("World\n", [0.0; 4])];
        assert_eq!(source_key(&a), source_key(&b));
    }
}
