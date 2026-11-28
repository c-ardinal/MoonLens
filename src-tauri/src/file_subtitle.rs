//! Subtitles for a media file: recognition and translation of its audio
//! track from start to end.
//!
//! Memory does not grow with the file. [`crate::media_audio`] decodes
//! the audio track packet by packet on its own thread and hands blocks
//! over a short queue; [`run`] holds one recognition window (under 30 s
//! of 16 kHz mono) and blocks the decoder while a window is recognized
//! and translated. What it keeps of a window afterwards is its text.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::sync::mpsc;

use crate::media_audio::{MediaAudio, OpenError, RATE};
use crate::pipeline::Pipeline;
use crate::providers::{Capability, TaskInput, TaskOutput, TranscriptSegment, TranslateContext};
use crate::settings::schema::SubtitleSettings;
use crate::settings::SettingsStore;
use crate::storage::{Storage, SubtitleLine};
use crate::subtitle::{is_silent, SubtitleEvent};
use crate::subtitle_burn::{self, BurnError};
use crate::subtitle_filter::StockFilter;
use crate::subtitle_record::{self, ExportContent, ExportFormat, Recorder};
use crate::tr;

/// Longest audio sent to recognition at once: just under Whisper's
/// 30 s window, as in live subtitles.
pub const WINDOW: usize = 28 * RATE;

/// A last segment that ends this close to the end of a window that
/// more audio follows may be a sentence cut short.
const TAIL_MS: u64 = 1000;
/// Such a segment is recognized again with the next window only when
/// that leaves the window at least this much shorter: every window
/// moves on.
const MIN_ADVANCE_MS: u64 = 10_000;

/// Blocks queued between the decoder thread and [`run`] (a block is at
/// most a second of audio).
pub const QUEUE: usize = 4;

/// A pause this long between two segments ends the cue.
const GAP_MS: u64 = 1500;
/// A cue is closed at the next segment once it is this long…
const MAX_CUE_MS: u64 = 15_000;
/// …or holds this much text (ASCII 1, other characters 3, as the
/// translation planner counts).
const MAX_CUE_COST: usize = 320;

/// Earlier cues given to the translation as context.
const CONTEXT_CUES: usize = 3;

/// Recognition and translation as [`run`] uses them (the provider chain
/// in the app, a stub in tests).
#[async_trait]
pub trait Engine: Send + Sync {
    /// Recognize 16 kHz mono audio of at most [`WINDOW`] samples.
    /// Segment times count from the start of `pcm`.
    async fn recognize(&self, pcm: &[i16]) -> Result<Vec<TranscriptSegment>, String>;
    /// `None` when the translation failed: the cue keeps its original.
    async fn translate(&self, text: &str, history: &[(String, String)]) -> Option<String>;
}

/// One subtitle. Times count from the start of the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub original: String,
    /// Empty when the translation failed.
    pub translated: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Cue(Cue),
    /// The audio up to here is done.
    Progress {
        done_ms: u64,
    },
}

/// Turn the audio blocks from `rx` into cues until the channel closes.
/// `Err` is a recognition failure (the cues emitted so far stand).
/// Dropping the future cancels the run.
pub async fn run(
    mut rx: mpsc::Receiver<Vec<i16>>,
    engine: &impl Engine,
    settings: &SubtitleSettings,
    emit: &(impl Fn(Event) + Sync),
) -> Result<(), String> {
    let mut window: Vec<i16> = Vec::with_capacity(WINDOW + RATE);
    let mut window_start = 0u64;
    let mut ended = false;
    let mut stock = StockFilter::default();
    let mut joiner = Joiner::default();
    let mut history: Vec<(String, String)> = Vec::new();
    loop {
        while !ended && window.len() < WINDOW {
            match rx.recv().await {
                Some(block) => window.extend_from_slice(&block),
                None => ended = true,
            }
        }
        if window.is_empty() {
            break;
        }
        let chunk = &window[..window.len().min(WINDOW)];
        let last_of_file = ended && chunk.len() == window.len();
        let offset_ms = samples_ms(window_start);
        let chunk_ms = samples_ms(chunk.len() as u64);
        let mut cut = chunk.len();
        // Silence makes Whisper invent stock phrases; skip it. Second
        // by second: a short quiet line among silence is no silence,
        // though the level of the whole window says so.
        if !chunk.chunks(RATE).all(is_silent) {
            let at = |s: f32| ((s.max(0.0) * 1000.0) as u64).min(chunk_ms);
            let mut segments: Vec<(u64, u64, String)> = engine
                .recognize(chunk)
                .await?
                .into_iter()
                .map(|s| (at(s.start_s), at(s.end_s), s.text))
                .collect();
            tracing::debug!(
                "file subtitles: window at {offset_ms} ms ({chunk_ms} ms): {segments:?}"
            );
            if !last_of_file {
                if let Some((keep, resume_ms)) = cut_short(&segments, chunk_ms) {
                    segments.truncate(keep);
                    cut = (resume_ms * RATE as u64 / 1000) as usize;
                }
            }
            for (start, end, text) in screen(&mut stock, settings, segments) {
                for done in joiner.push(offset_ms + start, offset_ms + end, &text) {
                    finish(engine, emit, done, &mut history).await;
                }
            }
        }
        window.drain(..cut);
        window_start += cut as u64;
        // A sentence left open may go on in the next window; a pause
        // before the cut ends it like any other.
        if let Some(done) = joiner.close_before(samples_ms(window_start)) {
            finish(engine, emit, done, &mut history).await;
        }
        emit(Event::Progress {
            done_ms: samples_ms(window_start),
        });
    }
    if let Some(done) = joiner.take() {
        finish(engine, emit, done, &mut history).await;
    }
    Ok(())
}

