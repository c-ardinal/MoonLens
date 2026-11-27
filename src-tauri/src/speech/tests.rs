use std::cell::RefCell;
use std::rc::Rc;

use super::*;

#[derive(Default)]
struct Shared {
    voices: Vec<Voice>,
    /// Texts given to the engine, in order.
    spoken: Vec<(String, String)>,
    playing: bool,
    stops: usize,
    fail: bool,
    blocked: bool,
    voice: String,
    events: Vec<(String, Event)>,
    listed: usize,
}

#[derive(Clone, Default)]
struct Fake(Rc<RefCell<Shared>>);

impl Engine for Fake {
    fn voices(&mut self) -> Vec<Voice> {
        let mut s = self.0.borrow_mut();
        s.listed += 1;
        s.voices.clone()
    }
    fn start(&mut self, text: &str, voice: &Voice, _: &Tuning) -> Result<()> {
        let mut s = self.0.borrow_mut();
        if s.fail {
            anyhow::bail!("engine failure");
        }
        s.spoken.push((text.to_string(), voice.id.clone()));
        s.playing = true;
        Ok(())
    }
    fn playing(&mut self) -> bool {
        self.0.borrow().playing
    }
    fn stop(&mut self) {
        let mut s = self.0.borrow_mut();
        s.stops += 1;
        s.playing = false;
    }
}

impl Host for Fake {
    fn tuning(&self) -> Tuning {
        Tuning {
            voice: self.0.borrow().voice.clone(),
            rate: 1.0,
            volume: 1.0,
        }
    }
    fn blocked(&self) -> bool {
        self.0.borrow().blocked
    }
    fn emit(&self, owner: &str, event: Event) {
        self.0.borrow_mut().events.push((owner.to_string(), event));
    }
}

fn voice(id: &str, lang: &str) -> Voice {
    Voice {
        id: id.into(),
        name: id.into(),
        lang: lang.into(),
    }
}

fn say(owner: &str, text: &str) -> Utterance {
    Utterance {
        owner: owner.into(),
        text: text.into(),
        lang: "ja".into(),
    }
}

fn player() -> (Player<Fake, Fake>, Fake) {
    let fake = Fake::default();
    fake.0.borrow_mut().voices = vec![voice("haruka", "ja-JP"), voice("zira", "en-US")];
    (Player::new(fake.clone(), fake.clone()), fake)
}

impl Fake {
    fn spoken(&self) -> Vec<String> {
        self.0
            .borrow()
            .spoken
            .iter()
            .map(|(t, _)| t.clone())
            .collect()
    }
    fn events(&self) -> Vec<(String, Event)> {
        std::mem::take(&mut self.0.borrow_mut().events)
    }
    /// The engine reaches the end of what it speaks.
    fn end(&self) {
        self.0.borrow_mut().playing = false;
    }
}

fn skipped(reason: Skip) -> Event {
    Event::Skipped {
        reason,
        lang: "ja".into(),
    }
}

#[test]
fn requests_are_spoken_one_after_another() {
    let (mut p, fake) = player();
    p.say(say("lens-1", "一行目"));
    p.say(say("lens-2", "二行目"));
    assert_eq!(fake.spoken(), ["一行目"]);
    p.tick();
    assert_eq!(fake.spoken(), ["一行目"], "the first is still being spoken");
    fake.end();
    p.tick();
    assert_eq!(fake.spoken(), ["一行目", "二行目"]);
    fake.end();
    p.tick();
    assert!(p.idle());
    assert_eq!(
        fake.events(),
        [
            ("lens-1".to_string(), Event::Started),
            ("lens-1".to_string(), Event::Ended),
            ("lens-2".to_string(), Event::Started),
            ("lens-2".to_string(), Event::Ended),
        ]
    );
}

#[test]
fn backlog_drops_what_waited_longest() {
    let (mut p, fake) = player();
    for text in ["a1", "a2", "a3"] {
        p.say(say("lens-1", text));
    }
    fake.events();
    p.say(say("lens-2", "a4"));
    // The window that asked learns its request left the queue.
    assert_eq!(fake.events(), [("lens-1".to_string(), Event::Dropped)]);
    for _ in 0..4 {
        fake.end();
        p.tick();
    }
    assert_eq!(fake.spoken(), ["a1", "a3", "a4"]);
}

#[test]
fn cancel_stops_only_the_requests_of_that_window() {
    let (mut p, fake) = player();
    p.say(say("lens-1", "a1"));
    p.say(say("lens-2", "b1"));
    p.say(say("lens-1", "a2"));
    p.cancel("lens-1");
    assert_eq!(fake.0.borrow().stops, 1);
    assert_eq!(fake.spoken(), ["a1", "b1"]);
    fake.end();
    p.tick();
    assert!(p.idle(), "a2 was dropped with its window");
}

