//! Layout sets: the window-translation windows that are open, saved as
//! one set and opened again later, by hand or by themselves when the
//! application they belong to starts.
//!
//! A set remembers, per window, where it was, what it translated into,
//! the view state of its renderer and the window of another application
//! it was attached to ([`crate::attach`]). Places of attached windows
//! are kept relative to that window, so a set fits again when the
//! application comes back somewhere else or at another size.
//!
//! One thread ([`watch`]) looks at the windows on the desktop every
//! [`POLL`] while there is anything to wait for: an application with a
//! set to restore that shows up gets its windows, and when it has
//! ended, the windows restored for it are closed again.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::app::AppState;
use crate::app_identity::AppIdentity;
use crate::attach;
use crate::follow::{self, Dock};
use crate::foreign_window::{self, ForeignWindow, Probe};
use crate::lens::{self, geometry::LensOuterRect as Rect};

/// Event to every window when the stored sets changed (no payload).
pub const EVENT: &str = "moonlens://layout-sets";

const FILE: &str = "layout-sets.json";
const FORMAT: u32 = 1;

/// How often the desktop is looked at while a set waits for its
/// application or windows restored for one are open.
const POLL: Duration = Duration::from_secs(2);
/// Polls in a row an application must show a window before its set is
/// restored: a launcher or splash window that flashes by is not it.
const SETTLED_POLLS: u8 = 2;

/// The window of another application a saved window was attached to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachLayout {
    pub app: AppIdentity,
    /// Title of the window, to find it again among the application's.
    pub title: String,
    pub dock: Dock,
    /// Where that window was when the set was saved.
    pub anchor: Rect,
}

/// One window of a set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowLayout {
    /// Client area, header included (physical pixels).
    pub rect: Rect,
    #[serde(default)]
    pub target_language: Option<String>,
    /// How its text is read (a caption that types itself out, or UI).
    #[serde(default)]
    pub read_mode: crate::region_watcher::ReadMode,
    /// View state of the renderer (see [`lens::LensRegistry::set_view`]).
    #[serde(default)]
    pub view: serde_json::Value,
    #[serde(default)]
    pub attach: Option<AttachLayout>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutSet {
    pub id: String,
    pub name: String,
    /// The application the set belongs to: the one most of its windows
    /// were attached to. `None`: restored by hand only.
    #[serde(default)]
    pub app: Option<AppIdentity>,
    /// Restore the set when `app` starts.
    #[serde(default)]
    pub auto_restore: bool,
    pub windows: Vec<WindowLayout>,
    /// Milliseconds since the Unix epoch.
    #[serde(default)]
    pub saved_at: u64,
}

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    sets: Vec<LayoutSet>,
}

/// `rect`, which was placed against `from`, placed the same way against
/// `to`: same share of its width and height, at the same relative spot.
pub fn rescale(rect: Rect, from: Rect, to: Rect) -> Rect {
    if from.w == 0 || from.h == 0 || (from.w, from.h) == (to.w, to.h) {
        return Rect {
            x: to.x + (rect.x - from.x),
            y: to.y + (rect.y - from.y),
            ..rect
        };
    }
    let (sx, sy) = (
        f64::from(to.w) / f64::from(from.w),
        f64::from(to.h) / f64::from(from.h),
    );
    Rect {
        x: to.x + (f64::from(rect.x - from.x) * sx).round() as i32,
        y: to.y + (f64::from(rect.y - from.y) * sy).round() as i32,
        w: ((f64::from(rect.w) * sx).round() as u32).max(1),
        h: ((f64::from(rect.h) * sy).round() as u32).max(1),
    }
}

/// Width of a window's header row that must lie on a monitor for the
/// window to be reached with the mouse.
const REACH: i32 = 64;

/// `rect` where it can be seen and reached: as it is when its header
/// row lies on one of `monitors`, else moved onto the nearest one.
pub fn keep_visible(rect: Rect, monitors: &[Rect]) -> Rect {
    let overlap = |m: &Rect| {
        let w = (rect.x + rect.w as i32).min(m.x + m.w as i32) - rect.x.max(m.x);
        let on_row = rect.y >= m.y && rect.y < m.y + m.h as i32;
        if on_row {
            w
        } else {
            0
        }
    };
    if monitors.is_empty()
        || monitors
            .iter()
            .any(|m| overlap(m) >= REACH.min(rect.w as i32))
    {
        return rect;
    }
    let (cx, cy) = (rect.x + rect.w as i32 / 2, rect.y + rect.h as i32 / 2);
    let distance = |m: &&Rect| {
        let dx = i64::from((m.x - cx).max(cx - (m.x + m.w as i32 - 1)).max(0));
        let dy = i64::from((m.y - cy).max(cy - (m.y + m.h as i32 - 1)).max(0));
        dx * dx + dy * dy
    };
    let Some(m) = monitors.iter().min_by_key(distance) else {
        return rect;
    };
    let (w, h) = (rect.w.min(m.w), rect.h.min(m.h));
    Rect {
        x: rect.x.min(m.x + m.w as i32 - w as i32).max(m.x),
        y: rect.y.min(m.y + m.h as i32 - h as i32).max(m.y),
        w,
        h,
    }
}

