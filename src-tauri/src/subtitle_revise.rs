//! Sentence-level correction of live subtitles.
//!
//! The subtitle loop translates each recognized chunk (a few seconds of
//! audio) on its own, so a sentence cut by chunk boundaries comes out as
//! disconnected fragments. Here the recognized chunks are joined back
//! into sentences ([`Assembler`]) and each finished sentence is
//! translated again as a whole, with the earlier sentences as context
//! ([`Reviser`]). The result replaces the fragments in the window.

use std::{collections::VecDeque, future::Future, pin::Pin, sync::Arc};

use serde::Serialize;

use crate::{providers::TranslateContext, translate_plan};

/// One finished sentence (or several, when a chunk held more than one):
/// the subtitle lines it replaces and its full text.
#[derive(Debug, Clone, PartialEq)]
pub struct Sentence {
    /// Ids of the lines this text was shown as so far.
    pub replaces: Vec<u64>,
    /// Id of the line that takes their place.
    pub id: u64,
    pub text: String,
    /// Start of the next sentence, recognized in the same chunk as the
    /// end of this one: `(line id, text)`.
    pub rest: Option<(u64, String)>,
    /// Translation the window already shows for exactly this text (the
    /// sentence was one whole chunk), so there is nothing to correct.
    pub draft: Option<String>,
    /// The translations of the replaced lines, joined: what the new
    /// line shows when translating the sentence again fails.
    pub fragments: String,
}

struct Piece {
    id: u64,
    /// Translation shown for this piece when it is a whole chunk.
    translated: Option<String>,
}

/// Joins recognized chunks into sentences.
#[derive(Default)]
pub struct Assembler {
    last_id: u64,
    pieces: Vec<Piece>,
    /// Text of the unfinished sentence.
    text: String,
}

/// An unfinished sentence longer than this ([`translate_plan::cost`]
/// units, about 40 s of speech) is closed where it stands: speech
/// recognition often leaves out punctuation, Japanese above all.
const MAX_OPEN_COST: usize = 600;

impl Assembler {
    /// Id for the next subtitle line.
    pub fn next_id(&mut self) -> u64 {
        self.last_id += 1;
        self.last_id
    }

    /// Whether part of a sentence is waiting for its end.
    pub fn is_open(&self) -> bool {
        !self.pieces.is_empty()
    }

    /// Forget the unfinished sentence.
    pub fn clear(&mut self) {
        self.pieces.clear();
        self.text.clear();
    }

    /// Add the chunk shown as line `id`. Returns the text up to the last
    /// sentence end the chunk brought, if any.
    ///
    /// A sentence end at the very end of `original` does not count yet:
    /// the recognizer puts a full stop wherever the audio was cut. The
    /// next chunk (or a pause, see [`Assembler::flush`]) settles it.
    ///
    /// `overlapped`: the chunk's audio began with the end of the
    /// previous chunk's, so its first words may repeat that chunk's last.
    pub fn push(
        &mut self,
        id: u64,
        original: &str,
        translated: &str,
        overlapped: bool,
    ) -> Option<Sentence> {
        let original = original.trim();
        if original.is_empty() {
            return None;
        }
        self.text = join(&self.text, original, overlapped);
        self.pieces.push(Piece {
            id,
            translated: Some(translated.trim().to_string()).filter(|t| !t.is_empty()),
        });
        match last_boundary(&self.text) {
            Some(at) => {
                let rest = self.text[at..].trim().to_string();
                self.text.truncate(at);
                Some(self.close(Some(rest)))
            }
            None if translate_plan::cost(&self.text) > MAX_OPEN_COST => Some(self.close(None)),
            None => None,
        }
    }

    /// The speaker paused (or the audio ended): whatever is open is a
    /// sentence now.
    pub fn flush(&mut self) -> Option<Sentence> {
        self.is_open().then(|| self.close(None))
    }

