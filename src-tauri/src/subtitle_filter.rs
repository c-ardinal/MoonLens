//! Keeps stock misrecognitions out of the live subtitles.
//!
//! Over silence and background music Whisper does not stay quiet: it
//! writes what captioned videos end with ("Thanks for watching", subtitle
//! credits) or a sound annotation ("[Music]"). [`StockFilter::screen`]
//! removes such sentences from a recognized chunk before it is shown or
//! translated, so a chunk that held nothing else costs no translation.
//!
//! What counts as stock: the bundled list (`subtitle_phrases.txt`), the
//! user's own list (`subtitle.blocked_phrases`), and whole-sentence sound
//! annotations. Nothing is added to the user's list automatically. A
//! short sentence that keeps coming back is only *suggested*
//! ([`Screened::candidate`]); the subtitle window asks, and the user
//! decides.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use crate::settings::schema::SubtitleSettings;

/// One language's section of the bundled list.
pub struct Bundled {
    pub lang: &'static str,
    pub phrases: Vec<&'static str>,
}

/// The bundled list, by language.
pub static BUNDLED: LazyLock<Vec<Bundled>> = LazyLock::new(|| {
    let mut out: Vec<Bundled> = Vec::new();
    for line in include_str!("subtitle_phrases.txt").lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(lang) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            out.push(Bundled {
                lang,
                phrases: Vec::new(),
            });
        } else if let Some(section) = out.last_mut() {
            section.phrases.push(line);
        }
    }
    out
});

/// What two spellings of one phrase share: letters and digits only,
/// lower case, full-width forms folded to ASCII. Punctuation, spacing and
/// symbols differ from one recognition to the next.
pub fn key(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| match c {
            // Full-width digits and Latin letters.
            '\u{FF10}'..='\u{FF5A}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            c => c,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

/// Split `text` into sentences. Every character stays in exactly one
/// slice (trailing spaces included), so kept slices concatenate back to
/// the original wording.
fn sentences(text: &str) -> Vec<&str> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].1;
        let next = chars.get(i + 1).map(|&(_, n)| n);
        let ends = match c {
            '。' | '！' | '？' | '…' => true,
            // "3.5" and "Amara.org" are not sentence ends.
            '.' | '!' | '?' => next.is_none_or(follows_an_end),
            _ => false,
        };
        if !ends {
            i += 1;
            continue;
        }
        // Closing punctuation and spaces belong to the sentence before.
        i += 1;
        while chars.get(i).is_some_and(|&(_, n)| follows_an_end(n)) {
            i += 1;
        }
        let end = chars.get(i).map_or(text.len(), |&(at, _)| at);
        out.push(&text[start..end]);
        start = end;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

fn follows_an_end(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '。' | '！' | '？' | '…' | '.' | '!' | '?' | '」' | '』' | '"' | '”'
        )
}

/// A sentence that is only a sound annotation or symbols: "[Music]",
/// "(拍手)", "♪♪", "*sighs*".
fn is_annotation(sentence: &str) -> bool {
    let s = sentence.trim().trim_end_matches(['.', '。', '!', '?']);
    if key(s).is_empty() {
        return true;
    }
    const PAIRS: [(char, char); 7] = [
        ('[', ']'),
        ('(', ')'),
        ('（', '）'),
        ('【', '】'),
        ('［', '］'),
        ('*', '*'),
        ('♪', '♪'),
    ];
    PAIRS.iter().any(|&(open, close)| {
        s.strip_prefix(open)
            .and_then(|r| r.strip_suffix(close))
            // One annotation, not "(a) real speech (b)".
            .is_some_and(|inner| !inner.contains([open, close]))
    })
}

/// The phrases to remove, as [`key`]s.
#[derive(Default)]
struct Blocklist {
    exact: HashSet<String>,
    prefixes: Vec<String>,
}

impl Blocklist {
    fn add(&mut self, phrase: &str) {
        let (phrase, prefix) = match phrase.trim().strip_suffix('*') {
            Some(p) => (p, true),
            None => (phrase, false),
        };
        let k = key(phrase);
        if k.is_empty() {
            return;
        }
        if prefix {
            self.prefixes.push(k);
        } else {
            self.exact.insert(k);
        }
    }

    fn new(user: &[String]) -> Self {
        let mut list = Self::default();
        for section in BUNDLED.iter() {
            for phrase in &section.phrases {
                list.add(phrase);
            }
        }
        // The user's entries are whole phrases: a `*` typed there is
        // punctuation like any other.
        for phrase in user {
            let k = key(phrase);
            if !k.is_empty() {
                list.exact.insert(k);
            }
        }
        list
    }

    fn has(&self, k: &str) -> bool {
        self.exact.contains(k) || self.prefixes.iter().any(|p| k.starts_with(p.as_str()))
    }
}

