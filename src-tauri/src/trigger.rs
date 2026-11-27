//! Extra triggers for the shortcut actions: mouse buttons, a modifier key
//! pressed twice, and gamepad button combinations.
//!
//! This module is the platform-neutral half: the text form of a trigger
//! (stored in `[triggers]` of the settings), and [`Detector`], the state
//! machine that turns raw input events into "action pressed / released".
//! The watcher that feeds it lives in `trigger_watch.rs`.
//!
//! Text forms (canonical output of [`Trigger`]'s `Display`):
//! - `Mouse4`, `Control+Shift+Mouse5`, `Alt+MouseMiddle`
//! - `Double:Control` (also `Alt`, `Shift`)
//! - `Pad:Back+Y`

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

/// Longest press that still counts as a tap of a modifier key.
pub const TAP_MAX_MS: u64 = 300;
/// Longest pause between the first tap's release and the second press.
pub const GAP_MAX_MS: u64 = 400;
/// Gamepads the watcher reports (XInput slots).
pub const PAD_SLOTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Control,
    Alt,
    Shift,
}

impl Modifier {
    const ALL: [Modifier; 3] = [Modifier::Control, Modifier::Alt, Modifier::Shift];

    fn name(self) -> &'static str {
        match self {
            Modifier::Control => "Control",
            Modifier::Alt => "Alt",
            Modifier::Shift => "Shift",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        const NAMES: [(&str, Modifier); 7] = [
            ("Control", Modifier::Control),
            ("Ctrl", Modifier::Control),
            ("CommandOrControl", Modifier::Control),
            ("CmdOrCtrl", Modifier::Control),
            ("Alt", Modifier::Alt),
            ("Option", Modifier::Alt),
            ("Shift", Modifier::Shift),
        ];
        NAMES
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(s))
            .map(|(_, m)| *m)
    }

    fn bit(self) -> u8 {
        match self {
            Modifier::Control => 1,
            Modifier::Alt => 2,
            Modifier::Shift => 4,
        }
    }
}

/// Modifier keys held together with a mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods(u8);

impl Mods {
    pub const NONE: Mods = Mods(0);

    pub fn with(self, m: Modifier) -> Self {
        Mods(self.0 | m.bit())
    }

    pub fn without(self, m: Modifier) -> Self {
        Mods(self.0 & !m.bit())
    }

    pub fn has(self, m: Modifier) -> bool {
        self.0 & m.bit() != 0
    }
}

/// Mouse buttons that can be a trigger. Left and right are left out:
/// they would fire on every ordinary click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Middle,
    X1,
    X2,
}

impl MouseButton {
    const ALL: [MouseButton; 3] = [MouseButton::Middle, MouseButton::X1, MouseButton::X2];

    fn name(self) -> &'static str {
        match self {
            MouseButton::Middle => "MouseMiddle",
            MouseButton::X1 => "Mouse4",
            MouseButton::X2 => "Mouse5",
        }
    }
}

/// A set of gamepad buttons. The bits are XInput's `wButtons`; the two
/// analog triggers take the bits XInput leaves unused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PadButtons(pub u16);

impl PadButtons {
    pub const NONE: PadButtons = PadButtons(0);
    pub const LT: u16 = 0x0400;
    pub const RT: u16 = 0x0800;
    /// An analog trigger counts as pressed from here (0..=255).
    pub const TRIGGER_THRESHOLD: u8 = 128;