/// A window on the desktop with the application it belongs to.
#[derive(Debug, Clone)]
pub struct AppWindow {
    pub window: ForeignWindow,
    pub app: AppIdentity,
}

/// The window of application `key` a saved window goes back onto: the
/// one with the saved title, else the largest.
pub fn pick_target<'a>(windows: &'a [AppWindow], key: &str, title: &str) -> Option<&'a AppWindow> {
    let of_app = || windows.iter().filter(|w| w.app.key == key);
    of_app()
        .find(|w| !title.is_empty() && w.window.title == title)
        .or_else(|| {
            of_app().max_by_key(|w| u64::from(w.window.rect.w) * u64::from(w.window.rect.h))
        })
}

/// The application a set of windows belongs to: the one most of them
/// are attached to (the earliest of those on a tie).
pub fn owner(windows: &[WindowLayout]) -> Option<AppIdentity> {
    let mut counts: Vec<(&AppIdentity, usize)> = Vec::new();
    for app in windows
        .iter()
        .filter_map(|w| w.attach.as_ref().map(|a| &a.app))
    {
        match counts.iter_mut().find(|(a, _)| a.key == app.key) {
            Some((_, n)) => *n += 1,
            None => counts.push((app, 1)),
        }
    }
    let most = counts.iter().map(|&(_, n)| n).max()?;
    counts
        .into_iter()
        .find(|&(_, n)| n == most)
        .map(|(app, _)| app.clone())
}

/// Name for a new set: the application's, numbered when taken.
pub fn default_name(app: Option<&AppIdentity>, taken: &[String]) -> String {
    let base = app.map_or("配置セット", |a| a.name.as_str());
    if app.is_some() && !taken.iter().any(|t| t == base) {
        return base.to_string();
    }
    (if app.is_some() { 2 } else { 1 }..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken.contains(name))
        .unwrap_or_else(|| base.to_string())
}

/// Which applications came and went, from one look at the desktop to
/// the next.
#[derive(Default)]
pub struct Presence {
    apps: HashMap<String, Seen>,
}

enum Seen {
    /// Shows a window, for this many polls in a row.
    Showing(u8),
    /// Counted as running; these processes showed its windows.
    Running(HashSet<u32>),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Applications that started (and kept a window up long enough).
    pub arrived: Vec<String>,
    /// Applications none of whose processes is left.
    pub left: Vec<String>,
}

impl Presence {
    /// `windows`: (application key, process) of every window there is
    /// now. `alive`: whether a process still runs as that application.
    ///
    /// An application has left when its processes ended, not when its
    /// windows are gone: a minimized application shows none.
    pub fn update(
        &mut self,
        windows: &[(String, u32)],
        alive: impl Fn(&str, u32) -> bool,
    ) -> Changes {
        let mut changes = Changes::default();
        let mut now: HashMap<&str, HashSet<u32>> = HashMap::new();
        for (key, pid) in windows {
            now.entry(key).or_default().insert(*pid);
        }
        self.apps.retain(|key, seen| match seen {
            Seen::Showing(_) => now.contains_key(key.as_str()),
            Seen::Running(pids) => {
                if let Some(more) = now.get(key.as_str()) {
                    pids.extend(more);
                }
                pids.retain(|&pid| alive(key, pid));
                if pids.is_empty() {
                    changes.left.push(key.clone());
                }
                !pids.is_empty()
            }
        });
        for (key, pids) in now {
            let seen = self.apps.entry(key.to_string()).or_insert(Seen::Showing(0));
            if let Seen::Showing(polls) = seen {
                *polls += 1;
                if *polls >= SETTLED_POLLS {
                    *seen = Seen::Running(pids);
                    changes.arrived.push(key.to_string());
                }
            }
        }
        changes.arrived.sort();
        changes.left.sort();
        changes
    }
}

/// The saved sets, kept in a file next to the history.
pub struct LayoutStore {
    path: PathBuf,
    sets: Mutex<Vec<LayoutSet>>,
}

