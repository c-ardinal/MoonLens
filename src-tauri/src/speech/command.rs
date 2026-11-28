//! Speech through a command of the OS: `say` on macOS, Speech
//! Dispatcher (`spd-say`) on Linux. The command runs for as long as it
//! speaks, so stopping is ending the process.

#![cfg_attr(windows, allow(dead_code))]

use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result};

use super::{Engine, Tuning, Voice};

/// Voices of `say -v ?`: `Kyoko               ja_JP    # こんにちは`.
/// Names may hold spaces ("Bad News").
pub fn parse_say_voices(listing: &str) -> Vec<Voice> {
    listing
        .lines()
        .filter_map(|line| {
            let head = line.split('#').next()?.trim_end();
            let (name, locale) = head.rsplit_once(char::is_whitespace)?;
            let name = name.trim();
            (!name.is_empty() && !locale.is_empty()).then(|| Voice {
                id: name.to_string(),
                name: name.to_string(),
                lang: locale.replace('_', "-"),
            })
        })
        .collect()
}

/// Voices of `spd-say -L`: a header row, then `NAME LANGUAGE VARIANT`
/// in columns. Names may hold spaces.
pub fn parse_spd_voices(listing: &str) -> Vec<Voice> {
    listing
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (rest, _variant) = line.trim().rsplit_once(char::is_whitespace)?;
            let (name, lang) = rest.trim_end().rsplit_once(char::is_whitespace)?;
            let name = name.trim();
            (!name.is_empty() && !lang.is_empty()).then(|| Voice {
                id: name.to_string(),
                name: name.to_string(),
                lang: lang.to_string(),
            })
        })
        .collect()
}

/// Words per minute of `say -r` at the voice's own speed.
const SAY_WPM: f32 = 175.0;

/// `spd-say` takes rate and volume as -100..=100 around the default.
fn spd_scale(ratio: f32) -> i32 {
    ((ratio - 1.0) * 100.0).round().clamp(-100.0, 100.0) as i32
}

fn listing(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

#[derive(Default)]
pub struct CommandEngine {
    child: Option<Child>,
}

impl CommandEngine {
    fn command(text: &str, voice: &Voice, tuning: &Tuning) -> Command {
        if cfg!(target_os = "macos") {
            let mut c = Command::new("say");
            c.args(["-v", &voice.id])
                .args(["-r", &(SAY_WPM * tuning.rate).round().to_string()])
                // `say` has no volume option; the text may set it.
                .arg(format!("[[volm {:.2}]] {text}", tuning.volume));
            c
        } else {
            let mut c = Command::new("spd-say");
            c.arg("--wait")
                .args(["-l", &voice.lang])
                .args(["-y", &voice.id])
                .args(["-r", &spd_scale(tuning.rate).to_string()])
                .args(["-i", &spd_scale(tuning.volume * 2.0).to_string()])
                .arg("--")
                .arg(text);
            c
        }
    }
}

impl Engine for CommandEngine {
    fn voices(&mut self) -> Vec<Voice> {
        if cfg!(target_os = "macos") {
            parse_say_voices(&listing("say", &["-v", "?"]))
        } else {
            parse_spd_voices(&listing("spd-say", &["-L"]))
        }
    }

    fn start(&mut self, text: &str, voice: &Voice, tuning: &Tuning) -> Result<()> {
        self.stop();
        let child = Self::command(text, voice, tuning)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("speech command")?;
        self.child = Some(child);
        Ok(())
    }

    fn playing(&mut self) -> bool {
        match self.child.as_mut().map(Child::try_wait) {
            Some(Ok(None)) => true,
            _ => {
                self.child = None;
                false
            }
        }
    }

    fn stop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = child.kill();
        let _ = child.wait();
        // Speech Dispatcher speaks on in its own process.
        if cfg!(target_os = "linux") {
            let _ = Command::new("spd-say")
                .arg("--cancel")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

impl Drop for CommandEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn say_listing_keeps_names_with_spaces() {
        let voices = parse_say_voices(
            "Kyoko               ja_JP    # こんにちは、私の名前はKyokoです。\n\
             Bad News            en_US    # The light you see is a train.\n\
             Eddy (English (US)) en_US    # Hello!\n\n",
        );
        let pairs: Vec<_> = voices
            .iter()
            .map(|v| (v.id.as_str(), v.lang.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("Kyoko", "ja-JP"),
                ("Bad News", "en-US"),
                ("Eddy (English (US))", "en-US")
            ]
        );
    }

    #[test]
    fn spd_listing_skips_the_header() {
        let voices = parse_spd_voices(
            "                NAME                    LANGUAGE                     VARIANT\n\
             \x20           Japanese                          ja                        none\n\
             \x20 English (America)                       en-US                        none\n",
        );
        let pairs: Vec<_> = voices
            .iter()
            .map(|v| (v.id.as_str(), v.lang.as_str()))
            .collect();
        assert_eq!(pairs, [("Japanese", "ja"), ("English (America)", "en-US")]);
    }

    #[test]
    fn spd_scale_centres_on_the_default() {
        assert_eq!(spd_scale(1.0), 0);
        assert_eq!(spd_scale(0.5), -50);
        assert_eq!(spd_scale(2.0), 100);
        assert_eq!(spd_scale(3.0), 100);
    }
}
