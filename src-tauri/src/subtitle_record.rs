//! History records of live-subtitle runs.
//!
//! [`Recorder`] watches the [`SubtitleEvent`]s of one recognition run and
//! saves them through [`Storage`] as one history row with timed lines.
//! The render functions turn a record into SRT or plain text for export.

use std::sync::Arc;

use parking_lot::Mutex;
use serde::Deserialize;

use crate::settings::SettingsStore;
use crate::storage::{Storage, SubtitleLine, SubtitleRecord};
use crate::subtitle::SubtitleEvent;
use crate::subtitle_revise::Revision;

/// Saves the lines of one run. The record is created at the first chunk,
/// so a run that recognized nothing leaves no history row.
pub struct Recorder {
    storage: Arc<Storage>,
    settings: Arc<SettingsStore>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    source: String,
    /// History id once the record exists.
    id: Option<i64>,
    /// Source and target language the record was created with.
    languages: (String, String),
    /// End of the previous line; the next line never starts before it.
    last_end_ms: i64,
    /// Event id of every saved line, in line order: where a later
    /// [`SubtitleEvent::Revised`] finds the lines it replaces. `None`
    /// for a line this run no longer knows (see [`State::forget`]).
    line_ids: Vec<Option<u64>>,
}

impl State {
    /// The record's lines were replaced from outside the run (a
    /// re-translation from the history page): `len` lines no event id
    /// names any more. Their corrections are dropped.
    fn forget(&mut self, len: usize) {
        self.line_ids = vec![None; len];
    }
}

impl Recorder {
    pub fn new(storage: Arc<Storage>, settings: Arc<SettingsStore>) -> Self {
        Self {
            storage,
            settings,
            state: Mutex::new(State::default()),
        }
    }

    /// Feed every event of the run, in order. Storage errors are logged
    /// and never reach the caller: subtitles keep running without history.
    pub fn observe(&self, event: &SubtitleEvent) {
        match event {
            SubtitleEvent::Started { device, .. } => {
                *self.state.lock() = State {
                    source: device.clone(),
                    ..State::default()
                };
            }
            SubtitleEvent::Chunk {
                id,
                original,
                translated,
                at_ms,
                duration_ms,
            } => self.record_chunk(*id, original, translated, *at_ms, *duration_ms),
            SubtitleEvent::Revised(revision) => self.record_revision(revision),
            SubtitleEvent::Stopped
            | SubtitleEvent::Status { .. }
            | SubtitleEvent::Candidate { .. }
            | SubtitleEvent::Error { .. } => {}
        }
    }

    /// A corrected sentence takes the place of the saved lines it was
    /// recognized in, and only when every one of them was saved: with
    /// history off for part of the sentence, its correction would bring
    /// that part in.
    fn record_revision(&self, revision: &Revision) {
        let mut st = self.state.lock();
        let Some(id) = st.id else { return };
        let at = |event: &u64| st.line_ids.iter().position(|l| *l == Some(*event));
        let found: Vec<usize> = revision.replaces.iter().filter_map(at).collect();
        let (Some(&first), Some(&last)) = (found.iter().min(), found.iter().max()) else {
            return;
        };
        let rest = revision.rest.as_ref();
        if found.len() < revision.replaces.len() {
            // The start of the next sentence stays in the last line of
            // this one: when that line was saved, the next correction
            // finds it there.
            let last_heard = revision.replaces.last().and_then(at);
            if let (Some(line), Some(rest)) = (last_heard, rest) {
                st.line_ids[line] = Some(rest.id);
            }
            return;
        }
        let lines = [
            Some((revision.id, &revision.original, &revision.translated)),
            rest.map(|r| (r.id, &r.original, &r.translated)),
        ];
        // An empty sentence is a blocked one: its lines just go.
        let lines: Vec<_> = lines
            .into_iter()
            .flatten()
            .filter(|(_, original, _)| !original.is_empty())
            .collect();
        let texts: Vec<(&str, &str)> = lines
            .iter()
            .map(|(_, original, translated)| (original.as_str(), translated.as_str()))
            .collect();
        let known = st.line_ids.len();
        match self
            .storage
            .revise_subtitle_lines(id, first as i64, last as i64, &texts, known)
        {
            Ok(()) => {
                st.line_ids
                    .splice(first..=last, lines.iter().map(|(event, ..)| Some(*event)));
            }
            Err(e) => {
                tracing::warn!("subtitle history: revising lines failed: {e:#}");
                if let Ok(Some(record)) = self.storage.subtitle_record(id) {
                    st.forget(record.lines.len());
                }
            }
        }
    }