impl LayoutStore {
    /// The sets in `dir`. A file that cannot be read is set aside
    /// (`.broken`) and the store starts empty.
    pub fn open(dir: &Path) -> Self {
        let path = dir.join(FILE);
        let sets = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<File>(&text) {
                Ok(file) => file.sets,
                Err(e) => {
                    tracing::warn!("layout sets unreadable, starting empty: {e}");
                    let _ = std::fs::rename(&path, path.with_extension("json.broken"));
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        };
        Self {
            path,
            sets: Mutex::new(sets),
        }
    }

    pub fn list(&self) -> Vec<LayoutSet> {
        self.sets.lock().clone()
    }

    pub fn get(&self, id: &str) -> Option<LayoutSet> {
        self.sets.lock().iter().find(|s| s.id == id).cloned()
    }

    /// Add `set`, or replace the one with its id (at `index` when given
    /// and new: a removal undone). One set per application restores by
    /// itself: turning it on for `set` turns it off for the others.
    pub fn put(&self, set: LayoutSet, index: Option<usize>) -> Result<(), String> {
        let mut sets = self.sets.lock();
        if set.auto_restore {
            if let Some(key) = set.app.as_ref().map(|a| a.key.as_str()) {
                for other in sets.iter_mut().filter(|s| s.id != set.id) {
                    if other.app.as_ref().is_some_and(|a| a.key == key) {
                        other.auto_restore = false;
                    }
                }
            }
        }
        match sets.iter_mut().find(|s| s.id == set.id) {
            Some(slot) => *slot = set,
            None => {
                let at = index.unwrap_or(sets.len()).min(sets.len());
                sets.insert(at, set);
            }
        }
        self.write(&sets)
    }

    /// Remove set `id`; returns it with the place it had.
    pub fn remove(&self, id: &str) -> Result<Option<(usize, LayoutSet)>, String> {
        let mut sets = self.sets.lock();
        let Some(at) = sets.iter().position(|s| s.id == id) else {
            return Ok(None);
        };
        let set = sets.remove(at);
        self.write(&sets)?;
        Ok(Some((at, set)))
    }

    /// The set that restores by itself for application `key`.
    pub fn auto_for(&self, key: &str) -> Option<LayoutSet> {
        self.sets
            .lock()
            .iter()
            .find(|s| s.auto_restore && s.app.as_ref().is_some_and(|a| a.key == key))
            .cloned()
    }

    fn waits(&self) -> bool {
        self.sets
            .lock()
            .iter()
            .any(|s| s.auto_restore && s.app.is_some())
    }

    fn write(&self, sets: &[LayoutSet]) -> Result<(), String> {
        let file = File {
            version: FORMAT,
            sets: sets.to_vec(),
        };
        let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
        crate::settings::store::write_atomic(&self.path, &text)
            .map_err(|e| format!("layout_save_failed|配置セットの保存に失敗：{e}"))
    }
}

/// A window opened from a set.
struct Restored {
    label: String,
    layout: WindowLayout,
    /// The window it was attached to when restored.
    target: Option<u64>,
    /// Seen off that window while the window was still there, at the
    /// last poll.
    loose: bool,
}

/// What becomes of the attachment of a window opened from a set.
#[derive(Debug, PartialEq, Eq)]
enum Tie {
    /// As it is.
    Keep,
    /// The user took the window off its target, or onto another one:
    /// theirs from now on, it is not attached again.
    Release,
    /// Its target is gone, or was not there yet: onto the window the
    /// application shows now.
    Reattach,
}

/// `target`: the window it was attached to when restored; `wants`: its
/// layout names an application window; `attached`: what it is attached
/// to now; `present`: the windows on the desktop.
fn tie(target: Option<u64>, wants: bool, attached: Option<u64>, present: &HashSet<u64>) -> Tie {
    match (attached, target) {
        (Some(now), Some(then)) if now == then => Tie::Keep,
        (Some(_), _) => Tie::Release,
        (None, Some(then)) if present.contains(&then) => Tie::Release,
        (None, Some(_)) => Tie::Reattach,
        (None, None) if wants => Tie::Reattach,
        (None, None) => Tie::Keep,
    }
}

/// The windows a set has open.
struct Instance {
    windows: Vec<Restored>,
    /// Opened because the application started: closed when it ends.
    auto: bool,
}

/// The saved sets and what is open from them.
pub struct LayoutSets {
    store: LayoutStore,
    /// By set id.
    active: Mutex<HashMap<String, Instance>>,
}

pub type Handle = Arc<LayoutSets>;

impl LayoutSets {
    pub fn open(dir: &Path) -> Handle {
        Arc::new(Self {
            store: LayoutStore::open(dir),
            active: Mutex::default(),
        })
    }

    /// Drop windows that were closed, and sets left without any.
    fn prune(&self, open: &HashSet<String>) {
        let mut active = self.active.lock();
        for instance in active.values_mut() {
            instance.windows.retain(|w| open.contains(&w.label));
        }
        active.retain(|_, instance| !instance.windows.is_empty());
    }