/// Drop the stock phrases from the segments of a window. A sentence is
/// judged whole, across the segments it was recognized in: speech that
/// only starts like a stock phrase stays. Segments keep their times;
/// a sentence that loses a part becomes one segment over its span.
fn screen(
    stock: &mut StockFilter,
    settings: &SubtitleSettings,
    segments: Vec<(u64, u64, String)>,
) -> Vec<(u64, u64, String)> {
    let mut out = Vec::with_capacity(segments.len());
    let mut rest = segments.as_slice();
    while !rest.is_empty() {
        let open = |(_, _, text): &(u64, u64, String)| !ends_sentence(text) && !is_note(text);
        let len = rest
            .iter()
            .position(|s| !open(s))
            .map_or(rest.len(), |i| i + 1);
        let (sentence, after) = rest.split_at(len);
        rest = after;
        let texts: Vec<&str> = sentence.iter().map(|(_, _, text)| text.as_str()).collect();
        let screened = stock.screen(settings, &texts, Instant::now());
        let text = screened.text.trim();
        // Unchanged but for its spacing: nothing was removed (symbols go
        // without being listed as blocked).
        let bare = |t: &str| t.split_whitespace().collect::<String>();
        if bare(text) == bare(&texts.concat()) {
            out.extend(sentence.iter().filter(|s| !s.2.trim().is_empty()).cloned());
        } else if !text.is_empty() {
            out.push((sentence[0].0, sentence[len - 1].1, text.to_string()));
        }
    }
    out
}

/// A segment that is an annotation of the recognizer ("[Music]"), not
/// part of a sentence.
fn is_note(text: &str) -> bool {
    let text = text.trim();
    text.starts_with(['[', '(', '（', '【', '♪']) && text.ends_with([']', ')', '）', '】', '♪'])
}

/// Translate a finished cue with the cues before it as context, and
/// hand it out.
async fn finish(
    engine: &impl Engine,
    emit: &(impl Fn(Event) + Sync),
    open: Open,
    history: &mut Vec<(String, String)>,
) {
    let translated = engine
        .translate(&open.text, history)
        .await
        .unwrap_or_default();
    if !translated.is_empty() {
        history.push((open.text.clone(), translated.clone()));
        if history.len() > CONTEXT_CUES {
            history.remove(0);
        }
    }
    emit(Event::Cue(Cue {
        start_ms: open.start_ms,
        end_ms: open.end_ms,
        original: open.text,
        translated,
    }));
}

fn samples_ms(samples: u64) -> u64 {
    samples * 1000 / RATE as u64
}

/// Whether the end of a window of `chunk_ms` may have cut a sentence
/// short: the last of `segments` (start, end, text; times in the
/// window) ends where the window does. Returns how many segments to
/// keep and where the next window then starts, so that the sentence is
/// recognized whole there: at the last sentence start in the window,
/// or else before the last segment. Whisper sets its segments at
/// pauses and sentence ends, which places the cut better than the
/// level of the audio can.
fn cut_short(segments: &[(u64, u64, String)], chunk_ms: u64) -> Option<(usize, u64)> {
    let last = segments.last()?;
    if last.1 + TAIL_MS < chunk_ms {
        return None;
    }
    // Between the end of the segment before and the start of this one.
    let resume = |i: usize| (segments[i - 1].1.min(segments[i].0) + segments[i].0) / 2;
    let far_enough = |i: &usize| resume(*i) >= MIN_ADVANCE_MS;
    let starts = (1..segments.len()).rev().filter(far_enough);
    let at = starts
        .clone()
        .find(|i| ends_sentence(&segments[i - 1].2))
        .or_else(|| starts.clone().next())?;
    Some((at, resume(at)))
}

/// A cue being put together from segments.
#[derive(Debug, PartialEq, Eq)]
struct Open {
    start_ms: u64,
    end_ms: u64,
    text: String,
}

/// Joins recognized segments into cues that are whole sentences where
/// the speech allows: a sentence translated in pieces loses its
/// meaning, above all between languages of different word order.
#[derive(Default)]
struct Joiner {
    open: Option<Open>,
    /// End of the last cue handed out: cue times never overlap.
    last_end_ms: u64,
}

impl Joiner {
    /// Add a segment. Returns the cues this completed, in order: the
    /// one the segment could not join, and the one it ended.
    fn push(&mut self, start_ms: u64, end_ms: u64, text: &str) -> impl Iterator<Item = Open> {
        let before = self.close_before(start_ms);
        let start_ms = start_ms.max(self.last_end_ms);
        let end_ms = end_ms.max(start_ms + 1);
        match &mut self.open {
            Some(open) => {
                open.text = join_text(&open.text, text);
                open.end_ms = end_ms.max(open.end_ms);
            }
            None => {
                self.open = Some(Open {
                    start_ms,
                    end_ms,
                    text: text.to_string(),
                })
            }
        }
        [before, self.take_if(|open| ends_sentence(&open.text))]
            .into_iter()
            .flatten()
    }