    fn close(&mut self, rest: Option<String>) -> Sentence {
        let pieces = std::mem::take(&mut self.pieces);
        let text = std::mem::take(&mut self.text).trim().to_string();
        let id = self.next_id();
        let rest = rest.filter(|r| !r.is_empty()).map(|r| {
            let rest_id = self.next_id();
            self.text = r.clone();
            self.pieces.push(Piece {
                id: rest_id,
                translated: None,
            });
            (rest_id, r)
        });
        let draft = match (&pieces[..], &rest) {
            ([only], None) => only.translated.clone(),
            _ => None,
        };
        let fragments = pieces
            .iter()
            .filter_map(|p| p.translated.as_deref())
            .collect::<Vec<_>>()
            .join(" ");
        Sentence {
            replaces: pieces.iter().map(|p| p.id).collect(),
            id,
            text,
            rest,
            draft,
            fragments,
        }
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3040}'..='\u{30ff}'   // kana
        | '\u{3400}'..='\u{9fff}' // ideographs
        | '\u{ac00}'..='\u{d7af}' // hangul
        | '\u{3000}'..='\u{303f}' // CJK punctuation
        | '\u{ff00}'..='\u{ffef}' // full-width forms
    )
}

/// A word reduced to what is compared across a chunk boundary.
fn bare(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Most words (or, without spaces between words, characters) at the
/// start of a chunk that may repeat the end of the previous one. Chunks
/// overlap by up to two seconds of audio.
const MAX_REPEAT_WORDS: usize = 6;
const MAX_REPEAT_CHARS: usize = 12;

/// `prev` followed by `next`, without a full stop the recognizer put at
/// the cut and, when the chunks share audio (`overlapped`), without the
/// words that `next` repeats from the end of `prev`.
fn join(prev: &str, next: &str, overlapped: bool) -> String {
    let (mut prev, mut next) = (prev.trim_end(), next.trim_start());
    if prev.is_empty() {
        return next.to_string();
    }
    let spaced =
        !prev.chars().next_back().is_some_and(is_cjk) && !next.chars().next().is_some_and(is_cjk);
    if spaced {
        let tail: Vec<String> = prev
            .split_whitespace()
            .rev()
            .take(MAX_REPEAT_WORDS)
            .map(bare)
            .collect();
        let head: Vec<(usize, &str)> = next
            .split_whitespace()
            .take(MAX_REPEAT_WORDS)
            .map(|w| (w.as_ptr() as usize - next.as_ptr() as usize + w.len(), w))
            .collect();
        let most = if overlapped {
            tail.len().min(head.len())
        } else {
            0
        };
        for n in (1..=most).rev() {
            let same = (0..n).all(|i| {
                let word = bare(head[i].1);
                !word.is_empty() && word == tail[n - 1 - i]
            });
            if same {
                next = next[head[n - 1].0..].trim_start();
                break;
            }
        }
        // "ask not." + "what your country" is one sentence.
        if next.chars().next().is_some_and(char::is_lowercase)
            && prev.ends_with('.')
            && !prev.ends_with("..")
        {
            prev = &prev[..prev.len() - 1];
        }
        if next.is_empty() {
            return prev.to_string();
        }
        format!("{prev} {next}")
    } else {
        let stem = prev.trim_end_matches(|c: char| !c.is_alphanumeric());
        let head: Vec<usize> = next
            .char_indices()
            .skip(1)
            .map(|(i, _)| i)
            .chain([next.len()])
            .take(MAX_REPEAT_CHARS)
            .collect();
        // One repeated character is too often a coincidence.
        for &end in head.iter().skip(1).rev() {
            if overlapped && stem.ends_with(&next[..end]) {
                return format!("{stem}{}", &next[end..]);
            }
        }
        format!("{prev}{next}")
    }
}

/// Words before a full stop that do not end a sentence.
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "st", "sr", "jr", "vs", "etc", "no", "mt",
];