    fn is_active(&self, id: &str) -> bool {
        self.active.lock().contains_key(id)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn next_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("set-{:x}-{seq:x}", now_ms())
}

fn changed(app: &AppHandle) {
    let _ = app.emit(EVENT, ());
}

/// The windows of other applications, each with its application.
/// `known` keeps the answer per process between calls.
fn app_windows(known: &mut HashMap<u32, Option<AppIdentity>>) -> Vec<AppWindow> {
    let windows = foreign_window::list();
    let pids: HashSet<u32> = windows.iter().map(|w| w.pid).collect();
    known.retain(|pid, _| pids.contains(pid));
    windows
        .into_iter()
        .filter_map(|window| {
            let app = known
                .entry(window.pid)
                .or_insert_with(|| attach::identity_of(&window))
                .clone()?;
            Some(AppWindow { window, app })
        })
        .collect()
}

fn monitors(app: &AppHandle) -> Vec<Rect> {
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| Rect {
            x: m.position().x,
            y: m.position().y,
            w: m.size().width,
            h: m.size().height,
        })
        .collect()
}

/// The window-translation windows that are open now, as a set saves
/// them.
fn snapshot(app: &AppHandle) -> Vec<Restored> {
    let state = app.state::<AppState>();
    let attachments = app.state::<attach::Handle>();
    state
        .lenses
        .list()
        .into_iter()
        .filter_map(|lens| {
            let win = app.get_webview_window(&lens.id)?;
            let rect = lens::inner_rect(&win).ok()?;
            let attachment = attachments.attachment(&lens.id);
            let target = attachment.target.as_ref().map(|t| t.id);
            let attach = attachment
                .target
                .zip(attachment.dock)
                .and_then(|(t, dock)| {
                    // A target that is hidden now has no place to keep the
                    // window's place against.
                    let Probe::At(anchor) = foreign_window::probe(t.id) else {
                        return None;
                    };
                    Some(AttachLayout {
                        app: t.identity?,
                        title: t.title,
                        dock,
                        anchor,
                    })
                });
            Some(Restored {
                label: lens.id,
                target: attach.as_ref().and(target),
                loose: false,
                layout: WindowLayout {
                    rect,
                    target_language: lens.target_language,
                    read_mode: lens.read_mode,
                    view: lens.view,
                    attach,
                },
            })
        })
        .collect()
}

/// Where window `layout` goes now and what it is attached to.
fn placement<'a>(
    layout: &WindowLayout,
    windows: &'a [AppWindow],
    monitors: &[Rect],
) -> (Rect, Option<&'a AppWindow>) {
    let target = layout
        .attach
        .as_ref()
        .and_then(|a| pick_target(windows, &a.app.key, &a.title));
    match (&layout.attach, target) {
        (Some(a), Some(t)) => (rescale(layout.rect, a.anchor, t.window.rect), Some(t)),
        _ => (keep_visible(layout.rect, monitors), None),
    }
}

fn attach_to(app: &AppHandle, label: &str, layout: &WindowLayout, target: &AppWindow) -> bool {
    let Some(dock) = layout.attach.as_ref().map(|a| a.dock) else {
        return false;
    };
    let attachments = app.state::<attach::Handle>();
    let header = f64::from(lens::HEADER_CSS);
    match attach::attach(app, &attachments, label, &target.window, dock, header) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("layout set: attaching {label}: {e:#}");
            false
        }
    }
}

/// Open the windows of `set` (main thread). The windows it has open
/// already are closed first, and those of the other sets of its
/// application: one set of an application takes the place of another.
/// Returns how many were opened.
fn restore(
    app: &AppHandle,
    sets: &Handle,
    set: &LayoutSet,
    windows: &[AppWindow],
    auto: bool,
) -> usize {
    let key = set.app.as_ref().map(|a| a.key.as_str());
    let replaced: Vec<String> = sets
        .active
        .lock()
        .keys()
        .filter(|id| {
            **id == set.id
                || key.is_some_and(|key| {
                    sets.store
                        .get(id)
                        .and_then(|other| other.app)
                        .is_some_and(|a| a.key == key)
                })
        })
        .cloned()
        .collect();
    for id in replaced {
        close_instance(app, sets, &id);
    }
    let state = app.state::<AppState>();
    let monitors = monitors(app);
    let mut opened = Vec::new();
    for layout in &set.windows {
        let (rect, target) = placement(layout, windows, &monitors);
        let label = match lens::create_at(
            app,
            &state.lenses,
            rect,
            layout.target_language.clone(),
            layout.read_mode,
            layout.view.clone(),
        ) {
            Ok(label) => label,
            Err(e) => {
                tracing::warn!("layout set {}: opening a window: {e:#}", set.name);
                continue;
            }
        };
        let attached = target.filter(|t| attach_to(app, &label, layout, t));
        opened.push(Restored {
            label,
            layout: layout.clone(),
            target: attached.map(|t| t.window.id),
            loose: false,
        });
    }
    let count = opened.len();
    if count > 0 {
        sets.active.lock().insert(
            set.id.clone(),
            Instance {
                windows: opened,
                auto,
            },
        );
    }
    tracing::info!(
        "layout set {:?}: {count} of {} windows restored (auto: {auto})",
        set.name,
        set.windows.len()
    );
    changed(app);
    count
}

/// Close the windows set `id` has open (main thread).
fn close_instance(app: &AppHandle, sets: &Handle, id: &str) {
    let Some(instance) = sets.active.lock().remove(id) else {
        return;
    };
    let state = app.state::<AppState>();
    for w in instance.windows {
        let _ = lens::close(app, &state.lenses, &w.label);
    }
    changed(app);
}

/// Windows of open sets whose target window is gone while their
/// application shows another one (a launcher gave way to the real
/// window): onto that one (main thread).
fn reattach(app: &AppHandle, sets: &Handle, windows: &[AppWindow]) {
    let attachments = app.state::<attach::Handle>();
    let present: HashSet<u64> = windows.iter().map(|w| w.window.id).collect();
    let monitors = monitors(app);
    let mut active = sets.active.lock();
    for w in active.values_mut().flat_map(|i| i.windows.iter_mut()) {
        let attached = attachments.attachment(&w.label).target.map(|t| t.id);
        match tie(w.target, w.layout.attach.is_some(), attached, &present) {
            Tie::Keep => {
                w.loose = false;
                continue;
            }
            Tie::Release if !w.loose => {
                // Once more: a target that just closed may still be in
                // this poll's window list.
                w.loose = true;
                continue;
            }
            Tie::Release => {
                w.target = None;
                w.layout.attach = None;
                continue;
            }
            Tie::Reattach => w.loose = false,
        }
        let (rect, Some(target)) = placement(&w.layout, windows, &monitors) else {
            continue;
        };
        let Some(win) = app.get_webview_window(&w.label) else {
            continue;
        };
        if let Ok(actual) = lens::inner_rect(&win) {
            follow::move_to(&win, actual, rect);
        }
        if attach_to(app, &w.label, &w.layout, target) {
            w.target = Some(target.window.id);
        }
    }
}

