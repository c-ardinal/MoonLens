//! Attaching one of our windows to a window of another app: it snaps
//! onto that window and stays with it as the app moves and resizes it.
//!
//! One thread per attachment asks the window system where the target is
//! ([`foreign_window::probe`]), often while it moves and seldom while it
//! holds still, and hands every change to the main thread, where
//! [`Follow`] decides the place and the window is moved in the same
//! turn. A minimized target hides the follower; a closed one ends the
//! attachment. The follower hears about every change of its attachment
//! through [`EVENT`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager, State, WebviewWindow};

use crate::app_identity::AppIdentity;
use crate::follow::{self, Dock, Follow};
use crate::foreign_window::{self, ForeignWindow, Probe, PROBE_IDLE_MS, PROBE_MOVING_MS};
use crate::lens::{self, geometry::LensOuterRect as Rect};

/// Event with the [`Attachment`] of the window it is sent to, on every
/// change.
pub const EVENT: &str = "moonlens://attach";

/// How long after the target's last move it is still asked for at the
/// fast rate, so a drag that pauses is picked up at once.
const MOVING_FOR: Duration = Duration::from_millis(600);
/// A target at rest: how often the follower is looked at anyway, which
/// notices what the user did to it.
const CHECK_EVERY: Duration = Duration::from_millis(100);

/// The window a follower is attached to.
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub id: u64,
    pub pid: u32,
    pub title: String,
    pub app: String,
    /// The application the window belongs to, as it is remembered
    /// across launches.
    pub identity: Option<AppIdentity>,
}

/// The application window `window` belongs to: by its process, else by
/// the name the window list gives.
pub fn identity_of(window: &ForeignWindow) -> Option<AppIdentity> {
    AppIdentity::of_pid(window.pid).or_else(|| AppIdentity::from_executable(&window.app))
}

/// What a follower is attached to and how (`target: None`: nothing).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Attachment {
    pub target: Option<Target>,
    pub dock: Option<Dock>,
}

struct Entry {
    /// Tells this attachment from an earlier one of the same follower.
    serial: u64,
    target: Target,
    follow: Follow,
    /// The follower's own header (CSS pixels; see [`Follow::with_header`]).
    header_css: f64,
    /// Where the target was last seen; `None` while it is hidden.
    anchor: Option<Rect>,
    /// The follower is hidden along with its target.
    hidden: bool,
    /// A step is on its way to the main thread: the next change only
    /// replaces `anchor`.
    pending: bool,
}

/// The attachments of our windows, by window label.
#[derive(Default)]
pub struct Attachments {
    entries: Mutex<HashMap<String, Entry>>,
    serial: AtomicU64,
}

pub type Handle = Arc<Attachments>;

impl Attachments {
    pub fn attachment(&self, label: &str) -> Attachment {
        match self.entries.lock().get(label) {
            Some(e) => Attachment {
                target: Some(e.target.clone()),
                dock: Some(e.follow.dock()),
            },
            None => Attachment::default(),
        }
    }

    /// The windows attached to window `target`.
    pub fn attached_to(&self, target: u64) -> Vec<String> {
        let mut labels: Vec<String> = self
            .entries
            .lock()
            .iter()
            .filter(|(_, e)| e.target.id == target)
            .map(|(label, _)| label.clone())
            .collect();
        labels.sort();
        labels
    }

    /// The application `label` is attached to a window of.
    pub fn app(&self, label: &str) -> Option<AppIdentity> {
        self.entries
            .lock()
            .get(label)
            .and_then(|e| e.target.identity.clone())
    }

    /// Whether `label` is hidden along with the window it follows.
    pub fn hidden(&self, label: &str) -> bool {
        self.entries.lock().get(label).is_some_and(|e| e.hidden)
    }

    /// Drop the attachment of a window that is closing.
    pub fn forget(&self, label: &str) {
        self.entries.lock().remove(label);
    }

    fn current(&self, label: &str, serial: u64) -> bool {
        self.entries
            .lock()
            .get(label)
            .is_some_and(|e| e.serial == serial)
    }
}