#[test]
fn cancel_of_a_waiting_window_leaves_the_speaker_alone() {
    let (mut p, fake) = player();
    p.say(say("lens-1", "a1"));
    p.say(say("lens-2", "b1"));
    p.cancel("lens-2");
    assert_eq!(fake.0.borrow().stops, 0);
    fake.end();
    p.tick();
    assert_eq!(fake.spoken(), ["a1"]);
}

#[test]
fn nothing_is_spoken_while_the_whole_pc_is_captured() {
    let (mut p, fake) = player();
    fake.0.borrow_mut().blocked = true;
    p.say(say("lens-1", "a1"));
    assert!(fake.spoken().is_empty());
    assert_eq!(
        fake.events(),
        [("lens-1".to_string(), skipped(Skip::Blocked))]
    );
}

#[test]
fn a_capture_of_the_whole_pc_starting_silences_speech() {
    let (mut p, fake) = player();
    p.say(say("lens-1", "a1"));
    p.say(say("lens-1", "a2"));
    p.say(say("subtitle-1", "b1"));
    fake.events();
    fake.0.borrow_mut().blocked = true;
    p.tick();
    assert!(p.idle());
    assert_eq!(fake.0.borrow().stops, 1);
    assert_eq!(
        fake.events(),
        [
            ("lens-1".to_string(), Event::Ended),
            ("lens-1".to_string(), skipped(Skip::Blocked)),
            ("subtitle-1".to_string(), skipped(Skip::Blocked)),
        ]
    );
    fake.0.borrow_mut().blocked = false;
    p.say(say("lens-1", "a3"));
    assert_eq!(fake.spoken(), ["a1", "a3"]);
}

#[test]
fn a_language_without_a_voice_is_reported_and_the_next_request_spoken() {
    let (mut p, fake) = player();
    p.say(Utterance {
        lang: "ko".into(),
        ..say("lens-1", "안녕하세요")
    });
    p.say(say("lens-1", "こんにちは"));
    assert_eq!(fake.spoken(), ["こんにちは"]);
    assert_eq!(
        fake.events()[0],
        (
            "lens-1".to_string(),
            Event::Skipped {
                reason: Skip::NoVoice,
                lang: "ko".into()
            }
        )
    );
}

#[test]
fn a_voice_installed_later_is_found() {
    let (mut p, fake) = player();
    let korean = Utterance {
        lang: "ko".into(),
        ..say("lens-1", "안녕하세요")
    };
    p.say(korean.clone());
    fake.0.borrow_mut().voices.push(voice("heami", "ko-KR"));
    p.say(korean);
    assert_eq!(fake.spoken(), ["안녕하세요"]);
}

#[test]
fn voices_are_listed_once_while_they_serve() {
    let (mut p, fake) = player();
    for text in ["a1", "a2"] {
        p.say(say("lens-1", text));
        fake.end();
        p.tick();
    }
    assert_eq!(fake.0.borrow().listed, 1);
}

#[test]
fn an_engine_failure_is_reported_and_does_not_stall_the_queue() {
    let (mut p, fake) = player();
    fake.0.borrow_mut().fail = true;
    p.say(say("lens-1", "a1"));
    assert!(p.idle());
    assert_eq!(
        fake.events(),
        [("lens-1".to_string(), skipped(Skip::Failed))]
    );
    fake.0.borrow_mut().fail = false;
    p.say(say("lens-1", "a2"));
    assert_eq!(fake.spoken(), ["a2"]);
}

#[test]
fn text_without_anything_to_pronounce_is_ignored() {
    let (mut p, fake) = player();
    p.say(say("lens-1", " … \n—"));
    assert!(p.idle());
    assert!(fake.events().is_empty());
}

#[test]
fn prepare_joins_lines_and_limits_the_length() {
    assert_eq!(
        prepare("  一行目\n\n二行目  ").as_deref(),
        Some("一行目 二行目")
    );
    assert_eq!(prepare("!?…"), None);
    let long = "あ".repeat(MAX_CHARS + 50);
    assert_eq!(prepare(&long).unwrap().chars().count(), MAX_CHARS);
}

#[test]
fn the_preferred_voice_is_used_only_for_its_language() {
    let voices = [
        voice("ayumi", "ja-JP"),
        voice("haruka", "ja-JP"),
        voice("zira", "en-US"),
        voice("hanhan", "zh-TW"),
        voice("huihui", "zh-CN"),
    ];
    let id = |lang, preferred| pick_voice(&voices, lang, preferred).map(|v| v.id.as_str());
    assert_eq!(id("ja", ""), Some("ayumi"));
    assert_eq!(id("ja", "haruka"), Some("haruka"));
    assert_eq!(id("en", "haruka"), Some("zira"));
    assert_eq!(id("zh-Hans", ""), Some("huihui"));
    assert_eq!(id("zh-Hant", ""), Some("hanhan"));
    assert_eq!(id("zh-TW", ""), Some("hanhan"));
    assert_eq!(id("zh", ""), Some("hanhan"));
    assert_eq!(id("ko", "haruka"), None);
}