/// A recognized chunk after screening.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Screened {
    /// What is left to show and translate; empty when the chunk held
    /// nothing but stock phrases.
    pub text: String,
    /// The sentences that were removed.
    pub blocked: Vec<String>,
    /// A sentence that came back often enough to ask the user about.
    pub candidate: Option<String>,
}

/// Occurrences of one sentence within [`REPEAT_WINDOW`] that make it a
/// candidate.
const REPEAT_COUNT: usize = 3;
/// Repeats further apart than this are ordinary speech ("Yes." now and
/// again), not a recognizer stuck on silence.
const REPEAT_WINDOW: Duration = Duration::from_secs(90);
/// Longest sentence ([`key`] characters) worth suggesting: invented
/// phrases are short, and a long repeated sentence is content.
const REPEAT_MAX_CHARS: usize = 40;
/// Distinct sentences tracked at once.
const REPEAT_TRACKED: usize = 64;

/// Per-session screening state (one per subtitle loop).
#[derive(Default)]
pub struct StockFilter {
    /// The blocklist and the user list it was built from.
    list: Option<(Vec<String>, Blocklist)>,
    /// When each recent short sentence was heard, by key.
    heard: HashMap<String, VecDeque<Instant>>,
    /// Suggested once already in this session.
    suggested: HashSet<String>,
}

impl StockFilter {
    /// Screen one recognized chunk, given as its segments.
    pub fn screen(
        &mut self,
        settings: &SubtitleSettings,
        segments: &[&str],
        now: Instant,
    ) -> Screened {
        if !settings.block_stock_phrases {
            return Screened {
                text: join(segments.iter().copied()),
                ..Default::default()
            };
        }
        let (text, blocked, kept) = self.strip(settings, segments);

        // A candidate is a chunk that says one short thing and nothing
        // else, again and again. Sentences inside longer speech repeat
        // for honest reasons.
        let candidate = match kept.split_first() {
            Some(((k, sentence), others)) if others.iter().all(|(o, _)| o == k) => {
                self.note(k, now).then(|| sentence.clone())
            }
            _ => None,
        };
        Screened {
            text,
            blocked,
            candidate,
        }
    }

    /// Whether `text`, screened earlier, consists only of phrases blocked
    /// by now: the user may approve a candidate while its chunk is still
    /// being translated, and that chunk must not come back.
    pub fn blocks_all(&mut self, settings: &SubtitleSettings, text: &str) -> bool {
        settings.block_stock_phrases && self.strip(settings, &[text]).0.is_empty()
    }

    /// Remove the stock sentences from a chunk. Returns what is left,
    /// what was removed, and the kept sentences as (key, text).
    fn strip(
        &mut self,
        settings: &SubtitleSettings,
        segments: &[&str],
    ) -> (String, Vec<String>, Vec<(String, String)>) {
        if self
            .list
            .as_ref()
            .is_none_or(|(user, _)| user != &settings.blocked_phrases)
        {
            self.list = Some((
                settings.blocked_phrases.clone(),
                Blocklist::new(&settings.blocked_phrases),
            ));
        }
        let list = &self.list.as_ref().expect("built above").1;

        let mut blocked = Vec::new();
        let mut kept = Vec::new();
        let mut rest: Vec<String> = Vec::new();
        for unit in units(segments) {
            let mut left = String::new();
            for sentence in sentences(&unit) {
                let k = key(sentence);
                if is_annotation(sentence) || list.has(&k) {
                    if !k.is_empty() {
                        blocked.push(sentence.trim().to_string());
                    }
                } else {
                    left.push_str(sentence);
                    kept.push((k, sentence.trim().to_string()));
                }
            }
            rest.push(left);
        }
        (join(rest.iter().map(String::as_str)), blocked, kept)
    }

    /// Record that a chunk said only `k`; `true` when that makes it a
    /// candidate (once per session).
    fn note(&mut self, k: &str, now: Instant) -> bool {
        if k.chars().count() > REPEAT_MAX_CHARS || self.suggested.contains(k) {
            return false;
        }
        self.heard.retain(|_, times| {
            while times
                .front()
                .is_some_and(|&t| now.duration_since(t) > REPEAT_WINDOW)
            {
                times.pop_front();
            }
            !times.is_empty()
        });
        if self.heard.len() >= REPEAT_TRACKED && !self.heard.contains_key(k) {
            return false;
        }
        let times = self.heard.entry(k.to_string()).or_default();
        times.push_back(now);
        if times.len() < REPEAT_COUNT {
            return false;
        }
        self.heard.remove(k);
        self.suggested.insert(k.to_string());
        true
    }
}

