//! Subtitles burnt into a copy of the video.
//!
//! MoonLens hands the work to an `ffmpeg` executable: it decodes the
//! video, draws the subtitles with libass and encodes the result frame
//! by frame, so memory stays flat whatever the length or resolution.
//! The audio is copied as it is. This module writes the subtitles as an
//! ASS script (style, size and line breaks fitted to the frame), picks
//! the encoder the PC can run (GPU first), and runs and stops ffmpeg.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::Serialize;

use crate::storage::SubtitleLine;
use crate::subtitle_record::ExportContent;

/// What ffmpeg says of the input before it starts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    pub duration_ms: Option<u64>,
    pub width: u32,
    pub height: u32,
    /// Bitrate of the video stream, else of the whole file.
    pub video_kbps: Option<u32>,
    pub has_audio: bool,
}

/// Read [`Probe`] from the banner `ffmpeg -i <file>` prints on stderr.
pub fn parse_probe(banner: &str) -> Option<Probe> {
    let mut probe = Probe::default();
    let mut file_kbps = None;
    for line in banner.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("Duration: ") {
            probe.duration_ms = rest.split(',').next().and_then(parse_clock);
            file_kbps = rest
                .split("bitrate: ")
                .nth(1)
                .and_then(|b| b.split_whitespace().next()?.parse().ok());
        } else if line.starts_with("Stream #") && line.contains(": Video: ") && probe.width == 0 {
            // Attached cover pictures come as video streams too.
            if line.contains("(attached pic)") {
                continue;
            }
            let fields: Vec<&str> = line.split(", ").collect();
            for field in &fields {
                let head = field.split_whitespace().next().unwrap_or("");
                if let Some((w, h)) = head.split_once('x') {
                    if let (Ok(w), Ok(h)) = (w.parse(), h.parse()) {
                        probe.width = w;
                        probe.height = h;
                    }
                }
                if let Some(kbps) = field.strip_suffix(" kb/s") {
                    probe.video_kbps = kbps.trim().parse().ok();
                }
            }
        } else if line.starts_with("Stream #") && line.contains(": Audio: ") {
            probe.has_audio = true;
        }
    }
    if probe.width == 0 {
        return None;
    }
    probe.video_kbps = probe.video_kbps.or(file_kbps);
    Some(probe)
}

/// `01:02:03.45` → milliseconds.
fn parse_clock(s: &str) -> Option<u64> {
    let mut parts = s.trim().split(':');
    let h: u64 = parts.next()?.parse().ok()?;
    let m: u64 = parts.next()?.parse().ok()?;
    let sec: f64 = parts.next()?.parse().ok()?;
    Some(h * 3_600_000 + m * 60_000 + (sec * 1000.0).round() as u64)
}

/// H.264 encoders, in the order they are tried: the GPU ones first,
/// then Windows' own, then software.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoder {
    Nvenc,
    Qsv,
    Amf,
    MediaFoundation,
    X264,
    OpenH264,
}

impl Encoder {
    pub const ORDER: [Encoder; 6] = [
        Encoder::Nvenc,
        Encoder::Qsv,
        Encoder::Amf,
        Encoder::MediaFoundation,
        Encoder::X264,
        Encoder::OpenH264,
    ];

    pub fn codec(self) -> &'static str {
        match self {
            Encoder::Nvenc => "h264_nvenc",
            Encoder::Qsv => "h264_qsv",
            Encoder::Amf => "h264_amf",
            Encoder::MediaFoundation => "h264_mf",
            Encoder::X264 => "libx264",
            Encoder::OpenH264 => "libopenh264",
        }
    }

    /// Quality settings. Encoders that take a target quality get one
    /// close to the source; the others get the source's bitrate.
    fn args(self, kbps: u32) -> Vec<String> {
        let bitrate = format!("{kbps}k");
        let list: &[&str] = match self {
            Encoder::Nvenc => &["-preset", "p4", "-rc", "vbr", "-cq", "21", "-b:v", "0"],
            Encoder::Qsv => &["-global_quality", "21"],
            Encoder::Amf => &["-rc", "cqp", "-qp_i", "20", "-qp_p", "22"],
            Encoder::MediaFoundation => &["-b:v", &bitrate],
            Encoder::X264 => &["-preset", "veryfast", "-crf", "20"],
            Encoder::OpenH264 => &["-b:v", &bitrate],
        };
        let mut args = vec!["-c:v".to_string(), self.codec().to_string()];
        args.extend(list.iter().map(|s| s.to_string()));
        args
    }
}

