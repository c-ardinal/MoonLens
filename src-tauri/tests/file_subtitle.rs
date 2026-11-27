//! Subtitles for media files, from the file on disk to the saved
//! record: real containers and codecs, scripted speech recognition.
//!
//! The memory tests count every allocation of the process: a job on a
//! file of hours must stay within the same few megabytes as one on a
//! file of minutes.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use async_trait::async_trait;
use moonlens_lib::{
    file_subtitle::{Engine, FileSubtitleService, Phase, Status, Update, WINDOW},
    media_audio::{MediaAudio, OpenError, MAX_BLOCK, RATE},
    pipeline::Pipeline,
    providers::TranscriptSegment,
    settings::{schema::Settings, SettingsStore},
    storage::Storage,
    subtitle_record::{ExportContent, ExportFormat},
};

/// Counts the bytes allocated through Rust's allocator.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(p, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The memory tests measure the whole process: one at a time, and not
/// beside the other tests of this file.
static ALONE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/media")
        .join(name)
}

fn decode(path: &Path) -> (Option<u64>, Vec<i16>) {
    let mut audio = MediaAudio::open(path).unwrap();
    let duration = audio.duration_ms();
    let mut samples = Vec::new();
    while let Some(block) = audio.read() {
        assert!(!block.is_empty() && block.len() <= MAX_BLOCK * 2);
        samples.extend(block);
    }
    (duration, samples)
}

fn rms(samples: &[i16]) -> f64 {
    let sum: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum / samples.len().max(1) as f64).sqrt()
}

#[test]
fn the_audio_of_a_video_is_decoded_to_16_khz_mono() {
    for name in ["jfk-1080p.mp4", "jfk.mkv"] {
        let (duration, samples) = decode(&fixture(name));
        // 11 s of speech; encoders pad the ends by a few frames.
        let seconds = samples.len() as f64 / RATE as f64;
        assert!((10.8..11.3).contains(&seconds), "{name}: {seconds} s");
        let stated = duration.expect("the container states the length") as f64 / 1000.0;
        assert!((10.8..11.3).contains(&stated), "{name}: {stated} s stated");
        assert!(rms(&samples) > 500.0, "{name}: speech, not silence");
    }
}

#[test]
fn the_speech_keeps_its_place_in_time() {
    // The loudest second of the original is the loudest of the decoded
    // audio of each video: nothing is shifted or stretched.
    let loudest = |samples: &[i16]| {
        (0..samples.len() / RATE)
            .max_by_key(|s| rms(&samples[s * RATE..(s + 1) * RATE]) as u64)
            .unwrap()
    };
    let original = decode(&fixture("../audio/jfk.wav")).1;
    for name in ["jfk-1080p.mp4", "jfk.mkv"] {
        assert_eq!(
            loudest(&decode(&fixture(name)).1),
            loudest(&original),
            "{name}"
        );
    }
}

#[test]
fn files_without_usable_audio_are_refused_with_the_reason() {
    let open = |path: &Path| MediaAudio::open(path).err();
    assert_eq!(open(&fixture("silent-video.mp4")), Some(OpenError::NoAudio));
    assert_eq!(
        open(&fixture("jfk-opus.webm")),
        Some(OpenError::UnsupportedCodec)
    );
    assert_eq!(open(&fixture("README.md")), Some(OpenError::UnknownFormat));
    assert!(matches!(
        open(&fixture("missing.mp4")),
        Some(OpenError::Io(_))
    ));
}

/// One sentence per recognition, numbered by call.
#[derive(Default)]
struct Stub {
    calls: AtomicUsize,
    longest: AtomicUsize,
}

#[async_trait]
impl Engine for Stub {
    async fn recognize(&self, pcm: &[i16]) -> Result<Vec<TranscriptSegment>, String> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        self.longest.fetch_max(pcm.len(), Ordering::SeqCst);
        Ok(vec![TranscriptSegment {
            start_s: 0.5,
            end_s: 2.5,
            text: format!("Sentence {n}."),
        }])
    }

    async fn translate(&self, text: &str, _history: &[(String, String)]) -> Option<String> {
        Some(format!("訳：{text}"))
    }
}

struct Job {
    service: FileSubtitleService,
    storage: Arc<Storage>,
    updates: Arc<Mutex<Vec<Update>>>,
    _dir: tempfile::TempDir,
}