/// Byte offset just behind the last sentence end in `text` that has
/// more text after it.
fn last_boundary(text: &str) -> Option<usize> {
    let mut found = None;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        let wide = matches!(c, '。' | '！' | '？');
        if !wide && !matches!(c, '.' | '!' | '?') {
            continue;
        }
        // Further marks and closing quotes belong to the same end.
        let mut end = at + c.len_utf8();
        while let Some(&(i, n)) = chars.peek() {
            if !matches!(
                n,
                '.' | '!'
                    | '?'
                    | '。'
                    | '！'
                    | '？'
                    | '"'
                    | '\''
                    | '”'
                    | '’'
                    | '」'
                    | '』'
                    | ')'
                    | '）'
            ) {
                break;
            }
            end = i + n.len_utf8();
            chars.next();
        }
        let after = &text[end..];
        let Some(next) = after.trim_start().chars().next() else {
            break;
        };
        if !wide {
            // "3.5", "example.com": no space, no sentence end.
            if after.len() == after.trim_start().len() && !is_cjk(next) {
                continue;
            }
            if c == '.' && end == at + 1 {
                let word = text[..at].rsplit(char::is_whitespace).next().unwrap_or("");
                let initial = word.chars().count() == 1 && word.chars().all(char::is_uppercase);
                if next.is_lowercase()
                    || initial
                    || word.contains('.')
                    || ABBREVIATIONS.contains(&word.to_lowercase().as_str())
                {
                    continue;
                }
            }
        }
        found = Some(end);
    }
    found
}

/// A corrected sentence for the window: lines `replaces` give way to
/// one line `id`, followed by `rest` when the last replaced line also
/// held the start of the next sentence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Revision {
    pub replaces: Vec<u64>,
    pub id: u64,
    pub original: String,
    pub translated: String,
    pub rest: Option<RevisedRest>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RevisedRest {
    pub id: u64,
    pub original: String,
    /// Empty when it was not translated (the next sentence is already
    /// waiting to replace this line).
    pub translated: String,
}

/// Source and target language of one request.
#[derive(Debug, Clone, PartialEq)]
pub struct Languages {
    pub source: String,
    pub target: String,
}

type Translating = Pin<Box<dyn Future<Output = Option<String>> + Send>>;

/// Translates `text` with `context`; `None` when it failed.
pub type Translator = Arc<dyn Fn(String, Languages, TranslateContext) -> Translating + Send + Sync>;

struct Job {
    sentence: Sentence,
    languages: Languages,
}

struct Done {
    sentence: Sentence,
    translated: Option<String>,
    rest: Option<String>,
}

/// History handed to the translator, in [`translate_plan::cost`] units.
/// When it outgrows `HISTORY_MAX` it is cut back to `HISTORY_KEEP` in one
/// step rather than a pair at a time: between cuts every request starts
/// with the same text as the one before, which the engine has cached.
const HISTORY_MAX: usize = 1500;
const HISTORY_KEEP: usize = 700;

/// Translates finished sentences again, one at a time and in order,
/// while the subtitle loop carries on. It runs inside the loop's task
/// (see [`Reviser::during`]), so stopping the loop stops it too.
pub struct Reviser {
    translate: Translator,
    queue: VecDeque<Job>,
    /// Jobs at the front of `queue` that may start.
    released: usize,
    running: Option<Pin<Box<dyn Future<Output = Done> + Send>>>,
    /// Finished sentences and their translations, oldest first.
    history: Vec<(String, String)>,
    /// The last revision's `rest` as translated: `(line id, text, translation)`.
    last_rest: Option<(u64, String, String)>,
}

impl Reviser {
    pub fn new(translate: Translator) -> Self {
        Self {
            translate,
            queue: VecDeque::new(),
            released: 0,
            running: None,
            history: Vec::new(),
            last_rest: None,
        }
    }

    /// Queue a sentence. It waits for [`Reviser::release`] unless
    /// `release` is set.
    pub fn push(&mut self, sentence: Sentence, languages: Languages, release: bool) {
        self.queue.push_back(Job {
            sentence,
            languages,
        });
        if release {
            self.release();
        }
    }

    /// Let every queued sentence be translated.
    pub fn release(&mut self) {
        self.released = self.queue.len();
    }