/// Watch the desktop for the applications of the saved sets, for as
/// long as the app runs.
pub fn watch(app: &AppHandle, sets: &Handle) {
    let (app, sets) = (app.clone(), sets.clone());
    let spawned = std::thread::Builder::new()
        .name("layout-sets".into())
        .spawn(move || {
            let mut presence = Presence::default();
            let mut known = HashMap::new();
            loop {
                std::thread::sleep(POLL);
                poll(&app, &sets, &mut presence, &mut known);
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("layout sets: no watcher thread: {e}");
    }
}

fn poll(
    app: &AppHandle,
    sets: &Handle,
    presence: &mut Presence,
    known: &mut HashMap<u32, Option<AppIdentity>>,
) {
    let open: HashSet<String> = app
        .state::<AppState>()
        .lenses
        .list()
        .into_iter()
        .map(|l| l.id)
        .collect();
    sets.prune(&open);
    if !sets.store.waits() && sets.active.lock().is_empty() {
        // Nothing to wait for: the desktop is left alone.
        *presence = Presence::default();
        return;
    }
    let windows = app_windows(known);
    let seen: Vec<(String, u32)> = windows
        .iter()
        .map(|w| (w.app.key.clone(), w.window.pid))
        .collect();
    let changes = presence.update(&seen, |key, pid| {
        AppIdentity::of_pid(pid).is_some_and(|a| a.key == key)
    });
    let arrived: Vec<LayoutSet> = changes
        .arrived
        .iter()
        .filter_map(|key| sets.store.auto_for(key))
        .filter(|set| !sets.is_active(&set.id))
        .collect();
    let (app_main, sets) = (app.clone(), sets.clone());
    let _ = app.run_on_main_thread(move || {
        for set in &arrived {
            restore(&app_main, &sets, set, &windows, true);
        }
        let ended: Vec<String> = sets
            .active
            .lock()
            .iter()
            .filter(|(_, instance)| instance.auto)
            .filter_map(|(id, _)| {
                let key = sets.store.get(id)?.app?.key;
                changes.left.contains(&key).then(|| id.clone())
            })
            .collect();
        for id in ended {
            close_instance(&app_main, &sets, &id);
        }
        reattach(&app_main, &sets, &windows);
    });
}

/// A saved set as the screens show it.
#[derive(Debug, Clone, Serialize)]
pub struct LayoutSetInfo {
    #[serde(flatten)]
    pub set: LayoutSet,
    /// Windows of the set are open.
    pub active: bool,
}

fn info(sets: &LayoutSets, set: LayoutSet) -> LayoutSetInfo {
    LayoutSetInfo {
        active: sets.is_active(&set.id),
        set,
    }
}

#[tauri::command]
pub async fn list_layout_sets(sets: State<'_, Handle>) -> Result<Vec<LayoutSetInfo>, String> {
    Ok(sets
        .store
        .list()
        .into_iter()
        .map(|set| info(&sets, set))
        .collect())
}

/// Save the open window-translation windows: as a new set, or over set
/// `id` (which keeps its name and whether it restores by itself).
#[tauri::command]
pub async fn save_layout_set(
    id: Option<String>,
    app: AppHandle,
    sets: State<'_, Handle>,
) -> Result<LayoutSetInfo, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let app_main = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(snapshot(&app_main));
    })
    .map_err(|e| e.to_string())?;
    let open = rx.await.map_err(|e| e.to_string())?;
    if open.is_empty() {
        return Err("layout_no_windows|保存対象のウィンドウ翻訳が未表示".into());
    }
    let windows: Vec<WindowLayout> = open.iter().map(|w| w.layout.clone()).collect();
    let owner = owner(&windows);
    let set = match id.and_then(|id| sets.store.get(&id)) {
        Some(old) => LayoutSet {
            auto_restore: old.auto_restore && owner.is_some(),
            app: owner,
            windows,
            saved_at: now_ms(),
            ..old
        },
        None => {
            let taken: Vec<String> = sets.store.list().into_iter().map(|s| s.name).collect();
            // The first set of an application restores by itself.
            let first = owner
                .as_ref()
                .is_some_and(|a| sets.store.auto_for(&a.key).is_none());
            LayoutSet {
                id: next_id(),
                name: default_name(owner.as_ref(), &taken),
                auto_restore: first,
                app: owner,
                windows,
                saved_at: now_ms(),
            }
        }
    };
    sets.store.put(set.clone(), None)?;
    // The open windows are the set's from now on: they are not opened
    // a second time for it, and end with its application like restored
    // ones when the set restores by itself.
    {
        let mut active = sets.active.lock();
        let labels: HashSet<&str> = open.iter().map(|w| w.label.as_str()).collect();
        for instance in active.values_mut() {
            instance
                .windows
                .retain(|w| !labels.contains(w.label.as_str()));
        }
        active.retain(|_, instance| !instance.windows.is_empty());
        active.insert(
            set.id.clone(),
            Instance {
                windows: open,
                auto: set.auto_restore,
            },
        );
    }
    changed(&app);
    Ok(info(&sets, set))
}