    /// Close the open cue when the next segment, starting at `next_ms`,
    /// must not join it: after a pause, or once the cue is long enough.
    fn close_before(&mut self, next_ms: u64) -> Option<Open> {
        self.take_if(|open| {
            next_ms.saturating_sub(open.end_ms) >= GAP_MS
                || open.end_ms - open.start_ms >= MAX_CUE_MS
                || cost(&open.text) >= MAX_CUE_COST
        })
    }

    fn take_if(&mut self, closes: impl Fn(&Open) -> bool) -> Option<Open> {
        if self.open.as_ref().is_some_and(closes) {
            self.take()
        } else {
            None
        }
    }

    fn take(&mut self) -> Option<Open> {
        let open = self.open.take()?;
        self.last_end_ms = open.end_ms;
        Some(open)
    }
}

fn ends_sentence(text: &str) -> bool {
    let closers = ['\'', '"', '”', '’', '」', '』', '）', ')', ']', '】'];
    text.trim_end()
        .trim_end_matches(closers)
        .ends_with(['。', '．', '.', '!', '?', '！', '？', '…'])
}

/// ASCII 1, other characters 3 (`translate_plan::cost`).
fn cost(text: &str) -> usize {
    text.chars().map(|c| if c.is_ascii() { 1 } else { 3 }).sum()
}

/// Join two pieces of a sentence: a space between words, none where
/// either side is Japanese or Chinese text or already a space.
fn join_text(left: &str, right: &str) -> String {
    let unspaced = |c: char| {
        matches!(c, '\u{3000}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}' | '\u{ff00}'..='\u{ffef}')
            || c.is_whitespace()
    };
    match (left.chars().last(), right.chars().next()) {
        (Some(a), Some(b)) if !unspaced(a) && !unspaced(b) => format!("{left} {right}"),
        _ => format!("{left}{right}"),
    }
}

/// Decode `audio` on the calling thread into `tx` until the track ends
/// or the receiver is gone. Blocks while the queue is full: the reader
/// of a long file waits for recognition instead of piling audio up.
pub fn feed(mut audio: MediaAudio, tx: mpsc::Sender<Vec<i16>>) {
    while let Some(block) = audio.read() {
        if tx.blocking_send(block).is_err() {
            break;
        }
    }
}

/// Start decoding `path` on a blocking thread. Returns the length of
/// its audio track, when the container states it, and the blocks.
pub fn open(path: &Path) -> Result<(Option<u64>, mpsc::Receiver<Vec<i16>>), OpenError> {
    let audio = MediaAudio::open(path)?;
    let duration_ms = audio.duration_ms();
    let (tx, rx) = mpsc::channel(QUEUE);
    tokio::task::spawn_blocking(move || feed(audio, tx));
    Ok((duration_ms, rx))
}

/// [`Engine`] on the provider chain, with the languages of the audio
/// translation settings.
pub struct PipelineEngine {
    pub pipeline: Arc<Pipeline>,
    pub source_language: String,
    pub target_language: String,
}

#[async_trait]
impl Engine for PipelineEngine {
    async fn recognize(&self, pcm: &[i16]) -> Result<Vec<TranscriptSegment>, String> {
        let input = TaskInput::Asr {
            audio_pcm16_mono_16k: crate::subtitle::pcm16_to_bytes(pcm),
            language_hint: (self.source_language != "auto").then(|| self.source_language.clone()),
            language_candidates: Vec::new(),
        };
        match self.pipeline.run(Capability::Asr, input).await {
            Ok(res) => Ok(match res.output {
                TaskOutput::Transcript { segments, .. } => segments,
                // A provider without times: one segment for the chunk.
                TaskOutput::Text(text) => vec![TranscriptSegment {
                    start_s: 0.0,
                    end_s: pcm.len() as f32 / RATE as f32,
                    text,
                }],
                _ => Vec::new(),
            }),
            Err(e) => {
                tracing::warn!("file subtitles: recognition failed: {e}");
                let detail = crate::user_error::provider(&e, Capability::Asr);
                Err(match detail.split_once('|') {
                    Some((_, message)) => message.to_string(),
                    None => detail,
                })
            }
        }
    }

