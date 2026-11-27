//! Composite windows for stream capture.
//!
//! Streaming and recording tools capture one window at a time, so a
//! capture of the game window has no translations in it: those are in
//! MoonLens' own windows. For a window that translation windows are
//! attached to ([`crate::attach`]), a composite window shows that
//! window with their translations drawn over it (crate
//! `moonlens-stream`; Windows only), for the tool to capture instead.
//!
//! One composite per attached-to window, shared by every translation
//! window attached to it. Each of those sends what it shows
//! ([`set_stream_paint`]) while the composite runs; it ends with the
//! last attachment, with the window it mirrors, or when the user closes
//! it. The translation windows hear about every change through
//! [`EVENT`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use moonlens_stream::{Compositor, Item, Layer};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager, State};

use crate::attach;

/// Event with the [`StreamState`] of the window it is sent to.
pub const EVENT: &str = "moonlens://stream";

/// Whether the window's translations go into a composite window.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct StreamState {
    /// This platform has composite windows.
    pub supported: bool,
    /// A composite of the window this one is attached to is running.
    pub active: bool,
    /// Title of that composite window: what to pick in the capture tool.
    pub title: Option<String>,
}

struct Composite {
    /// Tells this composite from an earlier one of the same window.
    serial: u64,
    compositor: Compositor,
}

/// What a translation window shows, and its native handle.
struct Paint {
    window: isize,
    items: Vec<Item>,
}

/// The running composites, by the id of the window they mirror.
#[derive(Default)]
pub struct Streams {
    composites: Mutex<HashMap<u64, Composite>>,
    paints: Mutex<HashMap<String, Paint>>,
    serial: AtomicU64,
}

pub type Handle = Arc<Streams>;

impl Streams {
    fn active(&self, target: u64) -> bool {
        self.composites.lock().contains_key(&target)
    }

    /// A translation window is closing.
    pub fn forget(&self, label: &str) {
        self.paints.lock().remove(label);
    }
}

fn target_of(app: &AppHandle, label: &str) -> Option<attach::Target> {
    app.state::<attach::Handle>().attachment(label).target
}

fn state(app: &AppHandle, streams: &Streams, label: &str) -> StreamState {
    let target = target_of(app, label).filter(|t| streams.active(t.id));
    StreamState {
        supported: moonlens_stream::SUPPORTED,
        active: target.is_some(),
        title: target.as_ref().map(title),
    }
}

fn emit(app: &AppHandle, streams: &Streams, label: &str) {
    let target = EventTarget::WebviewWindow {
        label: label.to_string(),
    };
    let _ = app.emit_to(target, EVENT, state(app, streams, label));
}

/// Hand the composite of `target` what the windows attached to that
/// window show now.
fn refresh(app: &AppHandle, streams: &Streams, target: u64) {
    let labels = app.state::<attach::Handle>().attached_to(target);
    let scene = {
        let paints = streams.paints.lock();
        labels
            .iter()
            .filter_map(|label| paints.get(label))
            .map(|p| Layer {
                window: p.window,
                items: p.items.clone(),
            })
            .collect()
    };
    if let Some(c) = streams.composites.lock().get(&target) {
        c.compositor.set_scene(scene);
    }
}

fn title(target: &attach::Target) -> String {
    let name = if target.title.trim().is_empty() {
        target.app.trim()
    } else {
        target.title.trim()
    };
    format!("MoonLens 配信 - {name}")
}