    /// Name and bit, in the order a combination is written.
    const NAMES: [(&'static str, u16); 16] = [
        ("Back", 0x0020),
        ("Start", 0x0010),
        ("LB", 0x0100),
        ("RB", 0x0200),
        ("LT", Self::LT),
        ("RT", Self::RT),
        ("LS", 0x0040),
        ("RS", 0x0080),
        ("Up", 0x0001),
        ("Down", 0x0002),
        ("Left", 0x0004),
        ("Right", 0x0008),
        ("A", 0x1000),
        ("B", 0x2000),
        ("X", 0x4000),
        ("Y", 0x8000),
    ];

    /// From an XInput report: digital buttons plus the analog triggers.
    pub fn from_xinput(buttons: u16, left_trigger: u8, right_trigger: u8) -> Self {
        let mut bits = buttons & !(Self::LT | Self::RT);
        if left_trigger >= Self::TRIGGER_THRESHOLD {
            bits |= Self::LT;
        }
        if right_trigger >= Self::TRIGGER_THRESHOLD {
            bits |= Self::RT;
        }
        PadButtons(bits)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    fn contains(self, other: PadButtons) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Mouse {
        mods: Mods,
        button: MouseButton,
    },
    /// A modifier key tapped and pressed again right away.
    Double(Modifier),
    /// Every button of the set down on one gamepad.
    Pad(PadButtons),
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trigger::Mouse { mods, button } => {
                for m in Modifier::ALL {
                    if mods.has(m) {
                        write!(f, "{}+", m.name())?;
                    }
                }
                f.write_str(button.name())
            }
            Trigger::Double(m) => write!(f, "Double:{}", m.name()),
            Trigger::Pad(buttons) => {
                f.write_str("Pad:")?;
                let mut first = true;
                for (name, bit) in PadButtons::NAMES {
                    if buttons.0 & bit != 0 {
                        if !first {
                            f.write_str("+")?;
                        }
                        f.write_str(name)?;
                        first = false;
                    }
                }
                Ok(())
            }
        }
    }
}

/// The text is not a trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError;

impl FromStr for Trigger {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if let Some(key) = strip_prefix_ci(s, "Double:") {
            return Modifier::parse(key.trim())
                .map(Trigger::Double)
                .ok_or(ParseError);
        }
        if let Some(list) = strip_prefix_ci(s, "Pad:") {
            let mut bits = 0u16;
            for part in list.split('+') {
                let part = part.trim();
                let (_, bit) = PadButtons::NAMES
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(part))
                    .ok_or(ParseError)?;
                bits |= bit;
            }
            return Ok(Trigger::Pad(PadButtons(bits)));
        }
        let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let last = parts.pop().ok_or(ParseError)?;
        let button = MouseButton::ALL
            .into_iter()
            .find(|b| b.name().eq_ignore_ascii_case(last))
            .ok_or(ParseError)?;
        let mut mods = Mods::NONE;
        for p in parts {
            mods = mods.with(Modifier::parse(p).ok_or(ParseError)?);
        }
        Ok(Trigger::Mouse { mods, button })
    }
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

/// A keyboard key as far as the detector cares: which modifier, or just
/// "some other key" (what key it was is never looked at or kept).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Modifier(Modifier),
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Key {
        key: Key,
        down: bool,
    },
    /// `mods`: the modifiers held at this moment.
    Mouse {
        button: MouseButton,
        down: bool,
        mods: Mods,
    },
    /// Every button now down on one gamepad (all up when it disconnects).
    Pad {
        slot: usize,
        buttons: PadButtons,
    },
}

/// Where a watcher sends what it sees.
pub type Sink = Arc<dyn Fn(InputEvent) + Send + Sync>;

/// The input devices a set of triggers needs watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sources {
    pub keyboard: bool,
    pub mouse: bool,
    pub pad: bool,
}

impl Sources {
    pub const ALL: Sources = Sources {
        keyboard: true,
        mouse: true,
        pad: true,
    };

    pub fn any(self) -> bool {
        self.keyboard || self.mouse || self.pad
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output<A> {
    /// A bound trigger went down (`pressed`) or up.
    Fire { action: A, pressed: bool },
    /// Capture mode: the trigger the user performed.
    Captured(Trigger),
}

/// Progress towards a double press of a modifier key.
#[derive(Debug, Default)]
struct Tap {
    /// Modifier keys physically down.
    mods: Mods,
    /// A modifier pressed on its own, with nothing else pressed since,
    /// and when it went down.
    candidate: Option<(Modifier, u64)>,
    /// The finished first tap and when it was released.
    last: Option<(Modifier, u64)>,
    /// The second press, still down (it owes a release).
    held: Option<Modifier>,
}

/// Turns input events into trigger outputs. Times are milliseconds on any
/// monotonic clock.
#[derive(Debug)]
pub struct Detector<A> {
    bindings: Vec<(Trigger, A)>,
    capturing: bool,
    tap: Tap,
    /// Mouse buttons now down, with the modifiers they went down with.
    mouse_down: Vec<(MouseButton, Mods)>,
    pads: [PadButtons; PAD_SLOTS],
    /// Capture mode: per gamepad, every button seen since it was at rest.
    pad_seen: [PadButtons; PAD_SLOTS],
}

impl<A: Copy> Default for Detector<A> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            capturing: false,
            tap: Tap::default(),
            mouse_down: Vec::new(),
            pads: [PadButtons::NONE; PAD_SLOTS],
            pad_seen: [PadButtons::NONE; PAD_SLOTS],
        }
    }
}

