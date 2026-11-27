//! Held "show the original" key of window translation (pure state).
//!
//! The translations of every window are hidden while the key is down.
//! Some platforms report auto-repeat as release/press pairs, so a
//! release only counts once [`RELEASE_GRACE_MS`] passed without a press.

/// How long after a release the translations come back.
pub const RELEASE_GRACE_MS: u64 = 80;

/// What holds the original: the keyboard shortcut or the extra trigger
/// (`trigger.rs`). Both can be down at once; the translations come back
/// when the last one is released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldSource {
    Shortcut,
    Trigger,
}

#[derive(Debug, Default)]
pub struct Hold {
    shown: bool,
    seq: u64,
    /// Which sources are down, indexed by [`HoldSource`].
    down: [bool; 2],
}

impl Hold {
    /// Key down. True when the original has to be shown now.
    pub fn press(&mut self, source: HoldSource) -> bool {
        self.down[source as usize] = true;
        self.seq += 1;
        !std::mem::replace(&mut self.shown, true)
    }

    /// The original is shown (key down, or released within the grace).
    pub fn shown(&self) -> bool {
        self.shown
    }

    /// The key binding is gone, and its release with it. True when the
    /// translations have to come back now.
    pub fn cancel(&mut self) -> bool {
        self.down = [false; 2];
        self.seq += 1;
        std::mem::replace(&mut self.shown, false)
    }

    /// Key up. Returns the ticket for [`Hold::settle`].
    pub fn release(&mut self, source: HoldSource) -> u64 {
        self.down[source as usize] = false;
        self.seq += 1;
        self.seq
    }

    /// The grace after the release with `ticket` is over. True when the
    /// translations have to come back now (no key event since, and no
    /// other source still down).
    pub fn settle(&mut self, ticket: u64) -> bool {
        ticket == self.seq && self.down == [false; 2] && std::mem::replace(&mut self.shown, false)
    }
}

#[cfg(test)]
mod tests {
    use super::HoldSource::{Shortcut, Trigger};
    use super::*;

    #[test]
    fn press_shows_and_settled_release_hides() {
        let mut h = Hold::default();
        assert!(h.press(Shortcut));
        assert!(h.shown());
        let t = h.release(Shortcut);
        assert!(h.settle(t));
        assert!(!h.shown());
    }

    #[test]
    fn auto_repeat_keeps_the_original_shown() {
        let mut h = Hold::default();
        assert!(h.press(Shortcut));
        let first = h.release(Shortcut);
        // Repeat: pressed again before the grace is over.
        assert!(!h.press(Shortcut));
        assert!(!h.settle(first));
        let last = h.release(Shortcut);
        assert!(h.settle(last));
    }

    #[test]
    fn cancel_ends_a_hold_and_outdates_its_release() {
        let mut h = Hold::default();
        assert!(!h.cancel());
        assert!(h.press(Shortcut));
        let t = h.release(Shortcut);
        assert!(h.cancel());
        assert!(!h.settle(t));
        assert!(!h.shown());
    }

    #[test]
    fn the_original_stays_until_the_last_source_is_released() {
        let mut h = Hold::default();
        assert!(h.press(Shortcut));
        assert!(!h.press(Trigger));
        let first = h.release(Shortcut);
        assert!(!h.settle(first));
        assert!(h.shown());
        let last = h.release(Trigger);
        assert!(h.settle(last));
        assert!(!h.shown());
    }

    #[test]
    fn a_tap_after_a_lost_release_recovers() {
        let mut h = Hold::default();
        assert!(h.press(Shortcut));
        // The release never arrived; the next tap ends it.
        assert!(!h.press(Shortcut));
        let t = h.release(Shortcut);
        assert!(h.settle(t));
        assert!(!h.settle(t));
    }
}
