//! Watches mouse buttons, modifier keys and gamepads for the extra
//! triggers of `[triggers]` and runs their actions.
//!
//! Only the devices a configured trigger needs are watched; with no
//! trigger set nothing runs at all. Input is observed, never taken away
//! from the application in front (Windows raw input, XInput polling), and
//! nothing about it is stored: key presses are reduced to "a modifier" or
//! "another key" before they reach the detector (`trigger.rs`).
//!
//! Windows only. Elsewhere [`SUPPORTED`] is false and nothing is watched.

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

use crate::hotkey::Action;
use crate::settings::schema::Triggers;
use crate::trigger::{Detector, Output, Sink, Sources, Trigger};

pub const SUPPORTED: bool = cfg!(windows);

/// How long the settings screen waits for the user to perform a trigger.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Default)]
struct Capture {
    /// Counts captures, so a finished one does not end its successor.
    generation: u64,
    reply: Option<oneshot::Sender<Trigger>>,
}

#[derive(Default)]
pub struct Service {
    detector: Arc<Mutex<Detector<Action>>>,
    capture: Arc<Mutex<Capture>>,
    watch: Mutex<Option<(Sources, backend::Watch)>>,
}

fn service(app: &AppHandle) -> tauri::State<'_, Service> {
    if app.try_state::<Service>().is_none() {
        app.manage(Service::default());
    }
    app.state::<Service>()
}

/// Bind the triggers of the settings. Returns the ones that are not a
/// trigger (shown with the shortcuts that failed to register).
pub fn apply(app: &AppHandle, triggers: &Triggers) -> Vec<String> {
    let mut failed = Vec::new();
    let mut bindings = Vec::new();
    for (action, text) in crate::hotkey::triggers(triggers) {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        match text.parse::<Trigger>() {
            Ok(t) => bindings.push((t, action)),
            Err(_) => {
                tracing::warn!("trigger {text} not understood");
                failed.push(text.to_string());
            }
        }
    }
    let svc = service(app);
    svc.detector.lock().set_bindings(bindings);
    svc.sync(app);
    failed
}

/// Wait for the user to perform a trigger (settings screen). `None` when
/// nothing came in time, the capture was cancelled or replaced by a newer
/// one, or the platform has no watcher.
pub async fn capture(app: &AppHandle) -> Option<Trigger> {
    if !SUPPORTED {
        return None;
    }
    let svc = service(app);
    let (tx, rx) = oneshot::channel();
    let generation = {
        let mut c = svc.capture.lock();
        c.generation += 1;
        c.reply = Some(tx);
        c.generation
    };
    svc.detector.lock().set_capturing(true);
    // A trigger held right now loses its release.
    if let Some(state) = app.try_state::<crate::app::AppState>() {
        crate::lens::cancel_hold(app, &state.lenses);
    }
    svc.sync(app);
    let got = tokio::time::timeout(CAPTURE_TIMEOUT, rx)
        .await
        .ok()
        .and_then(Result::ok);
    let current = {
        let mut c = svc.capture.lock();
        let current = c.generation == generation;
        if current {
            c.reply = None;
        }
        current
    };
    if current {
        svc.detector.lock().set_capturing(false);
        svc.sync(app);
    }
    got
}

/// End a running [`capture`] without a result.
pub fn cancel_capture(app: &AppHandle) {
    service(app).capture.lock().reply = None;
}

impl Service {
    /// Start, stop or restart the watcher so it covers exactly the
    /// devices the detector needs.
    fn sync(&self, app: &AppHandle) {
        let wanted = self.detector.lock().sources();
        let mut watch = self.watch.lock();
        if watch.as_ref().map(|(s, _)| *s).unwrap_or_default() == wanted {
            return;
        }
        // Stop the old threads first (dropping joins them).
        *watch = None;
        if wanted.any() {
            *watch = Some((wanted, backend::Watch::start(wanted, self.sink(app))));
        }
    }

    fn sink(&self, app: &AppHandle) -> Sink {
        static START: OnceLock<Instant> = OnceLock::new();
        let (app, detector, capture) = (app.clone(), self.detector.clone(), self.capture.clone());
        Arc::new(move |event| {
            let now = START.get_or_init(Instant::now).elapsed().as_millis() as u64;
            let outputs = detector.lock().feed(now, event);
            for output in outputs {
                match output {
                    Output::Fire { action, pressed } => {
                        // Like the global shortcuts: on the main thread.
                        let app_m = app.clone();
                        let _ = app.run_on_main_thread(move || action.run(&app_m, pressed));
                    }
                    Output::Captured(trigger) => {
                        if let Some(reply) = capture.lock().reply.take() {
                            let _ = reply.send(trigger);
                        }
                    }
                }
            }
        })
    }
}

#[cfg(not(windows))]
mod backend {
    use crate::trigger::{Sink, Sources};

    pub struct Watch;

    impl Watch {
        pub fn start(_sources: Sources, _sink: Sink) -> Self {
            Watch
        }
    }
}

#[cfg(windows)]
use crate::trigger_win as backend;
