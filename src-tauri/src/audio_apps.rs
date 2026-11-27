//! The applications a subtitle window can listen to.
//!
//! Windows keeps one audio session per process that opened an output
//! stream. A browser or a WebView2 application plays through a child
//! process, so sessions are grouped into applications ([`group`]): the
//! process at the top of each group is the one a process loopback is
//! opened on, with its child processes included.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

/// An application that has an audio output stream open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioApp {
    /// The process to capture, child processes included.
    pub pid: u32,
    /// Executable name without its extension.
    pub name: String,
    /// Title of the application's window, when it has one.
    pub title: Option<String>,
    /// Sound is playing right now.
    pub playing: bool,
}

/// What the source menu of a subtitle window shows.
#[derive(Debug, Clone, Serialize)]
pub struct AudioApps {
    /// Capture per application exists on this OS.
    pub supported: bool,
    pub apps: Vec<AudioApp>,
}

/// A running process, as far as grouping needs it.
#[derive(Debug, Clone)]
pub struct Proc {
    pub parent: Option<u32>,
    pub name: String,
}

/// Processes that only start other applications: an application never
/// belongs to their group, whatever window they show.
const SHELLS: [&str; 3] = ["explorer", "svchost", "sihost"];

/// Longest parent chain followed, against cycles in a stale snapshot.
const MAX_DEPTH: usize = 16;