fn emit(app: &AppHandle, registry: &Attachments, label: &str) {
    let target = EventTarget::WebviewWindow {
        label: label.to_string(),
    };
    let _ = app.emit_to(target, EVENT, registry.attachment(label));
    crate::stream::attachment_changed(app, label);
}

/// Attach window `label` to `target` at `dock`. The top `header_css`
/// CSS pixels of the follower are its own header (see
/// [`Follow::with_header`]). An earlier attachment is replaced.
pub fn attach(
    app: &AppHandle,
    registry: &Handle,
    label: &str,
    target: &ForeignWindow,
    dock: Dock,
    header_css: f64,
) -> Result<()> {
    let win = app
        .get_webview_window(label)
        .ok_or_else(|| anyhow!("window {label} not found"))?;
    if win.is_maximized().unwrap_or(false) {
        let _ = win.unmaximize();
    }
    let scale = win.scale_factor().unwrap_or(1.0);
    let mut follow = Follow::new(dock, scale).with_header(header_px(header_css, scale));
    // Docked, the window keeps its own size along the free axis: the
    // one it had before a dock stretched it.
    let own = registry
        .entries
        .lock()
        .get(label)
        .filter(|e| e.follow.stretches())
        .map(|e| e.follow.size())
        .or_else(|| win.inner_size().ok().map(|s| (s.width, s.height)));
    if let Some((width, height)) = own {
        follow = follow.with_size(width, height);
    }
    let serial = registry.serial.fetch_add(1, Ordering::Relaxed) + 1;
    let was_hidden = registry
        .entries
        .lock()
        .insert(
            label.to_string(),
            Entry {
                serial,
                target: Target {
                    id: target.id,
                    pid: target.pid,
                    title: target.title.clone(),
                    app: target.app.clone(),
                    identity: identity_of(target),
                },
                follow,
                header_css,
                anchor: None,
                hidden: false,
                pending: false,
            },
        )
        .is_some_and(|old| old.hidden);
    if was_hidden {
        follow::show_inactive(&win);
    }
    let (app_bg, registry_bg, label_bg, id) =
        (app.clone(), registry.clone(), label.to_string(), target.id);
    std::thread::Builder::new()
        .name("attach".into())
        .spawn(move || run(&app_bg, &registry_bg, &label_bg, serial, id))?;
    emit(app, registry, label);
    Ok(())
}

fn header_px(header_css: f64, scale: f64) -> u32 {
    (header_css * scale).ceil() as u32
}

/// Change how `label` sits on its target.
pub fn set_dock(app: &AppHandle, registry: &Handle, label: &str, dock: Dock) {
    let anchor = {
        let mut entries = registry.entries.lock();
        let Some(e) = entries.get_mut(label) else {
            return;
        };
        e.follow.set_dock(dock);
        e.anchor.map(|a| (e.serial, a))
    };
    if let Some((serial, anchor)) = anchor {
        post(app, registry, label, serial, Some(anchor));
    }
    emit(app, registry, label);
}

/// End the attachment of `label`; the window stays where it is. A
/// dock stretched it along the target: it gets its own size back.
pub fn detach(app: &AppHandle, registry: &Handle, label: &str) {
    let Some(old) = registry.entries.lock().remove(label) else {
        return;
    };
    if let Some(win) = app.get_webview_window(label) {
        if old.follow.stretches() {
            let (width, height) = old.follow.size();
            let _ = win.set_size(tauri::PhysicalSize::new(width, height));
        }
        if old.hidden {
            follow::show_inactive(&win);
        }
    }
    emit(app, registry, label);
}

