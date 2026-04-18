//! File-watcher that keeps the in-memory settings store in sync with
//! manual edits to `config.toml` on disk.
//!
//! The watcher spawns a dedicated OS thread that runs `notify`'s
//! recommended watcher, debounces bursty events (editors like Vim and
//! VS Code rename-over-original on save, producing 2-4 events per
//! save), and then calls [`SettingsStore::reload_from_disk`].
//!
//! `Watcher` is `Send + Sync`; drop it to stop the thread.

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

use super::SettingsStore;

/// Minimum quiet time between consecutive reloads. Bursts of filesystem
/// events during an editor save collapse into a single reload.
const DEBOUNCE: Duration = Duration::from_millis(120);

pub struct Watcher {
    _thread: Option<thread::JoinHandle<()>>,
    shutdown: mpsc::Sender<()>,
}

impl Watcher {
    pub fn spawn(store: Arc<SettingsStore>) -> Result<Self> {
        let path: PathBuf = store.config_path().to_path_buf();
        let watch_dir = path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let (fs_tx, fs_rx) = mpsc::channel::<notify::Result<Event>>();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
            let _ = fs_tx.send(res);
        })?;
        watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;

        let store_t = store.clone();
        let path_t = path.clone();
        let handle = thread::Builder::new()
            .name("moonlens-settings-watch".into())
            .spawn(move || {
                // Hold the watcher alive for the thread's lifetime.
                let _keep = watcher;
                let mut last_reload: Option<Instant> = None;
                loop {
                    if shutdown_rx.try_recv().is_ok() {
                        break;
                    }
                    match fs_rx.recv_timeout(Duration::from_millis(250)) {
                        Ok(Ok(event)) if event_touches(&event, &path_t) => {
                            // Debounce: ignore events that arrive within
                            // `DEBOUNCE` of the previous reload.
                            let now = Instant::now();
                            if last_reload
                                .map(|t| now.duration_since(t) < DEBOUNCE)
                                .unwrap_or(false)
                            {
                                continue;
                            }
                            last_reload = Some(now);
                            if let Err(e) = store_t.reload_from_disk() {
                                tracing::warn!("settings hot-reload failed: {e}");
                            }
                        }
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => tracing::warn!("settings watcher error: {e}"),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(Self {
            _thread: Some(handle),
            shutdown: shutdown_tx,
        })
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
    }
}

fn event_touches(event: &Event, target: &std::path::Path) -> bool {
    matches!(
        event.kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
    ) && event.paths.iter().any(|p| p == target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};
    use std::path::PathBuf;

    fn ev(kind: EventKind, paths: Vec<PathBuf>) -> Event {
        Event {
            kind,
            paths,
            attrs: Default::default(),
        }
    }

    #[test]
    fn event_touches_matches_modify_on_target() {
        let target = PathBuf::from("/tmp/config.toml");
        let e = ev(EventKind::Modify(ModifyKind::Any), vec![target.clone()]);
        assert!(event_touches(&e, &target));
    }

    #[test]
    fn event_touches_matches_create_and_remove() {
        let target = PathBuf::from("/tmp/config.toml");
        let c = ev(EventKind::Create(CreateKind::File), vec![target.clone()]);
        let r = ev(EventKind::Remove(RemoveKind::File), vec![target.clone()]);
        assert!(event_touches(&c, &target));
        assert!(event_touches(&r, &target));
    }

    #[test]
    fn event_touches_ignores_other_files() {
        let target = PathBuf::from("/tmp/config.toml");
        let e = ev(
            EventKind::Modify(ModifyKind::Any),
            vec![PathBuf::from("/tmp/other.toml")],
        );
        assert!(!event_touches(&e, &target));
    }

    #[test]
    fn event_touches_ignores_access_events() {
        use notify::event::AccessKind;
        let target = PathBuf::from("/tmp/config.toml");
        let e = ev(EventKind::Access(AccessKind::Any), vec![target.clone()]);
        assert!(!event_touches(&e, &target));
    }
}
