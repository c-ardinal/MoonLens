//! Whether a lens window is being dragged or resized right now.
//!
//! Reading a region costs OCR and model time, and a window that is on
//! its way somewhere else covers nothing worth reading yet. A pause of
//! the hand during a drag is not a drop either: readings wait until the
//! mouse button is released. Where the button state is unknown, a short
//! stretch without movement counts as the drop.

/// Poll interval while a move is in progress, so the drop is noticed
/// right away rather than a full watcher interval later.
pub const MOVING_POLL_MS: u64 = 60;

/// Polls without movement that count as a drop where the mouse button
/// state is unknown (about a third of a second).
const SETTLE_POLLS: u32 = 5;

/// Follows one window from the first movement to the drop.
#[derive(Debug, Default)]
pub struct DragGate {
    moving: bool,
    still: u32,
}

impl DragGate {
    /// A move or resize is in progress (poll at [`MOVING_POLL_MS`]).
    pub fn moving(&self) -> bool {
        self.moving
    }

    /// One poll: `moved` says the window's geometry changed since the
    /// last poll, `button_down` whether a mouse button is held (`None`:
    /// unknown). Returns whether the region may be read now.
    pub fn allow(&mut self, moved: bool, button_down: Option<bool>) -> bool {
        if moved {
            self.moving = true;
            self.still = 0;
            return false;
        }
        if !self.moving {
            return true;
        }
        let dropped = match button_down {
            Some(down) => !down,
            None => {
                self.still += 1;
                self.still >= SETTLE_POLLS
            }
        };
        if dropped {
            self.moving = false;
            self.still = 0;
        }
        dropped
    }
}

/// Whether a mouse button is held down, anywhere on the screen. `None`
/// where the platform gives no answer.
#[cfg(windows)]
pub fn button_down() -> Option<bool> {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(v_key: i32) -> i16;
    }
    const VK_LBUTTON: i32 = 0x01;
    const VK_RBUTTON: i32 = 0x02;
    // Physical buttons: with swapped buttons the right one drags.
    // SAFETY: plain query without pointers.
    let down = |key| (unsafe { GetAsyncKeyState(key) }) < 0;
    Some(down(VK_LBUTTON) || down(VK_RBUTTON))
}

#[cfg(target_os = "macos")]
pub fn button_down() -> Option<bool> {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceButtonState(state_id: i32, button: u32) -> bool;
    }
    const COMBINED_SESSION_STATE: i32 = 0;
    const LEFT: u32 = 0;
    const RIGHT: u32 = 1;
    // SAFETY: plain query without pointers.
    Some(unsafe {
        CGEventSourceButtonState(COMBINED_SESSION_STATE, LEFT)
            || CGEventSourceButtonState(COMBINED_SESSION_STATE, RIGHT)
    })
}

#[cfg(target_os = "linux")]
pub fn button_down() -> Option<bool> {
    use parking_lot::Mutex;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, KeyButMask};
    use x11rb::rust_connection::RustConnection;

    static X: Mutex<Option<(RustConnection, u32)>> = Mutex::new(None);
    let mut x = X.lock();
    if x.is_none() {
        *x = x11rb::connect(None).ok().map(|(c, screen)| {
            let root = c.setup().roots[screen].root;
            (c, root)
        });
    }
    let reply = x
        .as_ref()
        .and_then(|(c, root)| c.query_pointer(*root).ok()?.reply().ok());
    let Some(reply) = reply else {
        // No X server (plain Wayland): unknown.
        *x = None;
        return None;
    };
    let held = KeyButMask::BUTTON1 | KeyButMask::BUTTON2 | KeyButMask::BUTTON3;
    Some(u16::from(reply.mask) & u16::from(held) != 0)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn button_down() -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_at_rest_is_read() {
        let mut gate = DragGate::default();
        assert!(gate.allow(false, Some(false)));
        assert!(
            gate.allow(false, Some(true)),
            "a held button alone is no drag"
        );
    }

    #[test]
    fn nothing_is_read_until_the_button_is_released() {
        let mut gate = DragGate::default();
        assert!(!gate.allow(true, Some(true)));
        // The hand pauses mid-drag: still no reading, however long.
        for _ in 0..50 {
            assert!(!gate.allow(false, Some(true)));
            assert!(gate.moving());
        }
        assert!(!gate.allow(true, Some(true)));
        // Drop.
        assert!(gate.allow(false, Some(false)));
        assert!(!gate.moving());
        assert!(gate.allow(false, Some(false)));
    }

    #[test]
    fn a_move_without_the_mouse_is_read_at_once() {
        // Keyboard snap, maximize: the geometry changes with no button held.
        let mut gate = DragGate::default();
        assert!(!gate.allow(true, Some(false)));
        assert!(gate.allow(false, Some(false)));
    }

    #[test]
    fn unknown_button_state_waits_for_the_window_to_hold_still() {
        let mut gate = DragGate::default();
        assert!(!gate.allow(true, None));
        for _ in 0..SETTLE_POLLS - 1 {
            assert!(!gate.allow(false, None));
        }
        // Moving again starts the wait over.
        assert!(!gate.allow(true, None));
        for _ in 0..SETTLE_POLLS - 1 {
            assert!(!gate.allow(false, None));
        }
        assert!(gate.allow(false, None));
        assert!(!gate.moving());
    }
}