impl<A: Copy> Detector<A> {
    /// Replace the bindings. Held triggers are forgotten without a
    /// release: the caller ends whatever they were holding.
    pub fn set_bindings(&mut self, bindings: Vec<(Trigger, A)>) {
        *self = Self {
            bindings,
            capturing: self.capturing,
            ..Self::default()
        };
    }

    /// Capture mode: the next trigger performed is reported instead of
    /// firing actions, then the mode ends by itself.
    pub fn set_capturing(&mut self, capturing: bool) {
        *self = Self {
            bindings: std::mem::take(&mut self.bindings),
            capturing,
            ..Self::default()
        };
    }

    pub fn sources(&self) -> Sources {
        if self.capturing {
            return Sources::ALL;
        }
        let mut s = Sources::default();
        for (t, _) in &self.bindings {
            match t {
                Trigger::Mouse { .. } => s.mouse = true,
                Trigger::Double(_) => s.keyboard = true,
                Trigger::Pad(_) => s.pad = true,
            }
        }
        s
    }

    pub fn feed(&mut self, now_ms: u64, event: InputEvent) -> Vec<Output<A>> {
        match event {
            InputEvent::Key { key, down } => self.key(now_ms, key, down),
            InputEvent::Mouse { button, down, mods } => self.mouse(button, down, mods),
            InputEvent::Pad { slot, buttons } => self.pad(slot, buttons),
        }
    }

    fn fire(&self, trigger: Trigger, pressed: bool) -> Vec<Output<A>> {
        self.bindings
            .iter()
            .filter(|(t, _)| *t == trigger)
            .map(|(_, action)| Output::Fire {
                action: *action,
                pressed,
            })
            .collect()
    }

    fn captured(&mut self, trigger: Trigger) -> Vec<Output<A>> {
        self.set_capturing(false);
        vec![Output::Captured(trigger)]
    }

    fn key(&mut self, now: u64, key: Key, down: bool) -> Vec<Output<A>> {
        let Key::Modifier(m) = key else {
            // Ctrl+C and the like: the modifier is in use, not tapped.
            if down {
                self.tap.candidate = None;
                self.tap.last = None;
            }
            return Vec::new();
        };
        if down {
            if self.tap.mods.has(m) {
                return Vec::new(); // key repeat
            }
            let alone = self.tap.mods == Mods::NONE;
            self.tap.mods = self.tap.mods.with(m);
            let last = self.tap.last.take();
            if !alone {
                // Pressed while another modifier is down: a combination,
                // however often it is repeated.
                self.tap.candidate = None;
                return Vec::new();
            }
            match last {
                Some((l, at)) if l == m && now.saturating_sub(at) <= GAP_MAX_MS => {
                    if self.capturing {
                        return self.captured(Trigger::Double(m));
                    }
                    self.tap.held = Some(m);
                    self.fire(Trigger::Double(m), true)
                }
                _ => {
                    self.tap.candidate = Some((m, now));
                    Vec::new()
                }
            }
        } else {
            self.tap.mods = self.tap.mods.without(m);
            if self.tap.held == Some(m) {
                self.tap.held = None;
                return self.fire(Trigger::Double(m), false);
            }
            if let Some((c, since)) = self.tap.candidate {
                if c == m {
                    self.tap.candidate = None;
                    if now.saturating_sub(since) <= TAP_MAX_MS {
                        self.tap.last = Some((m, now));
                    }
                }
            }
            Vec::new()
        }
    }

    fn mouse(&mut self, button: MouseButton, down: bool, mods: Mods) -> Vec<Output<A>> {
        if down {
            if self.capturing {
                return self.captured(Trigger::Mouse { mods, button });
            }
            self.mouse_down.retain(|(b, _)| *b != button);
            self.mouse_down.push((button, mods));
            return self.fire(Trigger::Mouse { mods, button }, true);
        }
        // Released with the modifiers of the press: letting go of Ctrl
        // first must still end what the press started.
        let Some(i) = self.mouse_down.iter().position(|(b, _)| *b == button) else {
            return Vec::new();
        };
        let (_, mods) = self.mouse_down.remove(i);
        self.fire(Trigger::Mouse { mods, button }, false)
    }

