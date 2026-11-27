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
    /// End of the previous line; the next line never starts before it.
    last_end_ms: i64,
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
            SubtitleEvent::Started { device } => {
                *self.state.lock() = State {
                    source: device.clone(),
                    ..State::default()
                };
            }
            SubtitleEvent::Chunk {
                original,
                translated,
                at_ms,
                duration_ms,
            } => self.record_chunk(original, translated, *at_ms, *duration_ms),
            SubtitleEvent::Stopped | SubtitleEvent::Status { .. } | SubtitleEvent::Error { .. } => {
            }
        }
    }

    fn record_chunk(&self, original: &str, translated: &str, at_ms: u64, duration_ms: u64) {
        // Checked for every line, like every other history write:
        // turning history off stops the run in progress from saving too.
        let settings = self.settings.snapshot();
        if !settings.storage.keep_history {
            return;
        }
        let mut st = self.state.lock();
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
            Ok(_) => st.last_end_ms = end,
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
        SubtitleEvent::Chunk {
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
        });
        f.recorder.observe(&chunk("a", "b", 1000, 1000));
        assert!(f.storage.recent(10).unwrap().is_empty());
    }

    #[test]
    fn turning_history_off_stops_a_run_in_progress_from_saving() {
        let f = fixture();
        f.recorder.observe(&SubtitleEvent::Started {
            device: "mic".into(),
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
        });
        f.recorder.observe(&chunk("a", "A", 5_000, 4_000));
        // Starts before the previous end, then a zero-length one.
        f.recorder.observe(&chunk("b", "B", 6_000, 4_000));
        f.recorder.observe(&chunk("c", "C", 6_000, 0));
        // A duration longer than the run so far.
        let f2 = fixture();
        f2.recorder.observe(&SubtitleEvent::Started {
            device: "mic".into(),
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
        f.recorder
            .observe(&SubtitleEvent::Started { device: "A".into() });
        f.recorder.observe(&chunk("a", "A", 1000, 1000));
        let second = Recorder::new(f.storage.clone(), f.settings.clone());
        second.observe(&SubtitleEvent::Started { device: "B".into() });
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
}
