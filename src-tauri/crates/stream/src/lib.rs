//! Composite window for stream capture.
//!
//! Streaming and recording tools capture one window at a time, and the
//! translations MoonLens shows live in windows of their own: a capture
//! of the game window alone has no translations in it. A [`Compositor`]
//! therefore opens one more window that shows the other app's window
//! (mirrored through Windows.Graphics.Capture) with the translations
//! drawn over it, for the streaming tool to capture instead.
//!
//! What is drawn comes from the translation windows as a [`Scene`]: the
//! boxes and text lines exactly as they laid them out, so the composite
//! shows what the screen shows. Windows only; elsewhere
//! [`Compositor::start`] fails with [`Error::Unsupported`].

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod win;

/// Whether this platform can show a composite window.
pub const SUPPORTED: bool = cfg!(windows);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// One line of text. `rect` is `[x, y, w, h]` in physical pixels from
/// the top-left corner of its window's client area.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub text: String,
    pub rect: [f32; 4],
    /// Width the text takes in the translation window. The line is
    /// drawn no wider, whatever the font here measures.
    pub text_width: f32,
    /// The text runs past its box: the box colour is painted behind it.
    #[serde(default)]
    pub backed: bool,
}

/// One translated paragraph: a box that covers the source text and the
/// lines laid over it. Coordinates as in [`Line`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub rect: [f32; 4],
    /// The parts of `rect` the background covers, when it is not all
    /// of it (rows that end before a picture beside them).
    #[serde(default)]
    pub fills: Vec<[f32; 4]>,
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    #[serde(default)]
    pub bold: bool,
    /// Font size in physical pixels.
    pub font_px: f32,
    #[serde(default)]
    pub align: Align,
    #[serde(default)]
    pub lines: Vec<Line>,
}

/// What one translation window shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layer {
    /// Native handle of the translation window; the items are placed
    /// by where its client area is on the screen at each frame.
    pub window: isize,
    pub items: Vec<Item>,
}

pub type Scene = Vec<Layer>;

#[derive(Debug)]
pub enum Error {
    /// No composite window on this platform or this Windows version.
    Unsupported,
    /// The window to mirror is gone.
    TargetGone,
    Os(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported => f.write_str("composite windows are not supported here"),
            Error::TargetGone => f.write_str("the window to mirror is gone"),
            Error::Os(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for Error {}

/// A running composite window. Dropping it closes the window.
pub struct Compositor {
    #[cfg(windows)]
    inner: win::Running,
}

impl Compositor {
    /// Open a composite window titled `title` that mirrors window
    /// `target` (native handle). `on_end` runs once when it closes for
    /// whatever reason: [`Compositor::stop`], the target closing, the
    /// user closing the composite window, or a failure.
    pub fn start(
        target: isize,
        title: &str,
        on_end: impl FnOnce() + Send + 'static,
    ) -> Result<Self, Error> {
        #[cfg(windows)]
        {
            win::start(target, title, Box::new(on_end)).map(|inner| Self { inner })
        }
        #[cfg(not(windows))]
        {
            let _ = (target, title, on_end);
            Err(Error::Unsupported)
        }
    }

    /// Replace what is drawn over the mirrored window.
    pub fn set_scene(&self, scene: Scene) {
        #[cfg(windows)]
        self.inner.set_scene(scene);
        #[cfg(not(windows))]
        let _ = scene;
    }

    /// Native handle of the composite window.
    pub fn window(&self) -> isize {
        #[cfg(windows)]
        {
            self.inner.window
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    /// Close the composite window.
    pub fn stop(&self) {
        #[cfg(windows)]
        self.inner.stop();
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        self.stop();
    }
}