/// Follow window `id` for the attachment `serial` of `label` until that
/// attachment ends.
fn run(app: &AppHandle, registry: &Handle, label: &str, serial: u64, id: u64) {
    let mut last: Option<Rect> = None;
    let mut moved_at = Instant::now();
    let mut posted_at = Instant::now();
    while registry.current(label, serial) {
        match foreign_window::probe(id) {
            Probe::Gone => {
                if registry.current(label, serial) {
                    detach(app, registry, label);
                }
                return;
            }
            Probe::Hidden => {
                if last.take().is_some() || posted_at.elapsed() >= CHECK_EVERY {
                    post(app, registry, label, serial, None);
                    posted_at = Instant::now();
                }
            }
            Probe::At(rect) => {
                let moved = last != Some(rect);
                if moved {
                    last = Some(rect);
                    moved_at = Instant::now();
                }
                if moved || posted_at.elapsed() >= CHECK_EVERY {
                    post(app, registry, label, serial, Some(rect));
                    posted_at = Instant::now();
                }
            }
        }
        let pause = if last.is_some() && moved_at.elapsed() < MOVING_FOR {
            PROBE_MOVING_MS
        } else {
            PROBE_IDLE_MS
        };
        std::thread::sleep(Duration::from_millis(pause));
    }
}

/// The target of attachment `serial` is at `anchor` (`None`: hidden):
/// have the main thread bring the follower in line. Changes that come
/// in before it gets to that are folded into one step.
fn post(app: &AppHandle, registry: &Handle, label: &str, serial: u64, anchor: Option<Rect>) {
    {
        let mut entries = registry.entries.lock();
        let Some(e) = entries.get_mut(label).filter(|e| e.serial == serial) else {
            return;
        };
        e.anchor = anchor;
        if std::mem::replace(&mut e.pending, true) {
            return;
        }
    }
    let (app_main, registry, label) = (app.clone(), registry.clone(), label.to_string());
    let _ = app.run_on_main_thread(move || step(&app_main, &registry, &label, serial));
}

/// On the main thread: put the follower where its target is now.
fn step(app: &AppHandle, registry: &Handle, label: &str, serial: u64) {
    let win = app.get_webview_window(label);
    let mut entries = registry.entries.lock();
    let Some(e) = entries.get_mut(label).filter(|e| e.serial == serial) else {
        return;
    };
    e.pending = false;
    let Some(win) = win else {
        // Closed, however that came about.
        entries.remove(label);
        return;
    };
    let Some(anchor) = e.anchor else {
        // Also when it is hidden already by our account: a capture that
        // was under way hides the window and shows it again afterwards
        // where the window cannot be kept out of captures.
        if !std::mem::replace(&mut e.hidden, true) || win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        }
        return;
    };
    if std::mem::replace(&mut e.hidden, false) {
        follow::show_inactive(&win);
    }
    let Ok(actual) = lens::inner_rect(&win) else {
        return;
    };
    // On another display the header may measure differently.
    let scale = win.scale_factor().unwrap_or(1.0);
    e.follow.set_header(header_px(e.header_css, scale));
    let before = e.follow.dock();
    let target = e
        .follow
        .step(anchor, actual, follow::monitor_of(&win, anchor));
    let changed = e.follow.dock() != before;
    // Moving the window runs its move handlers, right here on Windows.
    drop(entries);
    if let Some(target) = target {
        follow::move_to(&win, actual, target);
    }
    if changed {
        emit(app, registry, label);
    }
}

/// A window `label` can be attached to.
#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    #[serde(flatten)]
    pub window: ForeignWindow,
    /// Share of `label`'s own area that lies on this window (0–1): the
    /// window right under it comes first.
    pub overlap: f32,
}

fn overlap(of: Rect, on: Rect) -> f32 {
    let w = (of.x + of.w as i32).min(on.x + on.w as i32) - of.x.max(on.x);
    let h = (of.y + of.h as i32).min(on.y + on.h as i32) - of.y.max(on.y);
    if w <= 0 || h <= 0 || of.w == 0 || of.h == 0 {
        return 0.0;
    }
    (w as f32 * h as f32) / (of.w as f32 * of.h as f32)
}