    async fn translate(&self, text: &str, history: &[(String, String)]) -> Option<String> {
        let input = TaskInput::Translate {
            text: text.to_string(),
            source_language: self.source_language.clone(),
            target_language: self.target_language.clone(),
            context: TranslateContext {
                history: history.to_vec(),
                ..Default::default()
            },
        };
        match self.pipeline.run(Capability::Translate, input).await {
            Ok(res) => match res.output {
                TaskOutput::Text(s) => Some(s),
                _ => None,
            },
            Err(e) => {
                tracing::warn!("file subtitles: translation failed: {e}");
                None
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// No file yet.
    #[default]
    Idle,
    Running,
    Done,
    Cancelled,
    Failed,
}

/// The job as the page shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Status {
    pub phase: Phase,
    /// File name without its folder.
    pub name: String,
    /// Length of the audio track, when the container states it.
    pub duration_ms: Option<u64>,
    /// The audio up to here is done.
    pub done_ms: u64,
    /// Subtitles so far.
    pub cues: usize,
    /// Why the job failed.
    pub error: Option<String>,
}

/// A change of the job: the new status and the subtitle that caused it,
/// if one did.
#[derive(Debug, Clone, Serialize)]
pub struct Update {
    pub status: Status,
    pub cue: Option<Cue>,
}

/// Event that carries an [`Update`] to the main window.
pub const EVENT: &str = "moonlens://file-subtitle";

pub type Notify = Arc<dyn Fn(Update) + Send + Sync>;

/// The file subtitle job: one file at a time. The subtitles of the last
/// file stay for export until the next one starts.
pub struct FileSubtitleService {
    settings: Arc<SettingsStore>,
    pipeline: Arc<Pipeline>,
    storage: Arc<Storage>,
    inner: Arc<Mutex<Inner>>,
    /// Stop flag of the running burn-in, if one runs.
    burn: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}

#[derive(Default)]
struct Inner {
    status: Status,
    /// Folder and name of the file, for the export dialog.
    path: PathBuf,
    target_language: String,
    lines: Vec<SubtitleLine>,
    job: Option<JoinHandle<()>>,
    /// Counts the jobs: a stopped job that is still winding down must
    /// not write into the next one.
    run: u64,
}

impl FileSubtitleService {
    pub fn new(
        settings: Arc<SettingsStore>,
        pipeline: Arc<Pipeline>,
        storage: Arc<Storage>,
    ) -> Self {
        Self {
            settings,
            pipeline,
            storage,
            inner: Default::default(),
            burn: Default::default(),
        }
    }

    pub fn status(&self) -> Status {
        self.inner.lock().status.clone()
    }

    /// Start on `path`. Must run inside the async runtime. `notify`
    /// hears every change, the last one included.
    pub fn start(&self, path: &Path, notify: Notify) -> Result<Status, String> {
        let engine = {
            let s = self.settings.snapshot();
            PipelineEngine {
                pipeline: self.pipeline.clone(),
                source_language: s.subtitle.source_language.clone(),
                target_language: s.subtitle.target_language.clone(),
            }
        };
        self.start_with(path, engine, notify)
    }

    /// [`Self::start`] with the recognition and translation given.
    pub fn start_with(
        &self,
        path: &Path,
        engine: impl Engine + 'static,
        notify: Notify,
    ) -> Result<Status, String> {
        let mut inner = self.inner.lock();
        if inner.status.phase == Phase::Running {
            return Err(tr!("file-subtitle:already_running"));
        }
        let (duration_ms, rx) = open(path).map_err(|e| match e {
            OpenError::Io(e) => tr!("file-subtitle:cannot_read_file", e = e),
            OpenError::UnknownFormat => tr!("file-subtitle:unknown_format"),
            OpenError::NoAudio => tr!("file-subtitle:no_audio_track"),
            OpenError::UnsupportedCodec => tr!("file-subtitle:unsupported_codec"),
        })?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let settings = self.settings.snapshot().subtitle.clone();
        *inner = Inner {
            status: Status {
                phase: Phase::Running,
                name: name.clone(),
                duration_ms,
                ..Default::default()
            },
            path: path.to_path_buf(),
            target_language: settings.target_language.clone(),
            lines: Vec::new(),
            job: None,
            run: inner.run + 1,
        };
        let run_id = inner.run;
        // The job translates with the languages it started with; so
        // does its record, whatever the settings become meanwhile.
        let recorder = Recorder::new(self.storage.clone(), self.settings.clone())
            .languages_fixed(&settings.source_language, &settings.target_language);
        recorder.observe(&SubtitleEvent::Started {
            device: name,
            source: crate::audio::AudioSource::System,
        });
        let shared = self.inner.clone();
        inner.job = Some(tauri::async_runtime::spawn(async move {
            let emit = |event: Event| {
                let mut inner = shared.lock();
                if inner.run != run_id {
                    return;
                }
                let cue = match event {
                    Event::Progress { done_ms } => {
                        inner.status.done_ms = done_ms;
                        None
                    }
                    Event::Cue(cue) => {
                        let seq = inner.lines.len();
                        inner.lines.push(SubtitleLine {
                            seq: seq as i64,
                            start_ms: cue.start_ms as i64,
                            end_ms: cue.end_ms as i64,
                            original: cue.original.clone(),
                            translated: cue.translated.clone(),
                        });
                        inner.status.cues = inner.lines.len();
                        recorder.observe(&SubtitleEvent::Chunk {
                            id: seq as u64,
                            original: cue.original.clone(),
                            translated: cue.translated.clone(),
                            at_ms: cue.end_ms,
                            duration_ms: cue.end_ms - cue.start_ms,
                            speaker: None,
                            language: inner.target_language.clone(),
                        });
                        Some(cue)
                    }
                };
                let status = inner.status.clone();
                drop(inner);
                notify(Update { status, cue });
            };
            let result = run(rx, &engine, &settings, &emit).await;
            let status = {
                let mut inner = shared.lock();
                if inner.run != run_id {
                    return;
                }
                match result {
                    Ok(()) => inner.status.phase = Phase::Done,
                    Err(e) => {
                        inner.status.phase = Phase::Failed;
                        inner.status.error = Some(e);
                    }
                }
                inner.job = None;
                inner.status.clone()
            };
            notify(Update { status, cue: None });
        }));
        Ok(inner.status.clone())
    }

    /// Stop the running job. Its subtitles so far stay for export.
    pub fn cancel(&self) -> Status {
        let mut inner = self.inner.lock();
        if let Some(job) = inner.job.take() {
            job.abort();
            inner.status.phase = Phase::Cancelled;
        }
        inner.status.clone()
    }

    /// The subtitles so far as a file: its text, the folder of the
    /// media file and the name offered for it (`talk.ja.srt` for
    /// `talk.mp4`, as players look for a subtitle file).
    pub fn export(&self, format: ExportFormat, content: ExportContent) -> Export {
        let inner = self.inner.lock();
        let text = match format {
            ExportFormat::Srt => subtitle_record::render_srt(&inner.lines, content),
            ExportFormat::Txt => subtitle_record::render_text(&inner.lines, content),
        };
        let stem = inner
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let language = match content {
            ExportContent::Original => String::new(),
            _ => format!(".{}", inner.target_language),
        };
        Export {
            text,
            folder: inner.path.parent().map(Path::to_path_buf),
            name: format!("{stem}{language}.{}", format.extension()),
        }
    }
}

impl FileSubtitleService {
    /// Name offered for the burnt-in copy of the media file and its
    /// folder (`talk.ja-subtitled.mp4` for `talk.mp4`).
    pub fn burn_target(&self) -> (Option<PathBuf>, String) {
        let inner = self.inner.lock();
        let stem = inner
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = subtitle_burn::output_extension(&inner.path);
        (
            inner.path.parent().map(Path::to_path_buf),
            format!("{stem}.{}-subtitled.{ext}", inner.target_language),
        )
    }

    /// Burn the subtitles so far into a copy of the media file at
    /// `output`, on a thread of its own. `notify` hears the progress and
    /// the end.
    pub fn start_burn(
        &self,
        ffmpeg: PathBuf,
        output: PathBuf,
        content: ExportContent,
        notify: impl Fn(BurnUpdate) + Send + 'static,
    ) -> Result<(), String> {
        let (input, lines, language) = {
            let inner = self.inner.lock();
            if inner.status.phase == Phase::Running {
                return Err(tr!("file-subtitle:already_running"));
            }
            (
                inner.path.clone(),
                inner.lines.clone(),
                inner.target_language.clone(),
            )
        };
        if output == input {
            return Err(tr!("file-burn:same_as_source"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut burn = self.burn.lock();
            if burn.is_some() {
                return Err(tr!("file-burn:already_running"));
            }
            *burn = Some(cancel.clone());
        }
        let slot = self.burn.clone();
        let file = output
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        std::thread::spawn(move || {
            let update = |phase, done_ms, error| BurnUpdate {
                phase,
                done_ms,
                file: file.clone(),
                error,
            };
            notify(update(BurnPhase::Running, 0, None));
            let job = subtitle_burn::Job {
                ffmpeg: &ffmpeg,
                input: &input,
                output: &output,
                lines: &lines,
                content,
                language: &language,
                encoder: None,
            };
            let result = subtitle_burn::burn(
                &job,
                |ms| notify(update(BurnPhase::Running, ms, None)),
                &cancel,
            );
            *slot.lock() = None;
            notify(match result {
                Ok(report) => {
                    tracing::info!(
                        "burn-in: {}x{} with {} in {} ms",
                        report.width,
                        report.height,
                        report.encoder.codec(),
                        report.elapsed_ms
                    );
                    update(BurnPhase::Done, report.duration_ms.unwrap_or(0), None)
                }
                Err(BurnError::Cancelled) => update(BurnPhase::Cancelled, 0, None),
                Err(e) => {
                    tracing::warn!("burn-in failed: {e}");
                    let message = match e {
                        BurnError::NoVideo => tr!("file-burn:no_video"),
                        BurnError::NoEncoder => tr!("file-burn:no_encoder"),
                        e => tr!("file-burn:failed", e = e),
                    };
                    update(BurnPhase::Failed, 0, Some(message))
                }
            });
        });
        Ok(())
    }

    /// Stop the running burn-in; its partial file is removed.
    pub fn cancel_burn(&self) {
        if let Some(cancel) = self.burn.lock().as_ref() {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BurnPhase {
    Running,
    Done,
    Cancelled,
    Failed,
}

/// Progress of a burn-in, as the page shows it.
#[derive(Debug, Clone, Serialize)]
pub struct BurnUpdate {
    pub phase: BurnPhase,
    /// Video time done so far.
    pub done_ms: u64,
    /// Name of the output file.
    pub file: String,
    pub error: Option<String>,
}

/// Event that carries a [`BurnUpdate`] to the main window.
pub const BURN_EVENT: &str = "moonlens://file-burn";

pub struct Export {
    pub text: String,
    pub folder: Option<PathBuf>,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    /// Audible audio: the silence gate lets it through.
    fn tone(samples: usize) -> Vec<i16> {
        (0..samples)
            .map(|i| ((i as f32 * 0.05).sin() * 3000.0) as i16)
            .collect()
    }

    /// Answers every recognition with one sentence a second into the
    /// chunk, numbered by call, and records what it was given.
    #[derive(Default)]
    struct Stub {
        calls: AtomicUsize,
        /// Samples received for recognition so far.
        recognized: AtomicU64,
        longest: AtomicUsize,
        /// Fail the n-th recognition (from 0).
        fail_at: Option<usize>,
        /// Translations fail.
        no_translation: bool,
        contexts: Mutex<Vec<Vec<(String, String)>>>,
    }

    #[async_trait]
    impl Engine for Stub {
        async fn recognize(&self, pcm: &[i16]) -> Result<Vec<TranscriptSegment>, String> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            self.recognized
                .fetch_add(pcm.len() as u64, Ordering::SeqCst);
            self.longest.fetch_max(pcm.len(), Ordering::SeqCst);
            if self.fail_at == Some(n) {
                return Err("engine failed".into());
            }
            Ok(vec![TranscriptSegment {
                start_s: 1.0,
                end_s: 3.0,
                text: format!("Sentence {n}."),
            }])
        }

        async fn translate(&self, text: &str, history: &[(String, String)]) -> Option<String> {
            self.contexts.lock().push(history.to_vec());
            (!self.no_translation).then(|| format!("訳：{text}"))
        }
    }

    async fn cues_of(blocks: Vec<Vec<i16>>, engine: &Stub) -> (Vec<Event>, Result<(), String>) {
        let (tx, rx) = mpsc::channel(QUEUE);
        let feeder = tokio::spawn(async move {
            for block in blocks {
                if tx.send(block).await.is_err() {
                    break;
                }
            }
        });
        let events = Mutex::new(Vec::new());
        let emit = |e: Event| events.lock().push(e);
        let result = run(rx, engine, &SubtitleSettings::default(), &emit).await;
        feeder.abort();
        (events.into_inner(), result)
    }

    fn cues(events: &[Event]) -> Vec<&Cue> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Cue(cue) => Some(cue),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn cue_times_are_those_of_the_file() {
        let engine = Stub::default();
        // 70 s in blocks of a second: three windows.
        let (events, result) = cues_of(vec![tone(RATE); 70], &engine).await;
        result.unwrap();
        let cues = cues(&events);
        assert_eq!(cues.len(), 3);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 3000));
        // A later window starts where the one before ended.
        assert_eq!(events[1], Event::Progress { done_ms: 28_000 });
        assert_eq!(cues[1].start_ms, 28_000 + 1000);
        assert_eq!(cues[1].translated, "訳：Sentence 1.");
        assert_eq!(
            events.last(),
            Some(&Event::Progress { done_ms: 70_000 }),
            "the whole file is reported done"
        );
    }

    #[tokio::test]
    async fn a_long_file_is_held_one_window_at_a_time() {
        let engine = Arc::new(Stub::default());
        // Three hours, produced only as fast as the run takes it.
        const BLOCKS: u64 = 3 * 3600;
        let produced = Arc::new(AtomicU64::new(0));
        let held_most = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel(QUEUE);
        let feeder = {
            let (produced, held_most, engine) =
                (produced.clone(), held_most.clone(), engine.clone());
            tokio::spawn(async move {
                let block = tone(RATE);
                for _ in 0..BLOCKS {
                    tx.send(block.clone()).await.unwrap();
                    let made = produced.fetch_add(RATE as u64, Ordering::SeqCst) + RATE as u64;
                    // Audio handed over and not yet through recognition.
                    let held = made - engine.recognized.load(Ordering::SeqCst);
                    held_most.fetch_max(held, Ordering::SeqCst);
                }
            })
        };
        let count = AtomicUsize::new(0);
        let emit = |e: Event| {
            if matches!(e, Event::Cue(_)) {
                count.fetch_add(1, Ordering::SeqCst);
            }
        };
        run(rx, &*engine, &SubtitleSettings::default(), &emit)
            .await
            .unwrap();
        feeder.await.unwrap();
        assert_eq!(
            engine.recognized.load(Ordering::SeqCst),
            BLOCKS * RATE as u64
        );
        assert!(engine.longest.load(Ordering::SeqCst) <= WINDOW);
        // One window, the queue, and the blocks in hand on either side.
        let limit = (WINDOW + (QUEUE + 3) * RATE) as u64;
        let held = held_most.load(Ordering::SeqCst);
        assert!(held <= limit, "{held} samples held, limit {limit}");
        assert!(count.load(Ordering::SeqCst) >= (BLOCKS / 28) as usize);
    }

    #[tokio::test]
    async fn silence_is_not_recognized() {
        let engine = Stub::default();
        let (events, result) = cues_of(vec![vec![0; RATE]; 40], &engine).await;
        result.unwrap();
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        assert_eq!(events.last(), Some(&Event::Progress { done_ms: 40_000 }));
    }

    #[test]
    fn stock_phrases_are_judged_by_the_sentence_not_the_segment() {
        let settings = SubtitleSettings {
            block_stock_phrases: true,
            ..Default::default()
        };
        let seg = |start: u64, end: u64, text: &str| (start, end, text.to_string());
        let screened = |segments: Vec<(u64, u64, String)>| {
            screen(&mut StockFilter::default(), &settings, segments)
        };
        // Speech that only starts like a stock phrase keeps its
        // segments and their times.
        let speech = vec![
            seg(0, 1500, "Thanks for watching"),
            seg(1500, 3000, "the kids yesterday."),
        ];
        assert_eq!(screened(speech.clone()), speech);
        // Symbols alone are no subtitle.
        assert_eq!(
            screened(vec![seg(0, 1000, "♪♪"), seg(2000, 3000, "...")]),
            vec![]
        );
        // A stock phrase goes, in one piece or two; its neighbours stay
        // where they were.
        assert_eq!(
            screened(vec![
                seg(0, 1000, "That is all."),
                seg(5000, 6000, "Thanks for"),
                seg(6000, 7000, "watching!"),
                seg(9000, 10_000, "[Music]"),
                seg(20_000, 21_000, "Welcome back."),
            ]),
            vec![
                seg(0, 1000, "That is all."),
                seg(20_000, 21_000, "Welcome back.")
            ]
        );
    }

    #[tokio::test]
    async fn a_short_quiet_line_among_silence_is_recognized() {
        // A second at four times the silence level, then silence: the
        // level of the whole window is below it.
        let quiet: Vec<i16> = tone(RATE).iter().map(|s| s / 5).collect();
        let mut blocks = vec![quiet];
        blocks.extend(vec![vec![0; RATE]; 27]);
        assert!(is_silent(&blocks.concat()));
        let engine = Stub::default();
        let (_, result) = cues_of(blocks, &engine).await;
        result.unwrap();
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_recognition_failure_ends_the_run_and_keeps_earlier_cues() {
        let engine = Stub {
            fail_at: Some(1),
            ..Default::default()
        };
        let (events, result) = cues_of(vec![tone(RATE); 70], &engine).await;
        assert_eq!(result, Err("engine failed".to_string()));
        assert_eq!(cues(&events).len(), 1);
    }

    #[tokio::test]
    async fn a_failed_translation_leaves_the_original() {
        let engine = Stub {
            no_translation: true,
            ..Default::default()
        };
        let (events, result) = cues_of(vec![tone(RATE); 10], &engine).await;
        result.unwrap();
        let cues = cues(&events);
        assert_eq!(cues[0].original, "Sentence 0.");
        assert_eq!(cues[0].translated, "");
    }

    #[tokio::test]
    async fn translation_gets_the_cues_before_as_context() {
        let engine = Stub::default();
        let (_, result) = cues_of(vec![tone(RATE); 28 * 5], &engine).await;
        result.unwrap();
        let contexts = engine.contexts.lock();
        assert!(contexts[0].is_empty());
        assert_eq!(
            contexts[1],
            vec![("Sentence 0.".to_string(), "訳：Sentence 0.".to_string())]
        );
        assert!(contexts.iter().all(|c| c.len() <= CONTEXT_CUES));
        assert_eq!(contexts.last().unwrap().len(), CONTEXT_CUES);
    }

    /// Recognition of a talk of sentences `SENTENCE_S` long, back to
    /// back from the start of the file: every sentence that starts in
    /// the chunk, the last one cut off where the chunk ends. The chunk
    /// tells its place in the file by its first sample.
    struct Talk {
        chunks: Mutex<Vec<(u64, usize)>>,
    }
    const SENTENCE_S: u64 = 6;

    /// Audible blocks of a second whose first sample is the second.
    fn numbered(seconds: u64) -> Vec<Vec<i16>> {
        (0..seconds)
            .map(|s| {
                let mut block = tone(RATE);
                block[0] = s as i16;
                block
            })
            .collect()
    }

    #[async_trait]
    impl Engine for Talk {
        async fn recognize(&self, pcm: &[i16]) -> Result<Vec<TranscriptSegment>, String> {
            let from = pcm[0] as u64;
            assert_eq!(pcm.len() % RATE, 0, "chunks start and end on a second");
            let seconds = (pcm.len() / RATE) as u64;
            self.chunks.lock().push((from, pcm.len()));
            let first = from.div_ceil(SENTENCE_S);
            Ok((first..)
                .take_while(|n| n * SENTENCE_S < from + seconds)
                .map(|n| {
                    let (start, end) = (n * SENTENCE_S, (n + 1) * SENTENCE_S);
                    let whole = end <= from + seconds;
                    TranscriptSegment {
                        start_s: (start - from) as f32,
                        end_s: (end.min(from + seconds) - from) as f32,
                        text: if whole {
                            format!("Sentence {n}.")
                        } else {
                            format!("Sentence {n} cut")
                        },
                    }
                })
                .collect())
        }

        async fn translate(&self, _: &str, _: &[(String, String)]) -> Option<String> {
            None
        }
    }

    #[tokio::test]
    async fn a_sentence_cut_off_by_a_window_is_recognized_whole_in_the_next() {
        let engine = Talk {
            chunks: Mutex::default(),
        };
        let (tx, rx) = mpsc::channel(QUEUE);
        tokio::spawn(async move {
            for block in numbered(120) {
                tx.send(block).await.unwrap();
            }
        });
        let events = Mutex::new(Vec::new());
        let emit = |e: Event| events.lock().push(e);
        run(rx, &engine, &SubtitleSettings::default(), &emit)
            .await
            .unwrap();
        let events = events.into_inner();
        let got: Vec<(u64, u64, &str)> = cues(&events)
            .iter()
            .map(|c| (c.start_ms, c.end_ms, c.original.as_str()))
            .collect();
        // Every sentence once, whole, at its time in the file.
        let want: Vec<(u64, u64, String)> = (0..120 / SENTENCE_S)
            .map(|n| (n * 6000, (n + 1) * 6000, format!("Sentence {n}.")))
            .collect();
        let want: Vec<(u64, u64, &str)> =
            want.iter().map(|(s, e, t)| (*s, *e, t.as_str())).collect();
        assert_eq!(got, want);
        // Windows start at sentence starts, and each moves on.
        let chunks = engine.chunks.lock();
        assert!(
            chunks.iter().all(|(from, _)| from % SENTENCE_S == 0),
            "{chunks:?}"
        );
        assert!(chunks
            .windows(2)
            .all(|w| w[1].0 >= w[0].0 + MIN_ADVANCE_MS / 1000));
    }

    #[test]
    fn a_window_that_ends_in_speech_resumes_at_its_last_sentence_start() {
        let seg = |s: u64, e: u64, t: &str| (s, e, t.to_string());
        // Ends at the end of the window: resume between the two.
        assert_eq!(
            cut_short(
                &[seg(0, 20_000, "One."), seg(21_000, 28_000, "Two")],
                28_000
            ),
            Some((1, 20_500))
        );
        // Whisper may date a segment before the end of the one before.
        assert_eq!(
            cut_short(
                &[seg(0, 20_000, "One."), seg(19_000, 27_500, "Two")],
                28_000
            ),
            Some((1, 19_000))
        );
        // A sentence split over the last segments goes as a whole.
        let split = [
            seg(0, 12_000, "One."),
            seg(12_400, 22_000, "Two starts"),
            seg(22_000, 28_000, "and goes"),
        ];
        assert_eq!(cut_short(&split, 28_000), Some((1, 12_200)));
        // Without a sentence end to go by: before the last segment.
        let plain = [seg(0, 12_000, "one"), seg(12_000, 28_000, "two")];
        assert_eq!(cut_short(&plain, 28_000), Some((1, 12_000)));
        // Ends before a pause at the end of the window: complete.
        assert_eq!(
            cut_short(
                &[seg(0, 20_000, "One."), seg(21_000, 26_000, "Two")],
                28_000
            ),
            None
        );
        // Nothing before it to end the window with.
        assert_eq!(cut_short(&[seg(0, 28_000, "One")], 28_000), None);
        // The window would hardly move on.
        assert_eq!(
            cut_short(&[seg(0, 3000, "One."), seg(3000, 28_000, "Two")], 28_000),
            None
        );
    }

    fn joined(segments: &[(u64, u64, &str)]) -> Vec<(u64, u64, String)> {
        let mut joiner = Joiner::default();
        let mut out = Vec::new();
        for (start, end, text) in segments {
            out.extend(joiner.push(*start, *end, text));
        }
        out.extend(joiner.take());
        out.into_iter()
            .map(|o| (o.start_ms, o.end_ms, o.text))
            .collect()
    }

    #[test]
    fn segments_of_one_sentence_become_one_cue() {
        assert_eq!(
            joined(&[
                (0, 2000, "And so, my fellow Americans,"),
                (2000, 5000, "ask not what your country can do for you."),
                (5200, 7000, "Next one."),
            ]),
            vec![
                (
                    0,
                    5000,
                    "And so, my fellow Americans, ask not what your country can do for you.".into()
                ),
                (5200, 7000, "Next one.".into()),
            ]
        );
    }

    #[test]
    fn japanese_segments_join_without_a_space() {
        assert_eq!(
            joined(&[(0, 1000, "今日は"), (1000, 2000, "晴れです。")]),
            vec![(0, 2000, "今日は晴れです。".into())]
        );
    }

    #[test]
    fn a_pause_ends_a_cue_without_sentence_punctuation() {
        assert_eq!(
            joined(&[(0, 1000, "well"), (1000 + GAP_MS, 4000, "anyway")]),
            vec![
                (0, 1000, "well".into()),
                (1000 + GAP_MS, 4000, "anyway".into())
            ]
        );
    }

    #[test]
    fn a_cue_does_not_grow_without_end() {
        let segments: Vec<(u64, u64, &str)> = (0..20)
            .map(|i| (i * 2000, i * 2000 + 2000, "and then"))
            .collect();
        let cues = joined(&segments);
        assert!(cues.len() >= 3, "{cues:?}");
        assert!(cues.iter().all(|(s, e, _)| e - s <= MAX_CUE_MS + 2000));
    }

    #[test]
    fn cue_times_never_overlap() {
        // Whisper may date a segment before the end of the one before.
        let cues = joined(&[
            (0, 3000, "One."),
            (2500, 2600, "Two."),
            (2550, 5000, "Three."),
        ]);
        for pair in cues.windows(2) {
            assert!(pair[1].0 >= pair[0].1, "{cues:?}");
        }
        assert!(cues.iter().all(|(s, e, _)| e > s));
    }
}