/// Rename set `id` and / or switch whether it restores by itself.
#[tauri::command]
pub async fn update_layout_set(
    id: String,
    name: Option<String>,
    auto_restore: Option<bool>,
    app: AppHandle,
    sets: State<'_, Handle>,
) -> Result<(), String> {
    let mut set = sets
        .store
        .get(&id)
        .ok_or("layout_missing|配置セットが存在しません")?;
    if let Some(name) = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
        set.name = name;
    }
    if let Some(on) = auto_restore {
        set.auto_restore = on && set.app.is_some();
        // Only the switch changes whether the open windows close with
        // their application: a rename leaves windows opened by hand as
        // they are.
        if let Some(instance) = sets.active.lock().get_mut(&id) {
            instance.auto = set.auto_restore;
        }
    }
    sets.store.put(set, None)?;
    changed(&app);
    Ok(())
}

/// Delete set `id`. Its open windows stay. Returns what
/// [`put_layout_set`] takes to undo it.
#[tauri::command]
pub async fn delete_layout_set(
    id: String,
    app: AppHandle,
    sets: State<'_, Handle>,
) -> Result<Option<(usize, LayoutSet)>, String> {
    let removed = sets.store.remove(&id)?;
    sets.active.lock().remove(&id);
    changed(&app);
    Ok(removed)
}

/// Put a deleted set back at `index`.
#[tauri::command]
pub async fn put_layout_set(
    set: LayoutSet,
    index: usize,
    app: AppHandle,
    sets: State<'_, Handle>,
) -> Result<(), String> {
    sets.store.put(set, Some(index))?;
    changed(&app);
    Ok(())
}

/// Open the windows of set `id` now. Returns how many were opened.
#[tauri::command]
pub async fn restore_layout_set(
    id: String,
    app: AppHandle,
    sets: State<'_, Handle>,
) -> Result<usize, String> {
    let set = sets
        .store
        .get(&id)
        .ok_or("layout_missing|配置セットが存在しません")?;
    let windows = tokio::task::spawn_blocking(|| app_windows(&mut HashMap::new()))
        .await
        .map_err(|e| e.to_string())?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let (app_main, sets_main) = (app.clone(), sets.inner().clone());
    app.run_on_main_thread(move || {
        let _ = tx.send(restore(&app_main, &sets_main, &set, &windows, false));
    })
    .map_err(|e| e.to_string())?;
    match rx.await.map_err(|e| e.to_string())? {
        0 => Err("layout_restore_failed|ウィンドウの復元に失敗".into()),
        opened => Ok(opened),
    }
}