    /// Drop the sentences that have not started.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.released = 0;
    }

    /// Whether a translation is running or may start.
    pub fn is_busy(&self) -> bool {
        self.running.is_some() || self.released > 0
    }

    /// The next corrected sentence, or `None` when nothing is left that
    /// may run. Safe to drop while it waits: the translation in flight
    /// is kept for the next call.
    pub async fn next(&mut self) -> Option<Revision> {
        loop {
            if self.running.is_none() {
                if self.released == 0 {
                    return None;
                }
                self.released -= 1;
                let job = self.queue.pop_front()?;
                self.start(job);
                continue;
            }
            let done = self.running.as_mut()?.await;
            self.running = None;
            return Some(self.finish(done));
        }
    }

    /// Wait for `work`, handing each sentence corrected meanwhile to
    /// `emit`.
    pub async fn during<T>(
        &mut self,
        emit: &(impl Fn(Revision) + Sync),
        work: impl Future<Output = T>,
    ) -> T {
        tokio::pin!(work);
        loop {
            tokio::select! {
                biased;
                out = &mut work => return out,
                Some(revision) = self.next() => emit(revision),
            }
        }
    }

    fn start(&mut self, job: Job) {
        let Job {
            sentence,
            languages,
        } = job;
        // Already shown in full: as one chunk, or as the previous
        // revision's rest.
        let shown = sentence.draft.clone().or_else(|| {
            let (id, text, translated) = self.last_rest.as_ref()?;
            (sentence.replaces == [*id] && sentence.text == *text && sentence.rest.is_none())
                .then(|| translated.clone())
        });
        if let Some(translated) = shown {
            self.remember(sentence.text, translated);
            return;
        }
        let context = TranslateContext {
            history: self.history.clone(),
            reference: None,
        };
        // A rest nobody waits to replace is worth translating: it stays
        // on screen until its sentence ends.
        let rest = sentence
            .rest
            .as_ref()
            .filter(|_| self.queue.is_empty())
            .map(|(_, text)| text.clone());
        let translate = self.translate.clone();
        self.running = Some(Box::pin(async move {
            let translated = translate(sentence.text.clone(), languages.clone(), context.clone())
                .await
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty());
            let rest = match (&translated, rest) {
                (Some(translated), Some(rest)) => {
                    let mut context = context;
                    context
                        .history
                        .push((sentence.text.clone(), translated.clone()));
                    translate(rest, languages, context).await
                }
                _ => None,
            };
            Done {
                sentence,
                translated,
                rest,
            }
        }));
    }

    fn finish(&mut self, done: Done) -> Revision {
        let Done {
            sentence,
            translated,
            rest,
        } = done;
        let shown_rest = self.last_rest.take();
        // The lines are replaced even when the translation failed, then
        // with what they showed: later sentences refer to the new line
        // ids (`rest`), so the window has to follow every step.
        let translated = match translated {
            Some(translated) => {
                self.remember(sentence.text.clone(), translated.clone());
                translated
            }
            // A replaced rest line shows the translation made for it
            // here, which the assembler never saw.
            None => match shown_rest {
                Some((id, _, rest)) if sentence.replaces.first() == Some(&id) => {
                    format!("{rest} {}", sentence.fragments).trim().to_string()
                }
                _ => sentence.fragments,
            },
        };
        let rest = sentence.rest.map(|(id, original)| {
            let translated = rest.map(|t| t.trim().to_string()).unwrap_or_default();
            if !translated.is_empty() {
                self.last_rest = Some((id, original.clone(), translated.clone()));
            }
            RevisedRest {
                id,
                original,
                translated,
            }
        });
        Revision {
            replaces: sentence.replaces,
            id: sentence.id,
            original: sentence.text,
            translated,
            rest,
        }
    }

    fn remember(&mut self, original: String, translated: String) {
        self.history.push((original, translated));
        let cost: usize = self
            .history
            .iter()
            .map(|(s, t)| translate_plan::cost(s) + translate_plan::cost(t))
            .sum();
        if cost > HISTORY_MAX {
            translate_plan::trim_history(&mut self.history, HISTORY_KEEP);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    fn chunks(asm: &mut Assembler, texts: &[&str]) -> Vec<Sentence> {
        texts
            .iter()
            .filter_map(|t| {
                let id = asm.next_id();
                asm.push(id, t, &format!("<{t}>"), true)
            })
            .collect()
    }

    #[test]
    fn sentence_cut_by_chunks_is_joined() {
        let mut asm = Assembler::default();
        let done = chunks(
            &mut asm,
            &[
                "And so, my fellow Americans,",
                "ask not what your country",
                "can do for you. Ask what you",
                "can do for your country.",
            ],
        );
        // Lines 1-3 become line 4; "Ask what you" stays as line 5.
        assert_eq!(
            done,
            vec![Sentence {
                replaces: vec![1, 2, 3],
                id: 4,
                text: "And so, my fellow Americans, ask not what your country can do for you."
                    .into(),
                rest: Some((5, "Ask what you".into())),
                draft: None,
                fragments: "<And so, my fellow Americans,> <ask not what your country> \
                            <can do for you. Ask what you>"
                    .into(),
            }]
        );
        // The pause after the speech ends the second sentence.
        assert_eq!(
            asm.flush(),
            Some(Sentence {
                replaces: vec![5, 6],
                id: 7,
                text: "Ask what you can do for your country.".into(),
                rest: None,
                draft: None,
                fragments: "<can do for your country.>".into(),
            })
        );
        assert!(!asm.is_open());
        assert_eq!(asm.flush(), None);
    }

    #[test]
    fn full_stop_at_a_cut_waits_for_the_next_chunk() {
        let mut asm = Assembler::default();
        // The recognizer ends every chunk with a full stop.
        assert!(chunks(&mut asm, &["Ask not.", "what your country can do."]).is_empty());
        let done = chunks(&mut asm, &["The next sentence"]);
        assert_eq!(done[0].text, "Ask not what your country can do.");
        assert_eq!(done[0].replaces, vec![1, 2, 3]);
        assert_eq!(done[0].rest, Some((5, "The next sentence".into())));
    }

    #[test]
    fn words_repeated_across_the_overlap_appear_once() {
        assert_eq!(
            join("ask not what your country", "your country can do", true),
            "ask not what your country can do"
        );
        assert_eq!(
            join("what your Country,", "country can do", true),
            "what your Country, can do"
        );
        assert_eq!(
            join("no overlap", "here at all", true),
            "no overlap here at all"
        );
        assert_eq!(join("the same", "the same", true), "the same");
        assert_eq!(join("", " first ", true), "first ");
        // Without spaces between words: by characters, two or more.
        assert_eq!(
            join("今日は天気が", "天気がいいですね", true),
            "今日は天気がいいですね"
        );
        assert_eq!(
            join("今日は天気が、", "天気がいい", true),
            "今日は天気がいい"
        );
        assert_eq!(join("私は", "は元気", true), "私はは元気");
        assert_eq!(
            join("こんにちは。", "元気ですか", true),
            "こんにちは。元気ですか"
        );
        // Chunks that share no audio: a repeated word was said twice.
        assert_eq!(
            join("He said no.", "No one left.", false),
            "He said no. No one left."
        );
        assert_eq!(join("天気が", "天気がいい", false), "天気が天気がいい");
        assert_eq!(join("ask not.", "what you", false), "ask not what you");
    }

    #[test]
    fn sentence_ends() {
        assert_eq!(last_boundary("One. Two"), Some(4));
        assert_eq!(last_boundary("One. Two. Three"), Some(9));
        assert_eq!(last_boundary("Really?! Yes"), Some(8));
        assert_eq!(last_boundary("He said \"go.\" Then"), Some(13));
        assert_eq!(last_boundary("一つ。二つ"), Some("一つ。".len()));
        assert_eq!(last_boundary("本当？はい"), Some("本当？".len()));
        // Not followed by more text.
        assert_eq!(last_boundary("One."), None);
        assert_eq!(last_boundary("One. "), None);
        // Not sentence ends.
        assert_eq!(last_boundary("It costs 3.5 dollars"), None);
        assert_eq!(last_boundary("Dr. Smith and Mr. Jones"), None);
        assert_eq!(last_boundary("John F. Kennedy spoke"), None);
        assert_eq!(last_boundary("the U.S. economy"), None);
        assert_eq!(last_boundary("wait... what"), Some(7));
        assert_eq!(last_boundary("see example.com now"), None);
    }

    #[test]
    fn whole_chunk_sentence_keeps_its_translation() {
        let mut asm = Assembler::default();
        assert!(chunks(&mut asm, &["Good evening."]).is_empty());
        let s = asm.flush().unwrap();
        assert_eq!(s.replaces, vec![1]);
        assert_eq!(s.draft.as_deref(), Some("<Good evening.>"));
    }

    #[test]
    fn speech_without_punctuation_is_closed_by_length() {
        let mut asm = Assembler::default();
        let mut done = Vec::new();
        for _ in 0..30 {
            done.extend(chunks(&mut asm, &["句読点のない日本語の発話が続く"]));
        }
        assert!(done.len() >= 2, "{done:?}");
        assert!(done
            .iter()
            .all(|s| translate_plan::cost(&s.text) <= MAX_OPEN_COST + 60));
        assert!(done.iter().all(|s| s.rest.is_none()));
    }

    type Calls = Arc<Mutex<Vec<(String, TranslateContext)>>>;

    /// A translator that upper-cases, recording what it was asked.
    fn upper() -> (Translator, Calls) {
        let calls: Calls = Default::default();
        let seen = calls.clone();
        let translate: Translator = Arc::new(move |text, _, context| {
            seen.lock().push((text.clone(), context));
            Box::pin(async move {
                tokio::task::yield_now().await;
                (!text.contains("FAIL")).then(|| text.to_uppercase())
            })
        });
        (translate, calls)
    }

    fn en_ja() -> Languages {
        Languages {
            source: "en".into(),
            target: "ja".into(),
        }
    }

    fn sentence(replaces: &[u64], id: u64, text: &str, rest: Option<(u64, &str)>) -> Sentence {
        Sentence {
            replaces: replaces.to_vec(),
            id,
            text: text.into(),
            rest: rest.map(|(id, t)| (id, t.to_string())),
            draft: None,
            fragments: format!("<{text}>"),
        }
    }

    #[tokio::test]
    async fn sentences_are_translated_in_order_with_history() {
        let (translate, calls) = upper();
        let mut r = Reviser::new(translate);
        assert_eq!(r.next().await, None);
        r.push(sentence(&[1, 2], 3, "first one.", None), en_ja(), true);
        r.push(sentence(&[4], 5, "second one.", None), en_ja(), true);
        assert!(r.is_busy());
        let a = r.next().await.unwrap();
        assert_eq!(
            (a.replaces.as_slice(), a.id, a.translated.as_str()),
            (&[1, 2][..], 3, "FIRST ONE.")
        );
        let b = r.next().await.unwrap();
        assert_eq!(b.translated, "SECOND ONE.");
        assert_eq!(r.next().await, None);
        assert!(!r.is_busy());
        let calls = calls.lock();
        assert!(calls[0].1.history.is_empty());
        assert_eq!(
            calls[1].1.history,
            vec![("first one.".to_string(), "FIRST ONE.".to_string())]
        );
    }

    #[tokio::test]
    async fn held_sentences_wait_for_release() {
        let (translate, calls) = upper();
        let mut r = Reviser::new(translate);
        r.push(sentence(&[1], 2, "one.", None), en_ja(), false);
        r.push(sentence(&[3], 4, "two.", None), en_ja(), false);
        assert!(!r.is_busy());
        assert_eq!(r.next().await, None);
        assert!(calls.lock().is_empty());
        r.release();
        assert_eq!(r.next().await.unwrap().translated, "ONE.");
        assert_eq!(r.next().await.unwrap().translated, "TWO.");
        assert_eq!(r.next().await, None);
    }

    #[tokio::test]
    async fn rest_is_translated_only_when_nothing_waits_behind() {
        let (translate, calls) = upper();
        let mut r = Reviser::new(translate);
        r.push(sentence(&[1], 2, "one.", Some((3, "and"))), en_ja(), true);
        r.push(
            sentence(&[3, 4], 5, "and two.", Some((6, "then"))),
            en_ja(),
            true,
        );
        let a = r.next().await.unwrap();
        assert_eq!(a.rest.as_ref().unwrap().translated, "");
        let b = r.next().await.unwrap();
        let rest = b.rest.unwrap();
        assert_eq!((rest.id, rest.translated.as_str()), (6, "THEN"));
        // The rest was translated knowing its sentence so far.
        {
            let calls = calls.lock();
            let (text, context) = calls.last().unwrap();
            assert_eq!(text, "then");
            assert_eq!(context.history.last().unwrap().0, "and two.");
        }
        // A pause right after it: the rest is the whole sentence and its
        // translation stands.
        let before = r.history.len();
        r.push(sentence(&[6], 7, "then", None), en_ja(), true);
        assert_eq!(r.next().await, None);
        assert_eq!(r.history.len(), before + 1);
    }

    #[tokio::test]
    async fn sentence_shown_whole_is_not_translated_again() {
        let (translate, calls) = upper();
        let mut r = Reviser::new(translate);
        let mut whole = sentence(&[1], 2, "Good evening.", None);
        whole.draft = Some("こんばんは。".into());
        r.push(whole, en_ja(), true);
        r.push(sentence(&[3, 4], 5, "next one.", None), en_ja(), true);
        assert_eq!(r.next().await.unwrap().id, 5);
        let calls = calls.lock();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1.history[0].1, "こんばんは。");
    }

    #[tokio::test]
    async fn failed_translation_keeps_what_the_lines_showed() {
        let (translate, _) = upper();
        let mut r = Reviser::new(translate);
        r.push(
            sentence(&[1, 2], 3, "FAIL here.", Some((4, "and"))),
            en_ja(),
            true,
        );
        // The lines still give way to the new ids the next sentence
        // refers to.
        let failed = r.next().await.unwrap();
        assert_eq!(
            (
                failed.replaces.as_slice(),
                failed.id,
                failed.translated.as_str()
            ),
            (&[1, 2][..], 3, "<FAIL here.>")
        );
        assert_eq!(failed.rest.unwrap().id, 4);
        r.push(
            sentence(&[4, 5], 6, "and fine.", Some((7, "then"))),
            en_ja(),
            true,
        );
        assert_eq!(r.next().await.unwrap().translated, "AND FINE.");
        assert!(r.history.iter().all(|(s, _)| !s.contains("FAIL")));
        // A failure after a translated rest keeps that translation too.
        let mut next = sentence(&[7, 8], 9, "then FAIL.", None);
        next.fragments = "<FAIL.>".into();
        r.push(next, en_ja(), true);
        assert_eq!(r.next().await.unwrap().translated, "THEN <FAIL.>");
    }

    #[tokio::test]
    async fn work_goes_on_while_the_loop_waits_for_something_else() {
        let (translate, _) = upper();
        let mut r = Reviser::new(translate);
        r.push(sentence(&[1, 2], 3, "one.", None), en_ja(), true);
        let got = Mutex::new(Vec::new());
        let (tx, rx) = tokio::sync::oneshot::channel::<u8>();
        let emit = |rev: Revision| {
            got.lock().push(rev.translated);
        };
        let out = r
            .during(&emit, async {
                // Long enough for the revision to finish first.
                for _ in 0..50 {
                    tokio::task::yield_now().await;
                }
                drop(tx);
                rx.await.is_err()
            })
            .await;
        assert!(out);
        assert_eq!(*got.lock(), vec!["ONE.".to_string()]);
    }

    #[tokio::test]
    async fn history_is_cut_back_in_one_step() {
        let (translate, _) = upper();
        let mut r = Reviser::new(translate);
        for i in 0..40 {
            r.remember(format!("{i} {}", "a".repeat(40)), "b".repeat(40));
        }
        let cost: usize = r
            .history
            .iter()
            .map(|(s, t)| translate_plan::cost(s) + translate_plan::cost(t))
            .sum();
        assert!(cost <= HISTORY_MAX, "{cost}");
        assert!(r.history.len() >= 8, "{}", r.history.len());
        assert!(r.history.last().unwrap().0.starts_with("39 "));
    }
}
