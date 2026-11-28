use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

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
    /// Speed given to the engine for each text.
    rates: Vec<f32>,
    /// Speed of the settings (0: 1.0).
    rate: f32,
    /// Time since `epoch`.
    clock: Duration,
    epoch: Option<Instant>,
}

#[derive(Clone, Default)]
struct Fake(Rc<RefCell<Shared>>);

impl Engine for Fake {
    fn voices(&mut self) -> Vec<Voice> {
        let mut s = self.0.borrow_mut();
        s.listed += 1;
        s.voices.clone()
    }
    fn start(&mut self, text: &str, voice: &Voice, tuning: &Tuning) -> Result<()> {
        let mut s = self.0.borrow_mut();
        if s.fail {
            anyhow::bail!("engine failure");
        }
        s.spoken.push((text.to_string(), voice.id.clone()));
        s.rates.push(tuning.rate);
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
            rate: match self.0.borrow().rate {
                r if r > 0.0 => r,
                _ => 1.0,
            },
            volume: 1.0,
        }
    }
    fn blocked(&self) -> bool {
        self.0.borrow().blocked
    }
    fn emit(&self, owner: &str, event: Event) {
        self.0.borrow_mut().events.push((owner.to_string(), event));
    }
    fn now(&self) -> Instant {
        let mut s = self.0.borrow_mut();
        let epoch = *s.epoch.get_or_insert_with(Instant::now);
        epoch + s.clock
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
        source_ms: None,
    }
}

/// A subtitle request: `text` translates speech of `source_ms`.
fn heard(owner: &str, text: &str, source_ms: u64) -> Utterance {
    Utterance {
        source_ms: Some(source_ms),
        ..say(owner, text)
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
    fn wait(&self, ms: u64) {
        self.0.borrow_mut().clock += Duration::from_millis(ms);
    }
    fn rates(&self) -> Vec<f32> {
        self.0.borrow().rates.clone()
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
fn text_without_anything_to_pronounce_is_not_spoken() {
    let (mut p, fake) = player();
    p.say(say("lens-1", " … \n—"));
    assert!(p.idle());
    assert!(fake.spoken().is_empty());
    // Answered, so the window stops waiting for it.
    assert_eq!(fake.events(), [("lens-1".to_string(), Event::Dropped)]);
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

/* --------------------------------- pacing --------------------------------- */

/// 15 kana: 2 s at the default 7.5 per second.
const FIFTEEN: &str = "あいうえおかきくけこさしすせそ";

#[test]
fn speech_without_an_original_keeps_the_speed_of_the_settings() {
    let (mut p, fake) = player();
    fake.0.borrow_mut().rate = 1.2;
    p.say(say("lens-1", FIFTEEN));
    assert_eq!(fake.rates(), [1.2]);
}

#[test]
fn a_translation_longer_than_its_original_is_spoken_faster() {
    let (mut p, fake) = player();
    // 2 s of speech for 1.6 s of original.
    p.say(heard("subtitle-1", FIFTEEN, 1600));
    assert!((fake.rates()[0] - 1.25).abs() < 0.01, "{:?}", fake.rates());
}

#[test]
fn a_translation_shorter_than_its_original_is_not_slowed_down() {
    let (mut p, fake) = player();
    p.say(heard("subtitle-1", FIFTEEN, 6000));
    assert_eq!(fake.rates(), [1.0]);
}

#[test]
fn pacing_never_goes_past_an_audible_speed() {
    let (mut p, fake) = player();
    p.say(heard("subtitle-1", FIFTEEN, 300));
    assert_eq!(fake.rates(), [pace::MAX_RATE]);
    // A faster speed chosen in the settings is kept.
    let (mut p, fake) = player();
    fake.0.borrow_mut().rate = 2.0;
    p.say(heard("subtitle-1", FIFTEEN, 300));
    assert_eq!(fake.rates(), [2.0]);
}

#[test]
fn speech_that_trails_catches_up() {
    let (mut p, fake) = player();
    p.say(heard("subtitle-1", FIFTEEN, 2400));
    p.say(heard("subtitle-1", FIFTEEN, 2400));
    // The first takes 5 s: the second waited well past LAG_OK.
    fake.wait(5000);
    fake.end();
    p.tick();
    let rates = fake.rates();
    assert_eq!(rates[0], 1.0, "on time: own speed");
    assert!(rates[1] > 1.0, "behind: faster ({rates:?})");
}

#[test]
fn a_stale_request_gives_way_to_a_newer_one_of_its_window() {
    let (mut p, fake) = player();
    p.say(heard("subtitle-1", "第一文", 2000));
    p.say(heard("subtitle-1", "第二文", 2000));
    fake.wait(pace::STALE.as_millis() as u64 + 1000);
    p.say(heard("subtitle-1", "第三文", 2000));
    fake.events();
    fake.end();
    p.tick();
    assert_eq!(fake.spoken(), ["第一文", "第三文"]);
    assert!(fake
        .events()
        .contains(&("subtitle-1".to_string(), Event::Dropped)));
}

#[test]
fn the_last_request_is_spoken_however_late() {
    let (mut p, fake) = player();
    p.say(heard("subtitle-1", "第一文", 2000));
    p.say(heard("subtitle-1", "第二文", 2000));
    fake.wait(pace::STALE.as_millis() as u64 + 1000);
    fake.end();
    p.tick();
    assert_eq!(fake.spoken(), ["第一文", "第二文"]);
}

#[test]
fn the_speed_of_a_voice_is_learnt_from_what_it_spoke() {
    let (mut p, fake) = player();
    // This voice is slower than the guess: 15 kana took 3 s at 1.0.
    p.say(heard("subtitle-1", FIFTEEN, 6000));
    fake.wait(3000);
    fake.end();
    p.tick();
    // Next: 3 s of speech for 2 s of original = 1.5.
    p.say(heard("subtitle-1", FIFTEEN, 2000));
    let rates = fake.rates();
    assert!((rates[1] - 1.5).abs() < 0.01, "{rates:?}");
}

#[test]
fn pace_rate_gives_up_to_half_of_the_original_to_catch_up() {
    let lag = Duration::from_millis;
    // On time: natural / source.
    assert!((pace::rate(1.0, 3000.0, 2000, lag(0)) - 1.5).abs() < 0.001);
    // 2 s behind: 0.5 s beyond LAG_OK, half of it made up now.
    assert!((pace::rate(1.0, 2800.0, 2000, lag(2000)) - 1.6).abs() < 0.001);
    // Far behind: the budget stops at half of the original.
    assert!((pace::rate(1.0, 1500.0, 2000, lag(60_000)) - 1.5).abs() < 0.001);
}

#[test]
fn a_measurement_that_makes_no_sense_is_ignored() {
    let mut speeds = pace::Speeds::default();
    // Too short to measure, and an engine that stalled for a minute.
    speeds.learn("haruka", "ja", 15, 1.0, Duration::from_millis(100));
    speeds.learn("haruka", "ja", 15, 1.0, Duration::from_secs(60));
    assert_eq!(speeds.speed("haruka", "ja"), pace::default_speed("ja"));
}