/// Open the composite of the window `label` is attached to.
fn start(app: &AppHandle, streams: &Handle, label: &str) -> Result<(), String> {
    let target = target_of(app, label)
        .ok_or_else(|| "stream_needs_attach|吸着先の指定が必要".to_string())?;
    if streams.active(target.id) {
        return Ok(());
    }
    let serial = streams.serial.fetch_add(1, Ordering::Relaxed) + 1;
    let (app_end, streams_end, id) = (app.clone(), streams.clone(), target.id);
    let compositor = Compositor::start(target.id as isize, &title(&target), move || {
        ended(&app_end, &streams_end, id, serial);
    })
    .map_err(|e| match e {
        moonlens_stream::Error::Unsupported => {
            "stream_unsupported|配信用ウィンドウは当該環境で利用不可".to_string()
        }
        moonlens_stream::Error::TargetGone => {
            "attach_target_gone|吸着先のウィンドウが存在しません".to_string()
        }
        moonlens_stream::Error::Os(e) => {
            tracing::warn!("stream composite of window {id}: {e}");
            format!("stream_failed|配信用ウィンドウの生成に失敗：{e}")
        }
    })?;
    tracing::info!("stream composite of window {id} opened");
    streams
        .composites
        .lock()
        .insert(target.id, Composite { serial, compositor });
    refresh(app, streams, target.id);
    announce(app, streams, target.id);
    Ok(())
}

/// The composite `serial` of window `target` closed (on its thread).
fn ended(app: &AppHandle, streams: &Streams, target: u64, serial: u64) {
    {
        let mut composites = streams.composites.lock();
        if composites.get(&target).is_some_and(|c| c.serial == serial) {
            composites.remove(&target);
        }
    }
    tracing::info!("stream composite of window {target} closed");
    announce(app, streams, target);
}

/// Tell every window attached to `target` its state.
fn announce(app: &AppHandle, streams: &Streams, target: u64) {
    for label in app.state::<attach::Handle>().attached_to(target) {
        emit(app, streams, &label);
    }
}

fn stop(app: &AppHandle, streams: &Streams, target: u64) {
    // Dropped outside the lock: closing ends in `ended`.
    let closed = streams.composites.lock().remove(&target);
    drop(closed);
    announce(app, streams, target);
}

/// The attachment of `label` changed: composites nothing is attached
/// to any more end, the others take the windows they have now.
pub fn attachment_changed(app: &AppHandle, label: &str) {
    let Some(streams) = app.try_state::<Handle>() else {
        return;
    };
    let targets: Vec<u64> = streams.composites.lock().keys().copied().collect();
    let attachments = app.state::<attach::Handle>();
    for target in targets {
        if attachments.attached_to(target).is_empty() {
            stop(app, &streams, target);
        } else {
            refresh(app, &streams, target);
        }
    }
    emit(app, &streams, label);
}

#[cfg(windows)]
fn native_handle(app: &AppHandle, label: &str) -> Option<isize> {
    app.get_webview_window(label)?
        .hwnd()
        .ok()
        .map(|h| h.0 as isize)
}

#[cfg(not(windows))]
fn native_handle(_app: &AppHandle, _label: &str) -> Option<isize> {
    None
}

#[tauri::command]
pub async fn stream_state(
    label: String,
    app: AppHandle,
    streams: State<'_, Handle>,
) -> Result<StreamState, String> {
    Ok(state(&app, &streams, &label))
}

/// Open (`on`) or close the composite of the window `label` is
/// attached to.
#[tauri::command]
pub async fn set_stream(
    label: String,
    on: bool,
    app: AppHandle,
    streams: State<'_, Handle>,
) -> Result<StreamState, String> {
    if on {
        start(&app, &streams, &label)?;
    } else if let Some(target) = target_of(&app, &label) {
        stop(&app, &streams, target.id);
    }
    Ok(state(&app, &streams, &label))
}

/// What window `label` shows now: the translated boxes and their text
/// lines, in physical pixels of its client area.
#[tauri::command]
pub async fn set_stream_paint(
    label: String,
    items: Vec<Item>,
    app: AppHandle,
    streams: State<'_, Handle>,
) -> Result<(), String> {
    let Some(window) = native_handle(&app, &label) else {
        return Ok(());
    };
    streams
        .paints
        .lock()
        .insert(label.clone(), Paint { window, items });
    if let Some(target) = target_of(&app, &label) {
        refresh(&app, &streams, target.id);
    }
    Ok(())
}