/// Bitrate for encoders without a quality mode: the source's, or about
/// 0.08 bit per pixel at 30 fps when the file does not say.
fn target_kbps(probe: &Probe) -> u32 {
    let guess = (probe.width as u64 * probe.height as u64 * 30 * 8 / 100 / 1000) as u32;
    probe.video_kbps.unwrap_or(guess).clamp(1_000, 80_000)
}

/// Where the ffmpeg executable is: `configured` (the setting) when it
/// names a file, else `MOONLENS_FFMPEG`, next to MoonLens (also in
/// `tools/`), then the `PATH`. MoonLens does not ship ffmpeg: the user
/// places it.
pub fn locate_ffmpeg(configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() && Path::new(configured).is_file() {
        return Some(PathBuf::from(configured));
    }
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Some(p) = std::env::var_os("MOONLENS_FFMPEG") {
        return Some(PathBuf::from(p));
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        for p in [dir.join(name), dir.join("tools").join(name)] {
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// Whether `path` is an ffmpeg that can burn subtitles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FfmpegCheck {
    Ok,
    /// Does not run, or is not ffmpeg.
    NotFfmpeg,
    /// ffmpeg built without libass (no `subtitles` filter).
    NoLibass,
}

pub fn check_ffmpeg(path: &Path) -> FfmpegCheck {
    let Ok(out) = command(path)
        .args(["-hide_banner", "-filters"])
        .stderr(Stdio::null())
        .output()
    else {
        return FfmpegCheck::NotFfmpeg;
    };
    let list = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() || !list.contains("Filters:") {
        FfmpegCheck::NotFfmpeg
    } else if list
        .lines()
        .any(|l| l.split_whitespace().nth(1) == Some("subtitles"))
    {
        FfmpegCheck::Ok
    } else {
        FfmpegCheck::NoLibass
    }
}

fn command(ffmpeg: &Path) -> Command {
    let mut cmd = Command::new(ffmpeg);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(Stdio::null());
    cmd
}

pub fn probe(ffmpeg: &Path, input: &Path) -> Result<Probe, BurnError> {
    let out = command(ffmpeg)
        .args(["-hide_banner", "-i"])
        .arg(input)
        .output()
        .map_err(BurnError::Spawn)?;
    parse_probe(&String::from_utf8_lossy(&out.stderr)).ok_or(BurnError::NoVideo)
}

/// The first encoder of [`Encoder::ORDER`] that encodes a test frame of
/// `width`×`height` on this PC (a GPU encoder is listed even when the
/// PC has no such GPU, so listing is not enough).
pub fn pick_encoder(ffmpeg: &Path, width: u32, height: u32) -> Option<Encoder> {
    Encoder::ORDER.into_iter().find(|e| {
        command(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg(format!("color=c=black:s={width}x{height}:d=0.2"))
            .args(["-frames:v", "3", "-pix_fmt", "yuv420p"])
            .args(e.args(4_000))
            .args(["-f", "null", "-"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

/// Font for the target language: a file shipped with the OS, handed to
/// libass directly (Windows has no fontconfig setup to find it).
struct Font {
    family: &'static str,
    file: Option<PathBuf>,
}

fn font_for(language: &str) -> Font {
    let base = language.split(['-', '_']).next().unwrap_or("");
    let candidates: &[(&str, &str)] = match base {
        "ja" => &[
            ("Yu Gothic", "YuGothM.ttc"),
            ("Meiryo", "meiryo.ttc"),
            ("MS Gothic", "msgothic.ttc"),
        ],
        "zh" => &[("Microsoft YaHei", "msyh.ttc"), ("SimSun", "simsun.ttc")],
        "ko" => &[("Malgun Gothic", "malgun.ttf")],
        _ => &[("Segoe UI", "segoeui.ttf"), ("Arial", "arial.ttf")],
    };
    if cfg!(windows) {
        let dir = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("Fonts");
        for (family, file) in candidates {
            let p = dir.join(file);
            if p.is_file() {
                return Font {
                    family,
                    file: Some(p),
                };
            }
        }
    }
    let family = match base {
        "ja" => "Noto Sans CJK JP",
        "zh" => "Noto Sans CJK SC",
        "ko" => "Noto Sans CJK KR",
        _ => "sans-serif",
    };
    Font { family, file: None }
}

/// Width of a character in ems, roughly: full width for CJK and kana,
/// a little over half for the rest.
fn char_em(c: char) -> f32 {
    match c as u32 {
        0x1100..=0x115F
        | 0x2E80..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD => 1.0,
        _ if c.is_whitespace() => 0.3,
        _ => 0.58,
    }
}

/// Break `text` into lines no wider than `max_em`, as even as possible.
/// libass only breaks at spaces, so text without them (Japanese,
/// Chinese) would run off the frame.
pub fn wrap(text: &str, max_em: f32) -> Vec<String> {
    let total: f32 = text.chars().map(char_em).sum();
    if total <= max_em {
        return vec![text.to_string()];
    }
    let target = total / (total / max_em).ceil();
    let chars: Vec<char> = text.chars().collect();
    let piece = |a: usize, b: usize| chars[a..b].iter().collect::<String>().trim().to_string();
    let mut out = Vec::new();
    let (mut start, mut width, mut last_break) = (0, 0.0, None);
    for (i, &c) in chars.iter().enumerate() {
        let em = char_em(c);
        if width + em > max_em && i > start {
            let end = last_break.filter(|&b| b > start).unwrap_or(i);
            out.push(piece(start, end));
            width = chars[end..i].iter().copied().map(char_em).sum();
            (start, last_break) = (end, None);
        }
        width += em;
        // A break may follow a space or a CJK character, unless closing
        // punctuation comes next; never inside a Latin word.
        let breakable = c == ' '
            || em == 1.0
                && chars
                    .get(i + 1)
                    .is_none_or(|n| !"、。，．）」』】！？ー…".contains(*n));
        if breakable {
            last_break = Some(i + 1);
            if width >= target && i + 1 < chars.len() {
                out.push(piece(start, i + 1));
                (start, width, last_break) = (i + 1, 0.0, None);
            }
        }
    }
    if start < chars.len() {
        out.push(piece(start, chars.len()));
    }
    out.retain(|l| !l.is_empty());
    out
}

/// Text as ASS dialogue: braces and backslashes would start override
/// codes, so they become their full-width forms.
fn ass_escape(s: &str) -> String {
    s.replace('\\', "＼").replace('{', "｛").replace('}', "｝")
}

fn ass_time(ms: i64) -> String {
    let cs = ms.max(0) / 10;
    format!(
        "{}:{:02}:{:02}.{:02}",
        cs / 360_000,
        cs / 6_000 % 60,
        cs / 100 % 60,
        cs % 100
    )
}

/// The ASS script for `lines` on a `width`×`height` frame. Sizes are in
/// pixels of the frame, so 1080p and 4K look the same.
pub fn render_ass(
    lines: &[SubtitleLine],
    content: ExportContent,
    width: u32,
    height: u32,
    font: &str,
) -> String {
    let size = (height as f32 * 0.055).round().max(12.0);
    let small = (size * 0.75).round();
    let outline = (size * 0.07).max(1.0);
    let margin = (height as f32 * 0.05).round();
    let max_em = width as f32 * 0.9 / size;
    let mut s = String::new();
    let _ = write!(
        s,
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {width}\nPlayResY: {height}\n\
         WrapStyle: 2\nScaledBorderAndShadow: yes\n\n\
         [V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, \
         OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, \
         Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Main,{font},{size},&H00FFFFFF,&H00FFFFFF,&H00000000,&H80000000,0,0,0,0,100,100,\
         0,0,1,{outline:.1},0,2,{margin},{margin},{margin},1\n\n\
         [Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"
    );
    for line in lines {
        let translated = line.translated.trim();
        let original = line.original.trim();
        let body = |text: &str, em: f32| {
            wrap(text, em)
                .iter()
                .map(|l| ass_escape(l))
                .collect::<Vec<_>>()
                .join("\\N")
        };
        let text = match content {
            ExportContent::Original => body(original, max_em),
            ExportContent::Translated if !translated.is_empty() => body(translated, max_em),
            ExportContent::Translated => body(original, max_em),
            ExportContent::Both if translated.is_empty() || translated == original => {
                body(original, max_em)
            }
            ExportContent::Both => format!(
                "{{\\fs{small}}}{}\\N{{\\r}}{}",
                body(original, max_em * size / small),
                body(translated, max_em)
            ),
        };
        if text.is_empty() {
            continue;
        }
        let _ = writeln!(
            s,
            "Dialogue: 0,{},{},Main,,0,0,0,,{text}",
            ass_time(line.start_ms),
            ass_time(line.end_ms)
        );
    }
    s
}

#[derive(Debug, thiserror::Error)]
pub enum BurnError {
    #[error("ffmpeg not found")]
    NoFfmpeg,
    #[error("cannot run ffmpeg: {0}")]
    Spawn(std::io::Error),
    #[error("no video stream")]
    NoVideo,
    #[error("no H.264 encoder works on this PC")]
    NoEncoder,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg failed: {0}")]
    Failed(String),
    #[error("cancelled")]
    Cancelled,
}

pub struct Job<'a> {
    pub ffmpeg: &'a Path,
    pub input: &'a Path,
    pub output: &'a Path,
    pub lines: &'a [SubtitleLine],
    pub content: ExportContent,
    pub language: &'a str,
    /// `None` picks one with [`pick_encoder`].
    pub encoder: Option<Encoder>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub encoder: Encoder,
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u64>,
    pub elapsed_ms: u64,
}

/// Output container for `input`: MP4 for MP4-family input (its audio
/// fits MP4), Matroska for everything else (it takes any audio).
pub fn output_extension(input: &Path) -> &'static str {
    let ext = input
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mp4" | "m4v" | "mov" => "mp4",
        _ => "mkv",
    }
}

/// Burn `job.lines` into `job.output`. `progress` hears the time done so
/// far (ms). Setting `cancel` stops ffmpeg and removes the partial file.
pub fn burn(
    job: &Job,
    mut progress: impl FnMut(u64),
    cancel: &AtomicBool,
) -> Result<Report, BurnError> {
    let started = Instant::now();
    let probe = probe(job.ffmpeg, job.input)?;
    let encoder = match job.encoder {
        Some(e) => e,
        None => pick_encoder(job.ffmpeg, probe.width, probe.height).ok_or(BurnError::NoEncoder)?,
    };
    let font = font_for(job.language);

    // ffmpeg runs inside a work folder and gets plain relative names:
    // filter arguments would need escaping for drive letters and colons.
    let work = std::env::temp_dir().join(format!("moonlens-burn-{}", std::process::id()));
    std::fs::create_dir_all(work.join("fonts"))?;
    let _cleanup = RemoveOnDrop(work.clone());
    std::fs::write(
        work.join("subs.ass"),
        render_ass(
            job.lines,
            job.content,
            probe.width,
            probe.height,
            font.family,
        ),
    )?;
    let mut filter = "subtitles=subs.ass".to_string();
    if let Some(file) = &font.file {
        std::fs::copy(
            file,
            work.join("fonts")
                .join(file.file_name().unwrap_or_default()),
        )?;
        filter.push_str(":fontsdir=fonts");
    }

    // ffmpeg runs in the work folder: relative paths would point there.
    let input = std::path::absolute(job.input)?;
    let output = std::path::absolute(job.output)?;
    let mut cmd = command(job.ffmpeg);
    cmd.current_dir(&work)
        .args(["-hide_banner", "-nostats", "-loglevel", "error", "-y"])
        .args(["-progress", "pipe:1", "-stats_period", "0.5"])
        .args(["-hwaccel", "auto", "-i"])
        .arg(&input)
        .args([
            "-map", "0:v:0", "-map", "0:a?", "-vf", &filter, "-pix_fmt", "yuv420p",
        ])
        .args(encoder.args(target_kbps(&probe)))
        .args(["-c:a", "copy"])
        .arg(&output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(BurnError::Spawn)?;
    let mut stderr = child.stderr.take().expect("piped");
    let errors = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    });
    let stdout = child.stdout.take().expect("piped");
    let mut cancelled = false;
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            cancelled = true;
            break;
        }
        if let Some(us) = line
            .strip_prefix("out_time_us=")
            .and_then(|v| v.parse::<u64>().ok())
        {
            progress(us / 1000);
        }
    }
    let status = child.wait()?;
    let errors = errors.join().unwrap_or_default();
    if cancelled || cancel.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(&output);
        return Err(BurnError::Cancelled);
    }
    if !status.success() {
        let _ = std::fs::remove_file(&output);
        let tail: String = errors
            .lines()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(BurnError::Failed(tail));
    }
    Ok(Report {
        encoder,
        width: probe.width,
        height: probe.height,
        duration_ms: probe.duration_ms,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start_ms: i64, end_ms: i64, original: &str, translated: &str) -> SubtitleLine {
        SubtitleLine {
            seq: 0,
            start_ms,
            end_ms,
            original: original.into(),
            translated: translated.into(),
        }
    }

    #[test]
    fn probe_reads_size_length_and_bitrate() {
        let banner = "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'a.mp4':\n  \
            Duration: 01:02:03.45, start: 0.000000, bitrate: 9000 kb/s\n  \
            Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 3840x2160 [SAR 1:1 DAR 16:9], 8500 kb/s, 30 fps, 30 tbr (default)\n  \
            Stream #0:1[0x2](und): Audio: aac (LC), 48000 Hz, stereo, fltp, 128 kb/s (default)\n";
        assert_eq!(
            parse_probe(banner),
            Some(Probe {
                duration_ms: Some(3_723_450),
                width: 3840,
                height: 2160,
                video_kbps: Some(8500),
                has_audio: true,
            })
        );
    }

    #[test]
    fn probe_skips_cover_art_and_falls_back_to_file_bitrate() {
        let banner = "  Duration: 00:00:10.00, start: 0.000000, bitrate: 320 kb/s\n  \
            Stream #0:0: Audio: mp3, 44100 Hz, stereo, fltp, 320 kb/s\n  \
            Stream #0:1: Video: mjpeg (Baseline), yuvj420p(pc), 600x600, 90k tbr (attached pic)\n";
        assert_eq!(parse_probe(banner), None);
        let banner = "  Duration: 00:00:10.00, start: 0.000000, bitrate: 2000 kb/s\n  \
            Stream #0:0: Video: vp9 (Profile 0), yuv420p(tv), 1920x1080, SAR 1:1 DAR 16:9, 30 fps\n";
        let p = parse_probe(banner).unwrap();
        assert_eq!(
            (p.width, p.height, p.video_kbps, p.has_audio),
            (1920, 1080, Some(2000), false)
        );
    }

    #[test]
    fn wrap_keeps_lines_inside_the_frame_and_even() {
        let text = "本日は晴天なり。".repeat(6);
        let lines = wrap(&text, 20.0);
        assert!(lines.len() >= 3);
        assert_eq!(lines.concat(), text);
        let widths: Vec<f32> = lines.iter().map(|l| l.chars().map(char_em).sum()).collect();
        assert!(widths.iter().all(|&w| w <= 20.0), "{widths:?}");
        let (min, max) = widths
            .iter()
            .fold((f32::MAX, 0f32), |(a, b), &w| (a.min(w), b.max(w)));
        assert!(max - min <= 4.0, "{widths:?}");
    }

    #[test]
    fn wrap_breaks_latin_only_at_spaces() {
        let text = "the quick brown fox jumps over the lazy dog and keeps running far away";
        for l in wrap(text, 16.0) {
            assert!(text.split(' ').any(|w| l.starts_with(w)), "{l}");
            assert!(!l.starts_with(' ') && !l.ends_with(' '));
        }
        assert_eq!(wrap("short", 16.0), vec!["short"]);
    }

    #[test]
    fn wrap_keeps_closing_punctuation_on_its_line() {
        for l in wrap(&"あいうえおかきくけこ、".repeat(5), 12.0) {
            assert!(!l.starts_with('、'), "{l}");
        }
    }

    #[test]
    fn ass_has_events_in_frame_pixels() {
        let lines = [
            line(1_000, 2_500, "Hello {world}", "こんにちは\\世界"),
            line(3_600_000, 3_601_230, "Bye", ""),
        ];
        let ass = render_ass(&lines, ExportContent::Translated, 1920, 1080, "Meiryo");
        assert!(ass.contains("PlayResY: 1080"));
        assert!(ass.contains("Style: Main,Meiryo,59,"));
        assert!(ass.contains("Dialogue: 0,0:00:01.00,0:00:02.50,Main,,0,0,0,,こんにちは＼世界\n"));
        // No translation: the original stands in.
        assert!(ass.contains("Dialogue: 0,1:00:00.00,1:00:01.23,Main,,0,0,0,,Bye\n"));
        let both = render_ass(&lines, ExportContent::Both, 1920, 1080, "Meiryo");
        assert!(both.contains(",,{\\fs44}Hello ｛world｝\\N{\\r}こんにちは＼世界\n"));
    }

    #[test]
    fn output_keeps_mp4_and_uses_matroska_otherwise() {
        assert_eq!(output_extension(Path::new("a/B.MP4")), "mp4");
        assert_eq!(output_extension(Path::new("a.webm")), "mkv");
        assert_eq!(output_extension(Path::new("a.avi")), "mkv");
    }

    /// The whole run with the ffmpeg on this machine, when there is one:
    /// a short clip comes out with the subtitles drawn on its lower part.
    #[test]
    fn burns_subtitles_with_real_ffmpeg() {
        let Some(ffmpeg) = locate_ffmpeg("") else {
            eprintln!("no ffmpeg: skipped");
            return;
        };
        let dir = std::env::temp_dir().join(format!("moonlens-burn-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.mp4");
        let ok = command(&ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
            ])
            .arg("color=c=black:s=320x180:d=2:r=10")
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=d=2",
                "-c:v",
                "mpeg4",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&input)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let output = dir.join("out.mp4");
        let lines = [line(0, 2_000, "Hello", "ＷＷＷＷＷＷ")];
        let mut seen = 0;
        let report = burn(
            &Job {
                ffmpeg: &ffmpeg,
                input: &input,
                output: &output,
                lines: &lines,
                content: ExportContent::Translated,
                language: "ja",
                encoder: None,
            },
            |ms| seen = seen.max(ms),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!((report.width, report.height), (320, 180));
        assert!(seen >= 1_000, "progress {seen}");
        let out = probe(&ffmpeg, &output).unwrap();
        assert!(out.has_audio && out.duration_ms.unwrap() >= 1_900);
        // The bottom band holds white text; the top stays black.
        let gray = |y: u32| {
            let raw = command(&ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-ss", "1", "-i"])
                .arg(&output)
                .args(["-frames:v", "1", "-vf"])
                .arg(format!("crop=320:40:0:{y},format=gray"))
                .args(["-f", "rawvideo", "-"])
                .output()
                .unwrap()
                .stdout;
            raw.iter().copied().max().unwrap_or(0)
        };
        assert!(gray(0) < 40, "top {}", gray(0));
        assert!(gray(130) > 200, "bottom {}", gray(130));
        assert_eq!(check_ffmpeg(&ffmpeg), FfmpegCheck::Ok);
        assert_eq!(check_ffmpeg(&input), FfmpegCheck::NotFfmpeg);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