    fn record_chunk(
        &self,
        event: u64,
        original: &str,
        translated: &str,
        at_ms: u64,
        duration_ms: u64,
    ) {
        // Checked for every line, like every other history write:
        // turning history off stops the run in progress from saving too.
        let settings = self.settings.snapshot();
        if !settings.storage.keep_history {
            return;
        }
        let mut st = self.state.lock();
        // One record holds one language pair (a later re-translation
        // uses it for every line): a change starts the next record.
        let languages = (
            settings.subtitle.source_language.clone(),
            settings.subtitle.target_language.clone(),
        );
        if st.id.is_some() && st.languages != languages {
            st.id = None;
            st.line_ids.clear();
        }
        let Some(id) = st.id.or_else(|| self.begin(&mut st, &settings)) else {
            return;
        };
        let end = i64::try_from(at_ms).unwrap_or(i64::MAX);
        let start = end
            .saturating_sub(i64::try_from(duration_ms).unwrap_or(i64::MAX))
            .max(st.last_end_ms);
        let end = end.max(start.saturating_add(1));
        match self
            .storage
            .append_subtitle_line(id, start, end, original, translated)
        {
            Ok(line) => {
                st.last_end_ms = end;
                if usize::try_from(line.seq) != Ok(st.line_ids.len()) {
                    st.forget(usize::try_from(line.seq).unwrap_or_default());
                }
                st.line_ids.push(Some(event));
            }
            Err(e) => tracing::warn!("subtitle history: appending a line failed: {e:#}"),
        }
    }

    /// Create the record, then apply the retention settings like every
    /// other history write.
    fn begin(&self, st: &mut State, s: &crate::settings::Settings) -> Option<i64> {
        let id = match self.storage.begin_subtitle_record(
            &st.source,
            &s.subtitle.source_language,
            &s.subtitle.target_language,
        ) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("subtitle history: creating the record failed: {e:#}");
                return None;
            }
        };
        st.id = Some(id);
        st.languages = (
            s.subtitle.source_language.clone(),
            s.subtitle.target_language.clone(),
        );
        if let Err(e) = self
            .storage
            .apply_retention(s.storage.retention_days, s.storage.max_entries)
        {
            tracing::warn!("history retention failed: {e}");
        }
        Some(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Srt,
    Txt,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Srt => "srt",
            Self::Txt => "txt",
        }
    }
}

/// Which text of a line an export carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportContent {
    Translated,
    Original,
    /// The original line, then the translated one.
    Both,
}

pub fn render(record: &SubtitleRecord, format: ExportFormat, content: ExportContent) -> String {
    match format {
        ExportFormat::Srt => render_srt(&record.lines, content),
        ExportFormat::Txt => render_text(&record.lines, content),
    }
}

/// SRT cues, numbered from 1 without gaps. Lines with no text to show
/// are skipped.
pub fn render_srt(lines: &[SubtitleLine], content: ExportContent) -> String {
    let cues = lines
        .iter()
        .filter_map(|l| Some((l, text_lines(l, content)?)));
    let mut out = Vec::new();
    for (index, (line, text)) in cues.enumerate() {
        out.push(format!(
            "{}\n{} --> {}\n{}\n",
            index + 1,
            srt_time(line.start_ms),
            srt_time(line.end_ms),
            text.join("\n")
        ));
    }
    out.join("\n")
}

/// Plain text: one line per cue (`both`: original, translation and a
/// blank line).
pub fn render_text(lines: &[SubtitleLine], content: ExportContent) -> String {
    let blocks: Vec<String> = lines
        .iter()
        .filter_map(|l| text_lines(l, content))
        .map(|text| match content {
            ExportContent::Both => format!("{}\n", text.join("\n")),
            _ => format!("{}\n", text.join(" ")),
        })
        .collect();
    match content {
        ExportContent::Both => blocks.join("\n"),
        _ => blocks.concat(),
    }
}