/// Candidates for `own`, the one it lies on most first (the window
/// list's own order otherwise: front to back where the system has one).
fn candidates(windows: Vec<ForeignWindow>, own: Option<Rect>) -> Vec<Candidate> {
    let mut list: Vec<Candidate> = windows
        .into_iter()
        .map(|window| Candidate {
            overlap: own.map_or(0.0, |own| overlap(own, window.rect)),
            window,
        })
        .collect();
    list.sort_by(|a, b| b.overlap.total_cmp(&a.overlap));
    list
}

fn header_css(app: &AppHandle, label: &str) -> f64 {
    let lenses = &app.state::<crate::app::AppState>().lenses;
    if lenses.insets(label).is_some() {
        f64::from(lens::HEADER_CSS)
    } else {
        0.0
    }
}

fn own_rect(win: &WebviewWindow) -> Option<Rect> {
    lens::inner_rect(win).ok()
}

/// Windows of other apps window `label` can be attached to.
#[tauri::command]
pub async fn list_attach_targets(label: String, app: AppHandle) -> Result<Vec<Candidate>, String> {
    let own = app.get_webview_window(&label).as_ref().and_then(own_rect);
    tokio::task::spawn_blocking(move || candidates(foreign_window::list(), own))
        .await
        .map_err(|e| e.to_string())
}

/// Attach window `label` to window `target` (an id from
/// [`list_attach_targets`]) at `dock`, or change the dock of the
/// attachment it has to that window.
#[tauri::command]
pub async fn attach_window(
    label: String,
    target: u64,
    dock: Dock,
    app: AppHandle,
    registry: State<'_, Handle>,
) -> Result<(), String> {
    let attached = registry.attachment(&label).target.map(|t| t.id);
    if attached == Some(target) {
        set_dock(&app, &registry, &label, dock);
        return Ok(());
    }
    let found = tokio::task::spawn_blocking(move || {
        foreign_window::list().into_iter().find(|w| w.id == target)
    })
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| {
        crate::tr!(
            "attach_target_gone|吸着先のウィンドウが存在しません",
            "attach_target_gone|The target window no longer exists"
        )
    })?;
    let header = header_css(&app, &label);
    attach(&app, &registry, &label, &found, dock, header).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn detach_window(
    label: String,
    app: AppHandle,
    registry: State<'_, Handle>,
) -> Result<(), String> {
    detach(&app, &registry, &label);
    Ok(())
}

#[tauri::command]
pub async fn window_attachment(
    label: String,
    registry: State<'_, Handle>,
) -> Result<Attachment, String> {
    Ok(registry.attachment(&label))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: u64, rect: Rect) -> ForeignWindow {
        ForeignWindow {
            id,
            pid: 1,
            title: format!("w{id}"),
            app: "app".into(),
            rect,
        }
    }

    const OWN: Rect = Rect {
        x: 100,
        y: 100,
        w: 400,
        h: 200,
    };

    #[test]
    fn the_window_under_the_follower_comes_first() {
        let far = Rect {
            x: 2000,
            y: 0,
            w: 800,
            h: 600,
        };
        let half = Rect {
            x: 300,
            y: 0,
            w: 800,
            h: 600,
        };
        let under = Rect {
            x: 0,
            y: 0,
            w: 1000,
            h: 800,
        };
        let list = candidates(
            vec![window(1, far), window(2, half), window(3, under)],
            Some(OWN),
        );
        let ids: Vec<u64> = list.iter().map(|c| c.window.id).collect();
        assert_eq!(ids, [3, 2, 1]);
        assert_eq!(list[0].overlap, 1.0);
        assert_eq!(list[1].overlap, 0.5);
        assert_eq!(list[2].overlap, 0.0);
    }

    #[test]
    fn windows_the_follower_is_not_on_keep_their_order() {
        let r = |x| Rect {
            x,
            y: 1000,
            w: 300,
            h: 300,
        };
        let list = candidates(
            vec![window(7, r(0)), window(5, r(900)), window(9, r(1500))],
            Some(OWN),
        );
        let ids: Vec<u64> = list.iter().map(|c| c.window.id).collect();
        assert_eq!(ids, [7, 5, 9]);
    }
}