/// Segments regrouped so that no sentence is cut in two: recognition
/// breaks segments at pauses, which may fall inside a sentence. A segment
/// that starts in lower case after one without closing punctuation
/// continues it ("Thanks for watching" + "the kids yesterday."); any
/// other boundary ends a sentence, as invented phrases usually arrive
/// unpunctuated in a segment of their own.
fn units(segments: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for segment in segments.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let continues = segment.chars().next().is_some_and(char::is_lowercase);
        match out.last_mut() {
            Some(last) if continues && !ends_a_sentence(last) => {
                last.push(' ');
                last.push_str(segment);
            }
            _ => out.push(segment.to_string()),
        }
    }
    out
}

fn ends_a_sentence(text: &str) -> bool {
    text.trim_end_matches(['」', '』', '"', '”', ']', ')', '）', '】', '♪'])
        .ends_with(['。', '！', '？', '…', '.', '!', '?'])
        || is_annotation(text)
}

/// Segments as one line, the way the subtitle shows them.
fn join<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Add `phrase` to the user's list unless it is there already (in any
/// spelling) or says nothing. Returns whether the list changed.
pub fn add_phrase(list: &mut Vec<String>, phrase: &str) -> bool {
    let phrase = phrase.trim();
    let k = key(phrase);
    if k.is_empty() || list.iter().any(|p| key(p) == k) {
        return false;
    }
    list.push(phrase.to_string());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> SubtitleSettings {
        SubtitleSettings::default()
    }

    fn screen(segments: &[&str]) -> Screened {
        StockFilter::default().screen(&on(), segments, Instant::now())
    }

    #[test]
    fn every_bundled_language_has_phrases() {
        let langs: Vec<_> = BUNDLED.iter().map(|b| b.lang).collect();
        for lang in ["en", "ja", "zh", "ko", "de", "fr", "es", "pt", "it", "ru"] {
            assert!(langs.contains(&lang), "{lang} missing from {langs:?}");
        }
        for section in BUNDLED.iter() {
            assert!(!section.phrases.is_empty(), "[{}] is empty", section.lang);
            for phrase in &section.phrases {
                assert!(
                    !key(phrase).is_empty(),
                    "[{}] {phrase:?} says nothing",
                    section.lang
                );
            }
        }
    }

    #[test]
    fn a_stock_phrase_alone_leaves_nothing_to_translate() {
        for said in [
            "Thanks for watching!",
            " thanks for watching ",
            "ご視聴ありがとうございました。",
            "ご視聴ありがとうございました",
            "字幕由Amara.org社区提供",
            "Untertitel im Auftrag des ZDF für funk, 2017",
            "Sous-titres réalisés par la communauté d'Amara.org",
            "Thanks for watching! Thanks for watching!",
        ] {
            let s = screen(&[said]);
            assert_eq!(s.text, "", "{said:?}");
            assert!(!s.blocked.is_empty(), "{said:?}");
        }
    }

    #[test]
    fn speech_around_a_stock_phrase_is_kept() {
        let s = screen(&["So that is the whole trick. Thanks for watching!"]);
        assert_eq!(s.text, "So that is the whole trick.");
        assert_eq!(s.blocked, ["Thanks for watching!"]);

        let s = screen(&["今日はここまでです。ご視聴ありがとうございました。"]);
        assert_eq!(s.text, "今日はここまでです。");

        // Segment by segment, too.
        let s = screen(&["Subtitles by the Amara.org community", "Open the door."]);
        assert_eq!(s.text, "Open the door.");
    }

    #[test]
    fn ordinary_speech_passes_untouched() {
        for said in [
            "Thank you.",
            "Thanks for watching the kids yesterday.",
            "I paid 3.5 dollars. It was cheap!",
            "視聴者の皆さん、ありがとうございます。",
            "He said (quietly) that it was over.",
        ] {
            let s = screen(&[said]);
            assert_eq!(s.text, said);
            assert!(s.blocked.is_empty(), "{said:?}");
        }
        // Segments are joined as before.
        assert_eq!(
            screen(&[" Hello. ", "", "How are you?"]).text,
            "Hello. How are you?"
        );
    }

    #[test]
    fn a_sentence_cut_across_segments_is_judged_whole() {
        // Real speech that only starts like a stock phrase.
        let s = screen(&["Thanks for watching", "the kids yesterday."]);
        assert_eq!(s.text, "Thanks for watching the kids yesterday.");
        assert!(s.blocked.is_empty());
        // A stock phrase in two pieces.
        assert_eq!(screen(&["Thanks for", "watching!"]).text, "");
        assert_eq!(
            screen(&["That is all.", "thanks for", "watching", "See you."]).text,
            "That is all. See you."
        );
        // A finished sentence or an annotation is not continued.
        assert_eq!(
            screen(&["Thanks for watching!", "see you."]).text,
            "see you."
        );
        assert_eq!(screen(&["[Music]", "welcome back."]).text, "welcome back.");
    }

    #[test]
    fn a_chunk_blocked_while_it_was_translated_is_recognized() {
        let mut settings = on();
        let mut f = StockFilter::default();
        let shown = f.screen(&settings, &["Thank you."], Instant::now()).text;
        assert!(!f.blocks_all(&settings, &shown));
        settings.blocked_phrases.push("Thank you.".into());
        assert!(f.blocks_all(&settings, &shown));
        assert!(!f.blocks_all(&settings, "Thank you. Please sit."));
        settings.block_stock_phrases = false;
        assert!(!f.blocks_all(&settings, &shown));
    }

    #[test]
    fn sound_annotations_are_removed() {
        for said in [
            "[Music]",
            "(拍手)",
            "♪♪",
            "[BLANK_AUDIO]",
            "*sighs*",
            "（音楽）",
            "...",
        ] {
            assert_eq!(screen(&[said]).text, "", "{said:?}");
        }
        assert_eq!(screen(&["[Music]", "Welcome back."]).text, "Welcome back.");
    }

    #[test]
    fn the_users_phrases_are_blocked_in_any_spelling() {
        let mut settings = on();
        settings.blocked_phrases = vec!["Thank you.".into()];
        let mut f = StockFilter::default();
        let now = Instant::now();
        assert_eq!(f.screen(&settings, &["thank you"], now).text, "");
        assert_eq!(f.screen(&settings, &["Ｔｈａｎｋ ｙｏｕ！"], now).text, "");
        // Only the whole sentence.
        assert_eq!(
            f.screen(&settings, &["Thank you for the ride."], now).text,
            "Thank you for the ride."
        );
        // A change to the list applies from the next chunk.
        settings.blocked_phrases.clear();
        assert_eq!(f.screen(&settings, &["thank you"], now).text, "thank you");
    }

    #[test]
    fn switched_off_nothing_is_removed_or_suggested() {
        let settings = SubtitleSettings {
            block_stock_phrases: false,
            blocked_phrases: vec!["Thank you.".into()],
            ..on()
        };
        let mut f = StockFilter::default();
        let now = Instant::now();
        for _ in 0..5 {
            let s = f.screen(&settings, &["Thank you.", "Thanks for watching!"], now);
            assert_eq!(s.text, "Thank you. Thanks for watching!");
            assert_eq!(s.candidate, None);
        }
    }

    #[test]
    fn a_sentence_repeated_in_a_short_time_is_suggested_once() {
        let mut f = StockFilter::default();
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        assert_eq!(f.screen(&on(), &["Thank you."], at(0)).candidate, None);
        assert_eq!(f.screen(&on(), &["Thank you!"], at(3)).candidate, None);
        // Speech in between does not reset the count.
        assert_eq!(
            f.screen(&on(), &["And now the weather."], at(5)).candidate,
            None
        );
        let third = f.screen(&on(), &["thank you. Thank you."], at(6));
        assert_eq!(third.candidate.as_deref(), Some("thank you."));
        // Shown all the same: blocking is the user's decision.
        assert_eq!(third.text, "thank you. Thank you.");
        // Asked once per session.
        for s in 7..20 {
            assert_eq!(f.screen(&on(), &["Thank you."], at(s)).candidate, None);
        }
    }

    #[test]
    fn repeats_far_apart_or_inside_speech_are_not_suggested() {
        let mut f = StockFilter::default();
        let t0 = Instant::now();
        for i in 0..6 {
            let now = t0 + Duration::from_secs(i * 60);
            assert_eq!(f.screen(&on(), &["Yes."], now).candidate, None, "{i}");
        }
        let mut f = StockFilter::default();
        for i in 0..6 {
            let now = t0 + Duration::from_secs(i);
            let s = f.screen(&on(), &["Yes. I will be there at nine."], now);
            assert_eq!(s.candidate, None, "{i}");
        }
        // Long sentences repeat as content (a chorus, a slogan).
        let mut f = StockFilter::default();
        let long = "This is the song that never ends, it goes on and on my friends.";
        for i in 0..6 {
            let now = t0 + Duration::from_secs(i);
            assert_eq!(f.screen(&on(), &[long], now).candidate, None, "{i}");
        }
    }

    #[test]
    fn approving_a_candidate_adds_it_once() {
        let mut list = vec![];
        assert!(add_phrase(&mut list, " Thank you. "));
        assert!(!add_phrase(&mut list, "thank you"));
        assert!(!add_phrase(&mut list, "  "));
        assert!(!add_phrase(&mut list, "♪"));
        assert_eq!(list, ["Thank you."]);
    }
}