    fn pad(&mut self, slot: usize, buttons: PadButtons) -> Vec<Output<A>> {
        let Some(prev) = self.pads.get(slot).copied() else {
            return Vec::new();
        };
        self.pads[slot] = buttons;
        if self.capturing {
            // The combination is complete when the buttons of that
            // gamepad come up again; an idle second gamepad says nothing.
            let seen = PadButtons(self.pad_seen[slot].0 | buttons.0);
            self.pad_seen[slot] = seen;
            if buttons.is_empty() && !seen.is_empty() {
                return self.captured(Trigger::Pad(seen));
            }
            return Vec::new();
        }
        // A combination held on two gamepads is one press: it fires when
        // the first one completes it and releases with the last one.
        let elsewhere =
            |combo| (self.pads.iter().enumerate()).any(|(i, b)| i != slot && b.contains(combo));
        let mut out = Vec::new();
        for (t, action) in &self.bindings {
            let Trigger::Pad(combo) = *t else { continue };
            if elsewhere(combo) {
                continue;
            }
            let (was, is) = (prev.contains(combo), buttons.contains(combo));
            if was != is {
                out.push(Output::Fire {
                    action: *action,
                    pressed: is,
                });
            }
        }
        out
    }
}

/// Decoding of Win32 raw input (kept here, free of Win32 types, so it is
/// tested on every platform).
pub mod raw {
    use super::{InputEvent, Key, Modifier, Mods, MouseButton};

    /// `RAWMOUSE::usButtonFlags` -> button events.
    pub fn mouse(flags: u16, mods: Mods) -> Vec<InputEvent> {
        const TABLE: [(u16, MouseButton, bool); 6] = [
            (0x0010, MouseButton::Middle, true),
            (0x0020, MouseButton::Middle, false),
            (0x0040, MouseButton::X1, true),
            (0x0080, MouseButton::X1, false),
            (0x0100, MouseButton::X2, true),
            (0x0200, MouseButton::X2, false),
        ];
        TABLE
            .iter()
            .filter(|(bit, _, _)| flags & bit != 0)
            .map(|&(_, button, down)| InputEvent::Mouse { button, down, mods })
            .collect()
    }