fn stem(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

/// The process whose tree is "the application" `pid` plays for: up the
/// parent chain while the parent is the same program (browser → its
/// audio process), or while only the parent shows a window (WebView2
/// host → its webview processes).
pub fn root_of(pid: u32, procs: &HashMap<u32, Proc>, titles: &HashMap<u32, String>) -> u32 {
    let mut at = pid;
    for _ in 0..MAX_DEPTH {
        let Some(me) = procs.get(&at) else { break };
        let Some((parent_pid, parent)) = me
            .parent
            .and_then(|p| procs.get(&p).map(|parent| (p, parent)))
        else {
            break;
        };
        let same_program = stem(&parent.name) == stem(&me.name);
        let hosted = !titles.contains_key(&at)
            && titles.contains_key(&parent_pid)
            && !SHELLS.contains(&stem(&parent.name).as_str());
        if !(same_program || hosted) {
            break;
        }
        at = parent_pid;
    }
    at
}

/// `pid` is `ancestor` or was started (directly or not) by it.
fn descends_from(pid: u32, ancestor: u32, procs: &HashMap<u32, Proc>) -> bool {
    let mut at = Some(pid);
    for _ in 0..MAX_DEPTH {
        match at {
            Some(p) if p == ancestor => return true,
            Some(p) => at = procs.get(&p).and_then(|x| x.parent),
            None => return false,
        }
    }
    false
}

/// Audio sessions (`pid`, playing) as applications: one entry per
/// process group, playing ones first, without the processes of `own`
/// (this application) when given.
pub fn group(
    sessions: &[(u32, bool)],
    procs: &HashMap<u32, Proc>,
    titles: &HashMap<u32, String>,
    own: Option<u32>,
) -> Vec<AudioApp> {
    let mut apps = BTreeMap::<u32, AudioApp>::new();
    for &(pid, playing) in sessions {
        let ours = own.is_some_and(|own| descends_from(pid, own, procs));
        if !procs.contains_key(&pid) || ours {
            continue;
        }
        let root = root_of(pid, procs, titles);
        let app = apps.entry(root).or_insert_with(|| AudioApp {
            pid: root,
            name: stem(&procs[&root].name),
            title: titles.get(&root).cloned(),
            playing: false,
        });
        app.playing |= playing;
        if app.title.is_none() {
            app.title = titles.get(&pid).cloned();
        }
    }
    let mut apps: Vec<AudioApp> = apps.into_values().collect();
    apps.sort_by(|a, b| (!a.playing, &a.name, a.pid).cmp(&(!b.playing, &b.name, b.pid)));
    apps
}

/// The applications with an audio output stream, for the source menu:
/// all but this application itself.
pub fn list() -> AudioApps {
    list_except(Some(std::process::id()))
}

/// [`list`] without the processes of `own` (and those it started).
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn list_except(own: Option<u32>) -> AudioApps {
    #[cfg(windows)]
    {
        let apps = match platform::sessions() {
            Ok(sessions) => group(&sessions, &processes(), &platform::window_titles(), own),
            Err(e) => {
                tracing::warn!("audio: listing applications failed: {e:#}");
                Vec::new()
            }
        };
        AudioApps {
            supported: true,
            apps,
        }
    }
    #[cfg(not(windows))]
    AudioApps {
        supported: false,
        apps: Vec::new(),
    }
}

/// Executable name (without extension) of the running process `pid`.
pub fn process_name(pid: u32) -> Option<String> {
    refreshed(pid)
        .process(sysinfo::Pid::from_u32(pid))
        .map(|p| stem(&p.name().to_string_lossy()))
}

pub fn is_alive(pid: u32) -> bool {
    refreshed(pid)
        .process(sysinfo::Pid::from_u32(pid))
        .is_some()
}

fn refreshed(pid: u32) -> sysinfo::System {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    system
}

#[cfg_attr(not(windows), allow(dead_code))]
fn processes() -> HashMap<u32, Proc> {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    system
        .processes()
        .iter()
        .map(|(pid, p)| {
            (
                pid.as_u32(),
                Proc {
                    parent: p.parent().map(|x| x.as_u32()),
                    name: p.name().to_string_lossy().into_owned(),
                },
            )
        })
        .collect()
}

#[cfg(windows)]
mod platform {
    use std::collections::HashMap;

    use anyhow::{anyhow, Result};
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    /// Audio sessions on every active output device: (process, playing).
    /// COM is used on a thread of its own, whatever mode the caller's
    /// thread is in.
    pub fn sessions() -> Result<Vec<(u32, bool)>> {
        std::thread::spawn(|| {
            let _ = wasapi::initialize_mta();
            let found = sessions_here();
            wasapi::deinitialize();
            found
        })
        .join()
        .map_err(|_| anyhow!("audio session listing panicked"))?
    }

    fn sessions_here() -> Result<Vec<(u32, bool)>> {
        use wasapi::{DeviceEnumerator, Direction, SessionState};
        let fail = |e: wasapi::WasapiError| anyhow!("{e}");
        let enumerator = DeviceEnumerator::new().map_err(fail)?;
        let devices = enumerator
            .get_device_collection(Direction::Render)
            .map_err(fail)?;
        let mut found = Vec::new();
        for device in &devices {
            // A device that went away or cannot be opened: skip it.
            let Ok(sessions) = device
                .and_then(|d| d.get_iaudiosessionmanager())
                .and_then(|m| m.get_audiosessionenumerator())
            else {
                continue;
            };
            for i in 0..sessions.get_count().unwrap_or(0) {
                let Ok(session) = sessions.get_session(i) else {
                    continue;
                };
                let (Ok(state), Ok(pid)) = (session.get_state(), session.get_process_id()) else {
                    continue;
                };
                // Process 0 is the system sounds session.
                if pid != 0 && state != SessionState::Expired {
                    found.push((pid, state == SessionState::Active));
                }
            }
        }
        Ok(found)
    }

    /// Title of the topmost visible titled window of each process.
    pub fn window_titles() -> HashMap<u32, String> {
        unsafe extern "system" fn each(hwnd: HWND, out: LPARAM) -> BOOL {
            // SAFETY: `out` is the map `window_titles` passes below and
            // outlives the enumeration; `hwnd` comes from the system.
            let titles = unsafe { &mut *(out.0 as *mut HashMap<u32, String>) };
            let mut text = [0u16; 256];
            let mut pid = 0u32;
            let len = unsafe {
                if !IsWindowVisible(hwnd).as_bool() {
                    return true.into();
                }
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                GetWindowTextW(hwnd, &mut text)
            };
            if pid != 0 && len > 0 {
                titles
                    .entry(pid)
                    .or_insert_with(|| String::from_utf16_lossy(&text[..len as usize]));
            }
            true.into()
        }
        let mut titles = HashMap::new();
        // SAFETY: the callback only runs during this call.
        let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut titles as *mut _ as isize)) };
        titles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn procs(list: &[(u32, Option<u32>, &str)]) -> HashMap<u32, Proc> {
        list.iter()
            .map(|&(pid, parent, name)| {
                (
                    pid,
                    Proc {
                        parent,
                        name: name.into(),
                    },
                )
            })
            .collect()
    }

    fn titles(list: &[(u32, &str)]) -> HashMap<u32, String> {
        list.iter().map(|&(pid, t)| (pid, t.to_string())).collect()
    }

    #[test]
    fn a_browser_is_one_application_captured_at_its_main_process() {
        let procs = procs(&[
            (1, None, "explorer.exe"),
            (10, Some(1), "chrome.exe"),
            (11, Some(10), "chrome.exe"),
            (12, Some(10), "chrome.exe"),
        ]);
        let titles = titles(&[(1, "Downloads"), (10, "Video - Google Chrome")]);
        let apps = group(&[(11, true), (12, false)], &procs, &titles, Some(999));
        assert_eq!(
            apps,
            vec![AudioApp {
                pid: 10,
                name: "chrome".into(),
                title: Some("Video - Google Chrome".into()),
                playing: true,
            }]
        );
    }

    #[test]
    fn a_webview_application_is_listed_as_its_host() {
        let procs = procs(&[
            (1, None, "explorer.exe"),
            (20, Some(1), "ms-teams.exe"),
            (21, Some(20), "msedgewebview2.exe"),
            (22, Some(21), "msedgewebview2.exe"),
        ]);
        let titles = titles(&[(20, "Teams")]);
        let apps = group(&[(22, false)], &procs, &titles, Some(999));
        assert_eq!(apps.len(), 1);
        assert_eq!((apps[0].pid, apps[0].name.as_str()), (20, "ms-teams"));
    }

    #[test]
    fn a_windowless_player_is_not_merged_into_the_shell() {
        let procs = procs(&[(1, None, "explorer.exe"), (30, Some(1), "player.exe")]);
        let titles = titles(&[(1, "Downloads")]);
        let apps = group(&[(30, true)], &procs, &titles, Some(999));
        assert_eq!((apps[0].pid, apps[0].name.as_str()), (30, "player"));
        assert_eq!(apps[0].title, None);
    }

    #[test]
    fn playing_applications_come_first_and_own_processes_are_left_out() {
        let procs = procs(&[
            (40, None, "moonlens.exe"),
            (41, Some(40), "msedgewebview2.exe"),
            (50, None, "zplayer.exe"),
            (60, None, "aplayer.exe"),
        ]);
        let apps = group(
            &[(41, true), (60, false), (50, true), (70, true)],
            &procs,
            &HashMap::new(),
            Some(40),
        );
        // Without an own process (tests that start the players).
        let all = group(&[(41, true)], &procs, &HashMap::new(), None);
        assert_eq!(all.len(), 1);
        let names: Vec<&str> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["zplayer", "aplayer"]);
    }

    #[test]
    fn this_process_is_found_by_its_id() {
        let pid = std::process::id();
        assert!(is_alive(pid));
        assert!(process_name(pid).is_some_and(|n| !n.is_empty()));
        assert!(!is_alive(u32::MAX - 7));
    }
}
