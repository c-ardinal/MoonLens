//! Held "show the original" key of window translation (pure state).
//!
//! The translations of every window are hidden while the key is down.
//! Some platforms report auto-repeat as release/press pairs, so a
//! release only counts once [`RELEASE_GRACE_MS`] passed without a press.

/// How long after a release the translations come back.
pub const RELEASE_GRACE_MS: u64 = 80;

#[derive(Debug, Default)]
pub struct Hold {
    shown: bool,
    seq: u64,
}

impl Hold {
    /// Key down. True when the original has to be shown now.
    pub fn press(&mut self) -> bool {
        self.seq += 1;
        !std::mem::replace(&mut self.shown, true)
    }

    /// Key up. Returns the ticket for [`Hold::settle`].
    pub fn release(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    /// The grace after the release with `ticket` is over. True when the
    /// translations have to come back now (no key event since).
    pub fn settle(&mut self, ticket: u64) -> bool {
        ticket == self.seq && std::mem::replace(&mut self.shown, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_shows_and_settled_release_hides() {
        let mut h = Hold::default();
        assert!(h.press());
        let t = h.release();
        assert!(h.settle(t));
    }

    #[test]
    fn auto_repeat_keeps_the_original_shown() {
        let mut h = Hold::default();
        assert!(h.press());
        let first = h.release();
        // Repeat: pressed again before the grace is over.
        assert!(!h.press());
        assert!(!h.settle(first));
        let last = h.release();
        assert!(h.settle(last));
    }

    #[test]
    fn a_tap_after_a_lost_release_recovers() {
        let mut h = Hold::default();
        assert!(h.press());
        // The release never arrived; the next tap ends it.
        assert!(!h.press());
        let t = h.release();
        assert!(h.settle(t));
        assert!(!h.settle(t));
    }
}