/// The renderer of window-translation window `id` reports its view
/// state, for a set to save.
#[tauri::command]
pub async fn set_lens_view(
    id: String,
    view: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.lenses.set_view(&id, view);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    fn app(name: &str) -> AppIdentity {
        AppIdentity::from_executable(name).unwrap()
    }

    fn attached(name: &str) -> WindowLayout {
        WindowLayout {
            rect: rect(0, 0, 100, 100),
            target_language: None,
            read_mode: Default::default(),
            view: serde_json::Value::Null,
            attach: Some(AttachLayout {
                app: app(name),
                title: String::new(),
                dock: Dock::Free,
                anchor: rect(0, 0, 800, 600),
            }),
        }
    }

    fn loose() -> WindowLayout {
        WindowLayout {
            attach: None,
            ..attached("x")
        }
    }

    fn set(id: &str, name: &str, auto: bool) -> LayoutSet {
        LayoutSet {
            id: id.into(),
            name: name.into(),
            app: Some(app(name)),
            auto_restore: auto,
            windows: vec![attached(name)],
            saved_at: 1,
        }
    }

    fn window(id: u64, name: &str, title: &str, r: Rect) -> AppWindow {
        AppWindow {
            window: ForeignWindow {
                id,
                pid: id as u32,
                title: title.into(),
                app: name.into(),
                rect: r,
            },
            app: app(name),
        }
    }

    #[test]
    fn a_window_keeps_its_place_on_a_target_that_only_moved() {
        let saved = rect(140, 520, 600, 120);
        let from = rect(100, 100, 1280, 720);
        let to = rect(400, 50, 1280, 720);
        assert_eq!(rescale(saved, from, to), rect(440, 470, 600, 120));
    }

    #[test]
    fn a_window_scales_with_a_target_of_another_size() {
        let saved = rect(100, 500, 640, 180);
        let from = rect(0, 0, 1280, 720);
        let to = rect(0, 0, 1920, 1080);
        assert_eq!(rescale(saved, from, to), rect(150, 750, 960, 270));
        // And back: nothing drifts over a round trip.
        assert_eq!(rescale(rescale(saved, from, to), to, from), saved);
    }

    #[test]
    fn a_window_beside_its_target_stays_beside_it() {
        let saved = rect(-300, 0, 280, 720);
        let from = rect(0, 0, 1280, 720);
        let to = rect(1000, 200, 640, 360);
        assert_eq!(rescale(saved, from, to), rect(850, 200, 140, 360));
    }

    #[test]
    fn a_window_on_a_monitor_stays_where_it_was() {
        let monitors = [rect(0, 0, 1920, 1080), rect(1920, 0, 1920, 1080)];
        for r in [
            rect(100, 100, 460, 320),
            rect(2500, 700, 460, 320),
            // Half off the right edge: its header can still be reached.
            rect(3700, 300, 460, 320),
        ] {
            assert_eq!(keep_visible(r, &monitors), r);
        }
    }

    #[test]
    fn a_window_off_every_monitor_comes_back_onto_the_nearest() {
        let monitors = [rect(0, 0, 1920, 1080)];
        // Saved on a second monitor that is no longer connected.
        assert_eq!(
            keep_visible(rect(2500, 200, 460, 320), &monitors),
            rect(1460, 200, 460, 320)
        );
        // Header above the top edge.
        assert_eq!(
            keep_visible(rect(100, -400, 460, 320), &monitors),
            rect(100, 0, 460, 320)
        );
        // Larger than the monitor.
        assert_eq!(
            keep_visible(rect(5000, 5000, 4000, 3000), &monitors),
            rect(0, 0, 1920, 1080)
        );
    }

    #[test]
    fn the_target_is_the_window_with_the_saved_title_else_the_largest() {
        let windows = [
            window(1, "game.exe", "Launcher", rect(0, 0, 400, 300)),
            window(2, "game.exe", "Game", rect(0, 0, 1280, 720)),
            window(3, "other.exe", "Game", rect(0, 0, 3000, 2000)),
            window(4, "game.exe", "Console", rect(0, 0, 1600, 900)),
        ];
        let id = |t: Option<&AppWindow>| t.map(|w| w.window.id);
        assert_eq!(id(pick_target(&windows, "game", "Game")), Some(2));
        assert_eq!(id(pick_target(&windows, "game", "Renamed")), Some(4));
        assert_eq!(id(pick_target(&windows, "game", "")), Some(4));
        assert_eq!(id(pick_target(&windows, "absent", "Game")), None);
    }

    #[test]
    fn a_set_belongs_to_the_application_most_windows_are_attached_to() {
        assert_eq!(owner(&[loose(), loose()]), None);
        let windows = [
            attached("chat.exe"),
            loose(),
            attached("Game.exe"),
            attached("game"),
        ];
        assert_eq!(owner(&windows).unwrap().key, "game");
        // A tie: the application of the earlier window.
        let tie = [attached("chat.exe"), attached("game.exe")];
        assert_eq!(owner(&tie).unwrap().key, "chat");
    }

    #[test]
    fn new_sets_are_named_after_their_application() {
        let game = app("Game.exe");
        let taken = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        assert_eq!(default_name(Some(&game), &[]), "Game");
        assert_eq!(default_name(Some(&game), &taken(&["Game"])), "Game 2");
        assert_eq!(
            default_name(Some(&game), &taken(&["Game", "Game 2"])),
            "Game 3"
        );
        assert_eq!(default_name(None, &[]), "配置セット 1");
        assert_eq!(
            default_name(None, &taken(&["配置セット 1"])),
            "配置セット 2"
        );
    }

    #[test]
    fn an_application_arrives_once_its_window_stayed_and_leaves_with_its_process() {
        let mut presence = Presence::default();
        let game = [("game".to_string(), 7u32)];
        let alive = |_: &str, _: u32| true;
        assert_eq!(presence.update(&game, alive), Changes::default());
        assert_eq!(presence.update(&game, alive).arrived, ["game"]);
        // Running: no second arrival, with or without a window.
        assert_eq!(presence.update(&game, alive), Changes::default());
        assert_eq!(presence.update(&[], alive), Changes::default());
        assert_eq!(presence.update(&game, alive), Changes::default());
        // Its process ended.
        assert_eq!(presence.update(&[], |_, _| false).left, ["game"]);
        // Started again: it arrives again.
        assert_eq!(presence.update(&game, alive), Changes::default());
        assert_eq!(presence.update(&game, alive).arrived, ["game"]);
    }

    #[test]
    fn a_window_that_flashes_by_is_no_arrival() {
        let mut presence = Presence::default();
        let splash = [("launcher".to_string(), 3u32)];
        let alive = |_: &str, _: u32| true;
        assert_eq!(presence.update(&splash, alive), Changes::default());
        assert_eq!(presence.update(&[], alive), Changes::default());
        assert_eq!(presence.update(&splash, alive), Changes::default());
        assert_eq!(presence.update(&[], alive), Changes::default());
    }

    #[test]
    fn an_application_runs_while_any_of_its_processes_does() {
        let mut presence = Presence::default();
        let both = [("game".to_string(), 1u32), ("game".to_string(), 2u32)];
        presence.update(&both, |_, _| true);
        assert_eq!(presence.update(&both, |_, _| true).arrived, ["game"]);
        assert_eq!(presence.update(&[], |_, pid| pid == 2), Changes::default());
        assert_eq!(presence.update(&[], |_, _| false).left, ["game"]);
    }

    #[test]
    fn sets_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let store = LayoutStore::open(dir.path());
        assert!(store.list().is_empty());
        store.put(set("a", "Game.exe", true), None).unwrap();
        store.put(set("b", "Chat.exe", false), None).unwrap();
        let again = LayoutStore::open(dir.path());
        assert_eq!(again.list(), store.list());
        assert_eq!(again.auto_for("game").unwrap().id, "a");
        assert_eq!(again.auto_for("chat"), None);
    }

    #[test]
    fn one_set_per_application_restores_by_itself() {
        let dir = tempfile::tempdir().unwrap();
        let store = LayoutStore::open(dir.path());
        store.put(set("a", "Game.exe", true), None).unwrap();
        store.put(set("b", "game", true), None).unwrap();
        store.put(set("c", "Chat.exe", true), None).unwrap();
        assert_eq!(store.auto_for("game").unwrap().id, "b");
        assert!(!store.get("a").unwrap().auto_restore);
        assert!(store.get("c").unwrap().auto_restore);
    }

    #[test]
    fn a_removed_set_goes_back_where_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let store = LayoutStore::open(dir.path());
        for id in ["a", "b", "c"] {
            store.put(set(id, id, false), None).unwrap();
        }
        let (at, removed) = store.remove("b").unwrap().unwrap();
        assert_eq!(store.remove("b").unwrap(), None);
        store.put(removed, Some(at)).unwrap();
        let ids: Vec<String> = store.list().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, ["a", "b", "c"]);
    }

    #[test]
    fn a_broken_file_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), "{ not json").unwrap();
        let store = LayoutStore::open(dir.path());
        assert!(store.list().is_empty());
        assert!(dir.path().join("layout-sets.json.broken").exists());
        store.put(set("a", "game", false), None).unwrap();
        assert_eq!(LayoutStore::open(dir.path()).list().len(), 1);
    }

    #[test]
    fn a_file_from_an_older_build_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"{"version":1,"sets":[{"id":"a","name":"Game","windows":[
            {"rect":{"x":1,"y":2,"w":3,"h":4}}]}]}"#;
        std::fs::write(dir.path().join(FILE), old).unwrap();
        let sets = LayoutStore::open(dir.path()).list();
        assert_eq!(sets[0].windows[0].rect, rect(1, 2, 3, 4));
        assert!(!sets[0].auto_restore);
    }

    #[test]
    fn a_window_goes_onto_its_application_when_that_runs() {
        let monitors = [rect(0, 0, 1920, 1080)];
        let layout = WindowLayout {
            rect: rect(100, 500, 640, 180),
            ..attached("game.exe")
        };
        let layout = WindowLayout {
            attach: layout.attach.map(|a| AttachLayout {
                anchor: rect(0, 0, 1280, 720),
                ..a
            }),
            ..layout
        };
        let running = [window(5, "game.exe", "Game", rect(320, 180, 1280, 720))];
        let (at, target) = placement(&layout, &running, &monitors);
        assert_eq!(at, rect(420, 680, 640, 180));
        assert_eq!(target.map(|t| t.window.id), Some(5));
        // Not running: where it was saved, attached to nothing.
        let (at, target) = placement(&layout, &[], &monitors);
        assert_eq!(at, layout.rect);
        assert!(target.is_none());
    }
    #[test]
    fn a_window_saved_without_a_read_mode_reads_subtitles() {
        let saved = r#"{"rect":{"x":0,"y":0,"w":10,"h":10}}"#;
        let window: WindowLayout = serde_json::from_str(saved).unwrap();
        assert_eq!(window.read_mode, crate::region_watcher::ReadMode::Subtitle);
        let ui = r#"{"rect":{"x":0,"y":0,"w":10,"h":10},"read_mode":"ui"}"#;
        let window: WindowLayout = serde_json::from_str(ui).unwrap();
        assert_eq!(window.read_mode, crate::region_watcher::ReadMode::Ui);
    }
    #[test]
    fn a_window_the_user_took_off_its_target_is_not_attached_again() {
        let present: HashSet<u64> = [1, 2].into();
        // Still on the window it was restored onto.
        assert_eq!(tie(Some(1), true, Some(1), &present), Tie::Keep);
        // Detached while that window is still there: the user's doing.
        assert_eq!(tie(Some(1), true, None, &present), Tie::Release);
        // Attached to another window by the user.
        assert_eq!(tie(Some(1), true, Some(2), &present), Tie::Release);
        assert_eq!(tie(None, true, Some(2), &present), Tie::Release);
    }

    #[test]
    fn a_window_whose_target_went_away_goes_onto_the_next_one() {
        let present: HashSet<u64> = [2].into();
        // The launcher window closed.
        assert_eq!(tie(Some(1), true, None, &present), Tie::Reattach);
        // Restored before its application showed a window.
        assert_eq!(tie(None, true, None, &present), Tie::Reattach);
        // Never attached to anything.
        assert_eq!(tie(None, false, None, &present), Tie::Keep);
    }
}