/// The text of a line for `content`, as non-empty trimmed rows. The
/// other text stands in for an empty one; `None` when neither has any.
fn text_lines(line: &SubtitleLine, content: ExportContent) -> Option<Vec<String>> {
    let rows = |text: &str| -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(str::to_string)
            .collect()
    };
    let (original, translated) = (rows(&line.original), rows(&line.translated));
    let chosen = match content {
        ExportContent::Translated if !translated.is_empty() => translated,
        ExportContent::Translated => original,
        ExportContent::Original if !original.is_empty() => original,
        ExportContent::Original => translated,
        ExportContent::Both => [original, translated].concat(),
    };
    (!chosen.is_empty()).then_some(chosen)
}

fn srt_time(ms: i64) -> String {
    let ms = ms.max(0);
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

/// Default name offered by the save dialog, from the row's creation time
/// in local time: `moonlens-字幕-20250131-0942.srt`.
pub fn export_file_name(created_at: i64, format: ExportFormat) -> String {
    let local = chrono::DateTime::from_timestamp(created_at, 0)
        .unwrap_or_default()
        .with_timezone(&chrono::Local)
        .naive_local();
    file_name_at(local, format)
}

fn file_name_at(at: chrono::NaiveDateTime, format: ExportFormat) -> String {
    format!(
        "moonlens-字幕-{}.{}",
        at.format("%Y%m%d-%H%M"),
        format.extension()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioSource;
    use crate::subtitle::Phase;

    fn line(start_ms: i64, end_ms: i64, original: &str, translated: &str) -> SubtitleLine {
        SubtitleLine {
            seq: 0,
            start_ms,
            end_ms,
            original: original.into(),
            translated: translated.into(),
        }
    }

    fn chunk(original: &str, translated: &str, at_ms: u64, duration_ms: u64) -> SubtitleEvent {
        numbered(0, original, translated, at_ms, duration_ms)
    }

    fn numbered(
        id: u64,
        original: &str,
        translated: &str,
        at_ms: u64,
        duration_ms: u64,
    ) -> SubtitleEvent {
        SubtitleEvent::Chunk {
            id,
            original: original.into(),
            translated: translated.into(),
            at_ms,
            duration_ms,
        }
    }

    struct Fixture {
        storage: Arc<Storage>,
        settings: Arc<SettingsStore>,
        recorder: Recorder,
        _dir: tempfile::TempDir,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::open_in_memory().unwrap());
        let settings = Arc::new(SettingsStore::load_from(dir.path().join("config.toml")).unwrap());
        let recorder = Recorder::new(storage.clone(), settings.clone());
        Fixture {
            storage,
            settings,
            recorder,
            _dir: dir,
        }
    }

    fn only_record(storage: &Storage) -> SubtitleRecord {
        let rows = storage.recent(10).unwrap();
        assert_eq!(rows.len(), 1, "exactly one history row");
        storage.subtitle_record(rows[0].id).unwrap().unwrap()
    }

    #[test]
    fn a_run_becomes_one_record_and_exports_as_srt() {
        let f = fixture();
        for ev in [
            SubtitleEvent::Started {
                device: "スピーカー (Realtek)".into(),
                source: AudioSource::System,
            },
            SubtitleEvent::Status {
                phase: Phase::Recognizing,
            },
            chunk("Hello there.", "こんにちは。", 3_000, 3_000),
            SubtitleEvent::Status {
                phase: Phase::Waiting,
            },
            // The lookback overlaps the previous chunk: start is clamped.
            chunk("How are you?", "お元気ですか。", 6_500, 3_800),
            // Translation failed.
            chunk("See you.", "", 83_000, 2_000),
            SubtitleEvent::Stopped,
        ] {
            f.recorder.observe(&ev);
        }
        let rec = only_record(&f.storage);
        assert_eq!(rec.source, "スピーカー (Realtek)");
        assert_eq!(rec.lines.len(), 3);
        let row = f.storage.entry(rec.history_id).unwrap().unwrap();
        assert_eq!(row.kind, "subtitle");
        assert_eq!(
            row.source_text.as_deref(),
            Some("Hello there.\nHow are you?\nSee you.")
        );
        assert_eq!(
            row.result_text.as_deref(),
            Some("こんにちは。\nお元気ですか。")
        );
        assert_eq!(
            render(&rec, ExportFormat::Srt, ExportContent::Translated),
            "1\n00:00:00,000 --> 00:00:03,000\nこんにちは。\n\n\
             2\n00:00:03,000 --> 00:00:06,500\nお元気ですか。\n\n\
             3\n00:01:21,000 --> 00:01:23,000\nSee you.\n"
        );
    }

    #[test]
    fn a_run_that_recognized_nothing_leaves_no_history_row() {
        let f = fixture();
        for ev in [
            SubtitleEvent::Started {
                device: "mic".into(),
                source: AudioSource::System,
            },
            SubtitleEvent::Status {
                phase: Phase::Waiting,
            },
            SubtitleEvent::Error {
                message: "x".into(),
            },
            SubtitleEvent::Stopped,
        ] {
            f.recorder.observe(&ev);
        }
        assert!(f.storage.recent(10).unwrap().is_empty());
    }

    #[test]
    fn history_off_records_nothing() {
        let f = fixture();
        let mut s = (*f.settings.snapshot()).clone();
        s.storage.keep_history = false;
        f.settings.save(s).unwrap();
        f.recorder.observe(&SubtitleEvent::Started {
            device: "mic".into(),
            source: AudioSource::System,
        });
        f.recorder.observe(&chunk("a", "b", 1000, 1000));
        assert!(f.storage.recent(10).unwrap().is_empty());
    }

    #[test]
    fn turning_history_off_stops_a_run_in_progress_from_saving() {
        let f = fixture();
        f.recorder.observe(&SubtitleEvent::Started {
            device: "mic".into(),
            source: AudioSource::System,
        });
        f.recorder.observe(&chunk("kept", "保持", 1000, 1000));
        let mut s = (*f.settings.snapshot()).clone();
        s.storage.keep_history = false;
        f.settings.save(s).unwrap();
        f.recorder.observe(&chunk("private", "非保存", 2000, 1000));
        let rec = only_record(&f.storage);
        assert_eq!(rec.lines.len(), 1);
        assert_eq!(rec.lines[0].original, "kept");
    }

    #[test]
    fn languages_come_from_the_settings_at_creation() {
        let f = fixture();
        let mut s = (*f.settings.snapshot()).clone();
        s.subtitle.source_language = "en".into();
        s.subtitle.target_language = "ja".into();
        f.settings.save(s).unwrap();
        f.recorder.observe(&SubtitleEvent::Started {
            device: "mic".into(),
            source: AudioSource::System,
        });
        f.recorder.observe(&chunk("a", "b", 1000, 1000));
        let rec = only_record(&f.storage);
        assert_eq!(
            (rec.source_language.as_str(), rec.target_language.as_str()),
            ("en", "ja")
        );
    }

    #[test]
    fn retention_applies_after_a_record_is_created() {
        let f = fixture();
        let mut s = (*f.settings.snapshot()).clone();
        s.storage.max_entries = 1;
        f.settings.save(s).unwrap();
        let old = f
            .storage
            .append("text", Some("old"), None, None, false, None)
            .unwrap();
        f.recorder.observe(&SubtitleEvent::Started {
            device: "mic".into(),
            source: AudioSource::System,
        });
        f.recorder.observe(&chunk("a", "b", 1000, 1000));
        let rows = f.storage.recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_ne!(rows[0].id, old);
        assert_eq!(rows[0].kind, "subtitle");
    }

    #[test]
    fn timing_never_overlaps_and_never_collapses() {
        let f = fixture();
        f.recorder.observe(&SubtitleEvent::Started {
            device: "mic".into(),
            source: AudioSource::System,
        });
        f.recorder.observe(&chunk("a", "A", 5_000, 4_000));
        // Starts before the previous end, then a zero-length one.
        f.recorder.observe(&chunk("b", "B", 6_000, 4_000));
        f.recorder.observe(&chunk("c", "C", 6_000, 0));
        // A duration longer than the run so far.
        let f2 = fixture();
        f2.recorder.observe(&SubtitleEvent::Started {
            device: "mic".into(),
            source: AudioSource::System,
        });
        f2.recorder.observe(&chunk("x", "X", 1_000, 9_000));
        let spans = |r: SubtitleRecord| -> Vec<(i64, i64)> {
            r.lines.iter().map(|l| (l.start_ms, l.end_ms)).collect()
        };
        assert_eq!(
            spans(only_record(&f.storage)),
            vec![(1_000, 5_000), (5_000, 6_000), (6_000, 6_001)]
        );
        assert_eq!(spans(only_record(&f2.storage)), vec![(0, 1_000)]);
    }

    #[test]
    fn a_new_run_starts_its_own_record() {
        let f = fixture();
        f.recorder.observe(&SubtitleEvent::Started {
            device: "A".into(),
            source: AudioSource::System,
        });
        f.recorder.observe(&chunk("a", "A", 1000, 1000));
        let second = Recorder::new(f.storage.clone(), f.settings.clone());
        second.observe(&SubtitleEvent::Started {
            device: "B".into(),
            source: AudioSource::System,
        });
        second.observe(&chunk("b", "B", 1000, 1000));
        let rows = f.storage.recent(10).unwrap();
        assert_eq!(rows.len(), 2);
        let sources: Vec<String> = rows
            .iter()
            .map(|r| f.storage.subtitle_record(r.id).unwrap().unwrap().source)
            .collect();
        assert_eq!(sources, vec!["B", "A"]);
    }

    fn sample() -> Vec<SubtitleLine> {
        vec![
            line(0, 1_500, "Hello", "こんにちは"),
            line(1_500, 3_661_042, "Bye", ""),
            line(4_000, 5_000, "", ""),
            line(5_000, 6_000, "", "訳のみ"),
        ]
    }

    #[test]
    fn srt_translated_falls_back_to_original_and_skips_empty_lines() {
        assert_eq!(
            render_srt(&sample(), ExportContent::Translated),
            "1\n00:00:00,000 --> 00:00:01,500\nこんにちは\n\n\
             2\n00:00:01,500 --> 01:01:01,042\nBye\n\n\
             3\n00:00:05,000 --> 00:00:06,000\n訳のみ\n"
        );
    }

    #[test]
    fn srt_original_falls_back_to_translated() {
        assert_eq!(
            render_srt(&sample(), ExportContent::Original),
            "1\n00:00:00,000 --> 00:00:01,500\nHello\n\n\
             2\n00:00:01,500 --> 01:01:01,042\nBye\n\n\
             3\n00:00:05,000 --> 00:00:06,000\n訳のみ\n"
        );
    }

    #[test]
    fn srt_both_puts_the_original_first() {
        assert_eq!(
            render_srt(&sample(), ExportContent::Both),
            "1\n00:00:00,000 --> 00:00:01,500\nHello\nこんにちは\n\n\
             2\n00:00:01,500 --> 01:01:01,042\nBye\n\n\
             3\n00:00:05,000 --> 00:00:06,000\n訳のみ\n"
        );
    }

    #[test]
    fn srt_never_contains_a_blank_line_inside_a_cue() {
        let lines = [line(0, 1_000, "a\n\n  b  ", "")];
        assert_eq!(
            render_srt(&lines, ExportContent::Original),
            "1\n00:00:00,000 --> 00:00:01,000\na\nb\n"
        );
    }

    #[test]
    fn srt_of_nothing_is_empty() {
        assert_eq!(render_srt(&[], ExportContent::Both), "");
        assert_eq!(render_srt(&[line(0, 1, "", "")], ExportContent::Both), "");
    }

    #[test]
    fn text_has_one_line_per_cue() {
        assert_eq!(
            render_text(&sample(), ExportContent::Translated),
            "こんにちは\nBye\n訳のみ\n"
        );
        assert_eq!(
            render_text(&sample(), ExportContent::Original),
            "Hello\nBye\n訳のみ\n"
        );
        assert_eq!(
            render_text(&[line(0, 1, "a\nb", "")], ExportContent::Original),
            "a b\n"
        );
    }

    #[test]
    fn text_both_separates_cues_with_a_blank_line() {
        assert_eq!(
            render_text(&sample(), ExportContent::Both),
            "Hello\nこんにちは\n\nBye\n\n訳のみ\n"
        );
        assert_eq!(render_text(&[], ExportContent::Both), "");
    }

    #[test]
    fn export_formats_and_contents_parse_from_the_ipc_strings() {
        let f: ExportFormat = serde_json::from_str("\"srt\"").unwrap();
        let c: ExportContent = serde_json::from_str("\"both\"").unwrap();
        assert_eq!((f, c), (ExportFormat::Srt, ExportContent::Both));
        assert!(serde_json::from_str::<ExportFormat>("\"pdf\"").is_err());
    }

    #[test]
    fn default_file_name_uses_date_and_minute() {
        let at = chrono::NaiveDate::from_ymd_opt(2025, 1, 31)
            .unwrap()
            .and_hms_opt(9, 42, 7)
            .unwrap();
        assert_eq!(
            file_name_at(at, ExportFormat::Srt),
            "moonlens-字幕-20250131-0942.srt"
        );
        assert_eq!(
            file_name_at(at, ExportFormat::Txt),
            "moonlens-字幕-20250131-0942.txt"
        );
        assert!(export_file_name(1_700_000_000, ExportFormat::Txt).starts_with("moonlens-字幕-"));
    }
    fn revised(
        replaces: &[u64],
        id: u64,
        original: &str,
        translated: &str,
        rest: Option<(u64, &str)>,
    ) -> SubtitleEvent {
        SubtitleEvent::Revised(Revision {
            replaces: replaces.to_vec(),
            id,
            original: original.into(),
            translated: translated.into(),
            rest: rest.map(|(id, original)| crate::subtitle_revise::RevisedRest {
                id,
                original: original.into(),
                translated: String::new(),
            }),
        })
    }

    fn keep_history(f: &Fixture, keep: bool) {
        let mut s = (*f.settings.snapshot()).clone();
        s.storage.keep_history = keep;
        f.settings.save(s).unwrap();
    }

    fn originals(storage: &Storage) -> Vec<String> {
        let lines = only_record(storage).lines;
        lines.into_iter().map(|l| l.original).collect()
    }

    #[test]
    fn a_corrected_sentence_replaces_the_lines_it_was_heard_in() {
        let f = fixture();
        f.recorder
            .observe(&numbered(1, "I went to", "行った", 2_000, 2_000));
        f.recorder
            .observe(&numbered(2, "the shop. Then", "店。それから", 4_000, 2_000));
        f.recorder.observe(&revised(
            &[1, 2],
            3,
            "I went to the shop.",
            "店に行った。",
            Some((4, "Then")),
        ));
        let rec = only_record(&f.storage);
        assert_eq!(rec.lines.len(), 2);
        assert_eq!(rec.lines[0].translated, "店に行った。");
        assert_eq!((rec.lines[0].start_ms, rec.lines[1].end_ms), (0, 4_000));
        assert_eq!(rec.lines[0].end_ms, rec.lines[1].start_ms);
        assert_eq!(rec.lines[1].original, "Then");

        // The rest and a new chunk become the next sentence.
        f.recorder
            .observe(&numbered(5, "I came home.", "帰った", 6_000, 2_000));
        f.recorder.observe(&revised(
            &[4, 5],
            6,
            "Then I came home.",
            "帰宅した。",
            None,
        ));
        assert_eq!(
            originals(&f.storage),
            ["I went to the shop.", "Then I came home."]
        );
        let row = f.storage.recent(1).unwrap().remove(0);
        assert_eq!(row.result_text.as_deref(), Some("店に行った。\n帰宅した。"));
    }

    #[test]
    fn a_blocked_sentence_leaves_the_record_and_unsaved_lines_stay_out() {
        let f = fixture();
        f.recorder
            .observe(&numbered(1, "Hello.", "こんにちは。", 2_000, 2_000));
        f.recorder
            .observe(&numbered(2, "Thanks for watching.", "", 4_000, 2_000));
        f.recorder.observe(&revised(&[2], 3, "", "", None));
        assert_eq!(originals(&f.storage), ["Hello."]);
        // Not one of the saved lines: nothing changes.
        f.recorder
            .observe(&revised(&[9], 10, "Other.", "他。", None));
        assert_eq!(originals(&f.storage), ["Hello."]);
        // The next chunk still lands after the remaining line.
        f.recorder
            .observe(&numbered(11, "Bye.", "では。", 6_000, 2_000));
        f.recorder
            .observe(&revised(&[11], 12, "Goodbye.", "さようなら。", None));
        assert_eq!(originals(&f.storage), ["Hello.", "Goodbye."]);
    }
    #[test]
    fn a_sentence_heard_partly_without_history_is_not_corrected() {
        let f = fixture();
        f.recorder
            .observe(&numbered(1, "My name", "私の名前", 2_000, 2_000));
        keep_history(&f, false);
        f.recorder
            .observe(&numbered(2, "is secret", "秘密", 4_000, 2_000));
        keep_history(&f, true);
        f.recorder
            .observe(&numbered(3, "today. Next", "今日。次", 6_000, 2_000));
        f.recorder.observe(&revised(
            &[1, 2, 3],
            4,
            "My name is secret today.",
            "秘密",
            Some((5, "Next")),
        ));
        assert_eq!(originals(&f.storage), ["My name", "today. Next"]);
        // The next sentence, heard with history on, is corrected again.
        f.recorder
            .observe(&numbered(6, "one.", "一。", 8_000, 2_000));
        f.recorder
            .observe(&revised(&[5, 6], 7, "Next one.", "次の一。", None));
        assert_eq!(originals(&f.storage), ["My name", "Next one."]);
    }

    #[test]
    fn lines_replaced_from_outside_are_not_touched_by_later_corrections() {
        let f = fixture();
        f.recorder.observe(&numbered(1, "a", "A", 2_000, 2_000));
        f.recorder.observe(&numbered(2, "b", "B", 4_000, 2_000));
        f.recorder.observe(&numbered(3, "c", "C", 6_000, 2_000));
        // A re-translation from the history page joins the first two.
        let rec = only_record(&f.storage);
        let joined = [line(0, 4_000, "a b", "AB"), rec.lines[2].clone()];
        f.storage
            .replace_subtitle_lines(rec.history_id, rec.revision, &joined)
            .unwrap();
        // Line 3 is now at another place: its correction is dropped.
        f.recorder.observe(&revised(&[3], 4, "see", "見る", None));
        assert_eq!(originals(&f.storage), ["a b", "c"]);
        // Lines of the run from here on are corrected as before.
        f.recorder.observe(&numbered(5, "d", "D", 8_000, 2_000));
        f.recorder.observe(&revised(&[5], 6, "dee", "ディー", None));
        assert_eq!(originals(&f.storage), ["a b", "c", "dee"]);
    }

    #[test]
    fn a_line_appended_after_an_outside_replacement_is_still_corrected() {
        let f = fixture();
        f.recorder.observe(&numbered(1, "a", "A", 2_000, 2_000));
        f.recorder.observe(&numbered(2, "b", "B", 4_000, 2_000));
        let rec = only_record(&f.storage);
        f.storage
            .replace_subtitle_lines(rec.history_id, rec.revision, &[line(0, 4_000, "a b", "AB")])
            .unwrap();
        f.recorder.observe(&numbered(3, "c", "C", 6_000, 2_000));
        f.recorder.observe(&revised(&[3], 4, "see", "見る", None));
        f.recorder.observe(&revised(&[2], 5, "bee", "蜂", None));
        assert_eq!(originals(&f.storage), ["a b", "see"]);
    }
    #[test]
    fn a_changed_language_pair_starts_the_next_record() {
        let f = fixture();
        f.recorder
            .observe(&numbered(1, "Hello.", "こんにちは。", 2_000, 2_000));
        let mut s = (*f.settings.snapshot()).clone();
        let first = s.subtitle.target_language.clone();
        s.subtitle.target_language = "ko".into();
        f.settings.save(s).unwrap();
        f.recorder
            .observe(&numbered(2, "Bye.", "안녕.", 4_000, 2_000));
        // A correction of the earlier record's line is dropped.
        f.recorder
            .observe(&revised(&[1], 3, "Hello!", "やあ。", None));

        let rows = f.storage.recent(10).unwrap();
        assert_eq!(rows.len(), 2);
        let record = |id| f.storage.subtitle_record(id).unwrap().unwrap();
        let (new, old) = (record(rows[0].id), record(rows[1].id));
        assert_eq!((old.target_language, old.lines.len()), (first, 1));
        assert_eq!(old.lines[0].original, "Hello.");
        assert_eq!(new.target_language, "ko");
        assert_eq!(new.lines[0].original, "Bye.");
        assert_eq!(new.lines[0].start_ms, 2_000);
    }
}