    /// `RAWKEYBOARD::VKey` / `Flags` -> key event. `None` for the 0xFF
    /// filler that escaped scan-code sequences produce.
    pub fn key(vkey: u16, flags: u16) -> Option<InputEvent> {
        const RI_KEY_BREAK: u16 = 1;
        let key = match vkey {
            0xFF => return None,
            0x11 | 0xA2 | 0xA3 => Key::Modifier(Modifier::Control),
            0x12 | 0xA4 | 0xA5 => Key::Modifier(Modifier::Alt),
            0x10 | 0xA0 | 0xA1 => Key::Modifier(Modifier::Shift),
            _ => Key::Other,
        };
        Some(InputEvent::Key {
            key,
            down: flags & RI_KEY_BREAK == 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Key = Key::Modifier(Modifier::Control);
    const SHIFT: Key = Key::Modifier(Modifier::Shift);

    fn key(key: Key, down: bool) -> InputEvent {
        InputEvent::Key { key, down }
    }

    fn pad(bits: u16) -> InputEvent {
        InputEvent::Pad {
            slot: 0,
            buttons: PadButtons(bits),
        }
    }

    fn detector(bindings: &[(&str, u8)]) -> Detector<u8> {
        let mut d = Detector::default();
        d.set_bindings(
            bindings
                .iter()
                .map(|(t, a)| (t.parse().unwrap(), *a))
                .collect(),
        );
        d
    }

    /// Feed `(time, event)` pairs and collect everything that comes out.
    fn run(d: &mut Detector<u8>, events: &[(u64, InputEvent)]) -> Vec<Output<u8>> {
        events.iter().flat_map(|(t, e)| d.feed(*t, *e)).collect()
    }

    fn fired(action: u8, pressed: bool) -> Output<u8> {
        Output::Fire { action, pressed }
    }

    #[test]
    fn text_form_round_trips() {
        for s in [
            "Mouse4",
            "MouseMiddle",
            "Control+Shift+Mouse5",
            "Alt+Mouse4",
            "Double:Control",
            "Double:Shift",
            "Pad:Back+Y",
            "Pad:LB+RB+LT+RT",
            "Pad:Start",
        ] {
            let t: Trigger = s.parse().unwrap_or_else(|_| panic!("{s}"));
            assert_eq!(t.to_string(), s);
        }
    }

    #[test]
    fn text_form_is_normalised() {
        let t = |s: &str| s.parse::<Trigger>().unwrap().to_string();
        assert_eq!(t("shift+CommandOrControl+mouse4"), "Control+Shift+Mouse4");
        assert_eq!(t("pad:y+back"), "Pad:Back+Y");
        assert_eq!(t(" double:ctrl "), "Double:Control");
    }

    #[test]
    fn rejects_what_is_not_a_trigger() {
        for s in [
            "",
            "Control+Shift+Space",
            "MouseLeft",
            "Mouse4+Control",
            "Double:A",
            "Pad:",
            "Pad:Back+Q",
        ] {
            assert!(s.parse::<Trigger>().is_err(), "{s}");
        }
    }

    #[test]
    fn double_press_fires_on_the_second_press_and_releases_with_it() {
        let mut d = detector(&[("Double:Control", 1)]);
        let out = run(
            &mut d,
            &[
                (0, key(CTRL, true)),
                (80, key(CTRL, false)),
                (200, key(CTRL, true)),
                (230, key(CTRL, true)), // key repeat while held
                (900, key(CTRL, false)),
            ],
        );
        assert_eq!(out, [fired(1, true), fired(1, false)]);
    }

    #[test]
    fn double_press_needs_both_taps_close_and_short() {
        let mut d = detector(&[("Double:Control", 1)]);
        // Second press too late.
        let late = [
            (0, key(CTRL, true)),
            (80, key(CTRL, false)),
            (80 + GAP_MAX_MS + 1, key(CTRL, true)),
            (700, key(CTRL, false)),
        ];
        assert_eq!(run(&mut d, &late), []);
        // First press held too long to be a tap.
        let mut d = detector(&[("Double:Control", 1)]);
        let long = [
            (0, key(CTRL, true)),
            (TAP_MAX_MS + 1, key(CTRL, false)),
            (TAP_MAX_MS + 50, key(CTRL, true)),
        ];
        assert_eq!(run(&mut d, &long), []);
    }

    #[test]
    fn shortcuts_typed_twice_are_not_a_double_press() {
        let mut d = detector(&[("Double:Control", 1), ("Double:Shift", 2)]);
        // Ctrl+C, Ctrl+V in quick succession.
        let copy_paste = [
            (0, key(CTRL, true)),
            (30, key(Key::Other, true)),
            (60, key(Key::Other, false)),
            (90, key(CTRL, false)),
            (150, key(CTRL, true)),
            (180, key(Key::Other, true)),
            (240, key(CTRL, false)),
        ];
        assert_eq!(run(&mut d, &copy_paste), []);
        // A key between the taps.
        let between = [
            (1000, key(CTRL, true)),
            (1050, key(CTRL, false)),
            (1100, key(Key::Other, true)),
            (1150, key(CTRL, true)),
            (1200, key(CTRL, false)),
        ];
        assert_eq!(run(&mut d, &between), []);
        // Ctrl+Shift pressed together, twice.
        let combo = [
            (2000, key(CTRL, true)),
            (2010, key(SHIFT, true)),
            (2050, key(SHIFT, false)),
            (2060, key(CTRL, false)),
            (2100, key(CTRL, true)),
            (2110, key(SHIFT, true)),
            (2150, key(SHIFT, false)),
            (2160, key(CTRL, false)),
        ];
        assert_eq!(run(&mut d, &combo), []);
    }

    #[test]
    fn a_tap_of_another_modifier_does_not_complete_the_pair() {
        let mut d = detector(&[("Double:Control", 1), ("Double:Shift", 2)]);
        let out = run(
            &mut d,
            &[
                (0, key(CTRL, true)),
                (50, key(CTRL, false)),
                (100, key(SHIFT, true)),
                (150, key(SHIFT, false)),
                (200, key(SHIFT, true)),
                (250, key(SHIFT, false)),
            ],
        );
        assert_eq!(out, [fired(2, true), fired(2, false)]);
    }

    #[test]
    fn mouse_button_matches_its_modifiers_exactly() {
        let ctrl = Mods::NONE.with(Modifier::Control);
        let mouse = |button, down, mods| InputEvent::Mouse { button, down, mods };
        let mut d = detector(&[("Mouse4", 1), ("Control+Mouse4", 2)]);
        let out = run(
            &mut d,
            &[
                (0, mouse(MouseButton::X1, true, Mods::NONE)),
                (10, mouse(MouseButton::X1, false, Mods::NONE)),
                (20, mouse(MouseButton::X1, true, ctrl)),
                // Ctrl released before the button: still the same press.
                (30, mouse(MouseButton::X1, false, Mods::NONE)),
                (40, mouse(MouseButton::X2, true, Mods::NONE)),
                (50, mouse(MouseButton::X2, false, Mods::NONE)),
            ],
        );
        assert_eq!(
            out,
            [
                fired(1, true),
                fired(1, false),
                fired(2, true),
                fired(2, false)
            ]
        );
    }

    #[test]
    fn pad_combination_fires_when_complete_and_releases_when_broken() {
        let (back, y, a) = (0x0020, 0x8000, 0x1000);
        let mut d = detector(&[("Pad:Back+Y", 1)]);
        let out = run(
            &mut d,
            &[
                (0, pad(y)),
                (10, pad(0)),
                (20, pad(back)),
                (30, pad(back | y)),
                (40, pad(back | y | a)), // more buttons: still held
                (50, pad(y | a)),
                (60, pad(0)),
            ],
        );
        assert_eq!(out, [fired(1, true), fired(1, false)]);
    }

    #[test]
    fn a_disconnected_pad_releases_and_other_slots_are_separate() {
        let mut d = detector(&[("Pad:Back+Y", 1)]);
        let slot = |slot, bits| InputEvent::Pad {
            slot,
            buttons: PadButtons(bits),
        };
        // Back on one pad and Y on another is not the combination.
        assert_eq!(
            run(&mut d, &[(0, slot(0, 0x0020)), (1, slot(1, 0x8000))]),
            []
        );
        let out = run(&mut d, &[(2, slot(1, 0x8020)), (3, slot(1, 0))]);
        assert_eq!(out, [fired(1, true), fired(1, false)]);
        assert_eq!(run(&mut d, &[(4, slot(PAD_SLOTS, 0x8020))]), []);
    }

    #[test]
    fn capture_reports_the_trigger_once_and_fires_nothing() {
        let mouse = |button, down| InputEvent::Mouse {
            button,
            down,
            mods: Mods::NONE.with(Modifier::Shift),
        };
        let mut d = detector(&[("Shift+Mouse5", 1)]);
        d.set_capturing(true);
        assert_eq!(d.sources(), Sources::ALL);
        let out = run(&mut d, &[(0, mouse(MouseButton::X2, true))]);
        assert_eq!(out, [Output::Captured("Shift+Mouse5".parse().unwrap())]);
        // The mode ended: the binding works again.
        let out = run(
            &mut d,
            &[
                (10, mouse(MouseButton::X2, false)),
                (20, mouse(MouseButton::X2, true)),
            ],
        );
        assert_eq!(out, [fired(1, true)]);
    }

    #[test]
    fn capture_takes_a_double_press_and_a_whole_pad_combination() {
        let mut d = detector(&[]);
        d.set_capturing(true);
        let out = run(
            &mut d,
            &[
                (0, key(SHIFT, true)),
                (50, key(SHIFT, false)),
                (100, key(SHIFT, true)),
            ],
        );
        assert_eq!(out, [Output::Captured(Trigger::Double(Modifier::Shift))]);

        d.set_capturing(true);
        let out = run(
            &mut d,
            &[
                (0, pad(0x0020)),
                (10, pad(0x8020)),
                (20, pad(0x8000)),
                (30, pad(0)),
            ],
        );
        assert_eq!(out, [Output::Captured("Pad:Back+Y".parse().unwrap())]);
    }

    #[test]
    fn taps_while_another_modifier_is_down_are_a_combination() {
        let mut d = detector(&[("Double:Control", 1), ("Double:Shift", 2)]);
        // Ctrl stays down while Shift is tapped three times.
        let out = run(
            &mut d,
            &[
                (0, key(CTRL, true)),
                (100, key(SHIFT, true)),
                (150, key(SHIFT, false)),
                (200, key(SHIFT, true)),
                (250, key(SHIFT, false)),
                (300, key(SHIFT, true)),
                (350, key(SHIFT, false)),
                (400, key(CTRL, false)),
            ],
        );
        assert_eq!(out, []);
        // The same while a double press of Ctrl is being held: only Ctrl
        // fires, and it is released.
        let out = run(
            &mut d,
            &[
                (1000, key(CTRL, true)),
                (1050, key(CTRL, false)),
                (1100, key(CTRL, true)),
                (1500, key(SHIFT, true)),
                (1550, key(SHIFT, false)),
                (1600, key(SHIFT, true)),
                (1900, key(CTRL, false)),
                (1950, key(SHIFT, false)),
            ],
        );
        assert_eq!(out, [fired(1, true), fired(1, false)]);
        // Afterwards a plain double press works again.
        let out = run(
            &mut d,
            &[
                (3000, key(SHIFT, true)),
                (3050, key(SHIFT, false)),
                (3100, key(SHIFT, true)),
                (3150, key(SHIFT, false)),
            ],
        );
        assert_eq!(out, [fired(2, true), fired(2, false)]);
    }

    #[test]
    fn a_combination_on_two_pads_is_one_press() {
        let slot = |slot, bits| InputEvent::Pad {
            slot,
            buttons: PadButtons(bits),
        };
        let mut d = detector(&[("Pad:Back+Y", 1)]);
        let out = run(
            &mut d,
            &[
                (0, slot(0, 0x8020)),
                (1, slot(1, 0x8020)),
                (2, slot(0, 0)), // still held on the second pad
                (3, slot(1, 0)),
            ],
        );
        assert_eq!(out, [fired(1, true), fired(1, false)]);
    }

    #[test]
    fn capture_is_not_ended_by_an_idle_second_pad() {
        let slot = |slot, bits| InputEvent::Pad {
            slot,
            buttons: PadButtons(bits),
        };
        let mut d = detector(&[]);
        d.set_capturing(true);
        let out = run(
            &mut d,
            &[
                (0, slot(0, 0x0020)), // Back held on the first pad
                (1, slot(1, 0)),      // the second pad reports in, idle
                (2, slot(0, 0x8020)),
                (3, slot(0, 0)),
            ],
        );
        assert_eq!(out, [Output::Captured("Pad:Back+Y".parse().unwrap())]);
    }

    #[test]
    fn only_the_devices_in_use_are_watched() {
        assert!(!detector(&[]).sources().any());
        assert_eq!(
            detector(&[("Mouse4", 1)]).sources(),
            Sources {
                mouse: true,
                ..Sources::default()
            }
        );
        assert_eq!(
            detector(&[("Double:Alt", 1), ("Pad:A", 2)]).sources(),
            Sources {
                keyboard: true,
                mouse: false,
                pad: true
            }
        );
    }

    #[test]
    fn raw_input_is_decoded() {
        let none = Mods::NONE;
        assert_eq!(raw::mouse(0x0001 | 0x0004, none), []); // left, right
        assert_eq!(
            raw::mouse(0x0040 | 0x0200, none),
            [
                InputEvent::Mouse {
                    button: MouseButton::X1,
                    down: true,
                    mods: none
                },
                InputEvent::Mouse {
                    button: MouseButton::X2,
                    down: false,
                    mods: none
                },
            ]
        );
        assert_eq!(raw::key(0x11, 0), Some(key(CTRL, true)));
        assert_eq!(raw::key(0xA1, 1), Some(key(SHIFT, false)));
        assert_eq!(raw::key(0x41, 2), Some(key(Key::Other, true)));
        assert_eq!(raw::key(0xFF, 0), None);
    }

    #[test]
    fn analog_triggers_join_the_buttons() {
        let b = PadButtons::from_xinput(0x1000 | 0x0400, 200, 10);
        assert_eq!(Trigger::Pad(b).to_string(), "Pad:LT+A");
        // The undocumented bits XInput may set are not LT / RT.
        assert_eq!(PadButtons::from_xinput(0x0C00, 0, 0), PadButtons::NONE);
    }
}