/// Run the job on `path` to its end.
async fn job(path: &Path, configure: impl FnOnce(&mut Settings)) -> (Job, Status) {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::load_from(dir.path().join("config.toml")).unwrap();
    let mut s = Settings::default();
    configure(&mut s);
    store.save(s).unwrap();
    let settings = Arc::new(store);
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let pipeline = Arc::new(Pipeline::with_providers(
        settings.clone(),
        storage.clone(),
        HashMap::new(),
    ));
    let service = FileSubtitleService::new(settings, pipeline, storage.clone());
    let updates = Arc::new(Mutex::new(Vec::new()));
    let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel();
    let notify = {
        let updates = updates.clone();
        Arc::new(move |update: Update| {
            if update.status.phase != Phase::Running {
                let _ = done_tx.send(update.status.clone());
            }
            updates.lock().unwrap().push(update);
        })
    };
    let started = service.start_with(path, Stub::default(), notify).unwrap();
    assert_eq!(started.phase, Phase::Running);
    let last = done_rx.recv().await.unwrap();
    let job = Job {
        service,
        storage,
        updates,
        _dir: dir,
    };
    (job, last)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_video_becomes_a_saved_record_and_an_srt_file() {
    let _alone = ALONE.lock().await;
    let (job, last) = job(&fixture("jfk-1080p.mp4"), |_| {}).await;
    assert_eq!(last.phase, Phase::Done);
    assert_eq!(last.name, "jfk-1080p.mp4");
    assert_eq!(last.cues, 1);
    assert_eq!(job.service.status(), last);
    assert!((10_800..11_300).contains(&last.done_ms), "{}", last.done_ms);

    // The page heard the subtitle as it was made.
    let updates = job.updates.lock().unwrap();
    let cue = updates.iter().find_map(|u| u.cue.clone()).unwrap();
    assert_eq!((cue.start_ms, cue.end_ms), (500, 2500));
    assert_eq!(cue.translated, "訳：Sentence 0.");

    let export = job.service.export(ExportFormat::Srt, ExportContent::Both);
    assert_eq!(export.name, "jfk-1080p.ja.srt");
    assert_eq!(export.folder.as_deref(), fixture("jfk-1080p.mp4").parent());
    assert_eq!(
        export.text,
        "1\n00:00:00,500 --> 00:00:02,500\nSentence 0.\n訳：Sentence 0.\n"
    );
    let original = job
        .service
        .export(ExportFormat::Txt, ExportContent::Original);
    assert_eq!(original.name, "jfk-1080p.txt");

    // History holds it like a live subtitle run, under the file name.
    let entry = &job.storage.recent(10).unwrap()[0];
    let record = job.storage.subtitle_record(entry.id).unwrap().unwrap();
    assert_eq!(record.source, "jfk-1080p.mp4");
    assert_eq!(record.lines.len(), 1);
    assert_eq!(record.lines[0].original, "Sentence 0.");
    assert_eq!(
        (record.lines[0].start_ms, record.lines[0].end_ms),
        (500, 2500)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn history_off_keeps_the_subtitles_for_export_only() {
    let _alone = ALONE.lock().await;
    let (job, last) = job(&fixture("jfk.mkv"), |s| s.storage.keep_history = false).await;
    assert_eq!(last.phase, Phase::Done);
    assert!(job.storage.recent(10).unwrap().is_empty());
    let export = job
        .service
        .export(ExportFormat::Srt, ExportContent::Translated);
    assert!(export.text.contains("訳：Sentence 0."), "{}", export.text);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_that_cannot_be_used_does_not_start_a_job() {
    let _alone = ALONE.lock().await;
    let (job, _) = job(&fixture("jfk.mkv"), |_| {}).await;
    let refused = job.service.start_with(
        &fixture("silent-video.mp4"),
        Stub::default(),
        Arc::new(|_| {}),
    );
    assert!(refused.is_err());
    // The subtitles of the file before are still there.
    assert_eq!(job.service.status().phase, Phase::Done);
    assert_eq!(job.service.status().cues, 1);
}

/// Recognition that takes a moment, so that a job can be stopped
/// part-way.
struct Slow;

#[async_trait]
impl Engine for Slow {
    async fn recognize(&self, _pcm: &[i16]) -> Result<Vec<TranscriptSegment>, String> {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        Ok(vec![TranscriptSegment {
            start_s: 0.5,
            end_s: 2.5,
            text: "Slow sentence.".into(),
        }])
    }

    async fn translate(&self, text: &str, _history: &[(String, String)]) -> Option<String> {
        Some(format!("訳：{text}"))
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_job_keeps_its_subtitles_until_the_next_file() {
    let _alone = ALONE.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let long = long_wav(dir.path(), 3600);
    // Any finished job gives the service to work with.
    let (job, _) = job(&fixture("jfk.mkv"), |_| {}).await;

    let (cue_tx, mut cue_rx) = tokio::sync::mpsc::unbounded_channel();
    let notify = Arc::new(move |update: Update| {
        if update.cue.is_some() {
            let _ = cue_tx.send(());
        }
    });
    job.service.start_with(&long, Slow, notify).unwrap();
    assert!(job
        .service
        .start_with(&fixture("jfk.mkv"), Stub::default(), Arc::new(|_| {}))
        .is_err());
    cue_rx.recv().await.unwrap();
    cue_rx.recv().await.unwrap();
    let stopped = job.service.cancel();
    assert_eq!(stopped.phase, Phase::Cancelled);
    assert!(
        stopped.cues >= 2 && stopped.done_ms < 3_600_000,
        "{stopped:?}"
    );
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(
        job.service.status(),
        stopped,
        "nothing moves after the stop"
    );
    let export = job
        .service
        .export(ExportFormat::Srt, ExportContent::Translated);
    assert_eq!(
        export.text.matches("訳：Slow sentence.").count(),
        stopped.cues
    );

    // The next file starts from nothing.
    let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel();
    let notify = Arc::new(move |update: Update| {
        if update.status.phase == Phase::Done {
            let _ = done_tx.send(update.status.clone());
        }
    });
    job.service
        .start_with(&fixture("jfk-1080p.mp4"), Stub::default(), notify)
        .unwrap();
    let last = done_rx.recv().await.unwrap();
    assert_eq!((last.cues, last.name.as_str()), (1, "jfk-1080p.mp4"));
    let export = job
        .service
        .export(ExportFormat::Srt, ExportContent::Original);
    assert!(!export.text.contains("Slow"), "{}", export.text);
}

/// A WAV file of `seconds` of an audible tone (8 kHz, 8 bit, mono),
/// written piece by piece.
fn long_wav(dir: &Path, seconds: u32) -> PathBuf {
    const WAV_RATE: u32 = 8000;
    let path = dir.join(format!("tone-{seconds}.wav"));
    let mut file = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    let data = seconds * WAV_RATE;
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + data).to_le_bytes()).unwrap();
    file.write_all(b"WAVEfmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    file.write_all(&1u16.to_le_bytes()).unwrap(); // mono
    file.write_all(&WAV_RATE.to_le_bytes()).unwrap();
    file.write_all(&WAV_RATE.to_le_bytes()).unwrap(); // bytes per second
    file.write_all(&1u16.to_le_bytes()).unwrap(); // block align
    file.write_all(&8u16.to_le_bytes()).unwrap(); // bits per sample
    file.write_all(b"data").unwrap();
    file.write_all(&data.to_le_bytes()).unwrap();
    let second: Vec<u8> = (0..WAV_RATE)
        .map(|i| (128.0 + (i as f32 * 0.3).sin() * 40.0) as u8)
        .collect();
    for _ in 0..seconds {
        file.write_all(&second).unwrap();
    }
    file.flush().unwrap();
    path
}

/// Most bytes allocated at once while a job on `seconds` of audio
/// runs, over what was allocated before it, and its last status.
async fn peak_of_job(dir: &Path, seconds: u32) -> (usize, Status) {
    let path = long_wav(dir, seconds);
    let before = LIVE.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let (job, last) = job(&path, |_| {}).await;
    let peak = PEAK.load(Ordering::SeqCst).saturating_sub(before);
    drop(job);
    std::fs::remove_file(path).unwrap();
    (peak, last)
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_does_not_grow_with_the_length_of_the_file() {
    let _alone = ALONE.lock().await;
    let dir = tempfile::tempdir().unwrap();
    const MB: usize = 1024 * 1024;
    // One recognition window is 0.9 MB of samples.
    const { assert!(WINDOW * 2 < MB) };

    let (short, last) = peak_of_job(dir.path(), 10 * 60).await;
    assert_eq!(last.phase, Phase::Done);
    assert!(
        (599_990..=600_000).contains(&last.done_ms),
        "{}",
        last.done_ms
    );

    let (long, last) = peak_of_job(dir.path(), 3 * 3600).await;
    assert_eq!(last.phase, Phase::Done);
    assert!(
        (10_799_990..=10_800_000).contains(&last.done_ms),
        "{}",
        last.done_ms
    );
    assert!(last.cues >= 3 * 3600 / 28, "{} cues", last.cues);

    // The audio of three hours is 345 MB as 16 kHz samples.
    assert!(long < 16 * MB, "{} MB for three hours", long / MB);
    // 18 times the audio: what grows is the text of the subtitles.
    assert!(
        long < short + 2 * MB,
        "10 minutes: {short} bytes, 3 hours: {long} bytes"
    );
}
