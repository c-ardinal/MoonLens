//! Problems found outside a user action (start-up, a settings file edited
//! by hand, a hotkey another app owns) that the user still has to hear
//! about. Each notice has a stable id so the same problem is listed once;
//! the main window shows them on the home screen until dismissed.

use std::sync::OnceLock;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Notice {
    /// Stable key: pushing the same id again replaces the old text.
    pub id: String,
    pub message: String,
    /// Optional in-app page that fixes the problem (e.g.
    /// `/settings/#section-hotkeys`).
    pub action: Option<String>,
}

static NOTICES: Mutex<Vec<Notice>> = Mutex::new(Vec::new());
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Let later pushes reach open windows (`moonlens://notices`).
pub fn attach(app: &AppHandle) {
    let _ = APP.set(app.clone());
    emit();
}

pub fn push(id: &str, message: impl Into<String>, action: Option<&str>) {
    let n = Notice {
        id: id.to_string(),
        message: message.into(),
        action: action.map(str::to_string),
    };
    tracing::warn!("notice {}: {}", n.id, n.message);
    {
        let mut all = NOTICES.lock();
        match all.iter_mut().find(|x| x.id == n.id) {
            Some(slot) => *slot = n,
            None => all.push(n),
        }
    }
    emit();
}

/// Drop a notice because its cause went away (or the user dismissed it).
pub fn clear(id: &str) {
    let removed = {
        let mut all = NOTICES.lock();
        let before = all.len();
        all.retain(|n| n.id != id);
        before != all.len()
    };
    if removed {
        emit();
    }
}

pub fn list() -> Vec<Notice> {
    NOTICES.lock().clone()
}

fn emit() {
    if let Some(app) = APP.get() {
        let _ = app.emit("moonlens://notices", list());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_id_replaces_and_clear_removes() {
        push("t.a", "one", None);
        push("t.a", "two", Some("/settings/"));
        let a: Vec<_> = list().into_iter().filter(|n| n.id == "t.a").collect();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].message, "two");
        clear("t.a");
        assert!(list().iter().all(|n| n.id != "t.a"));
    }
}
