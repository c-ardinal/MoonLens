//! Audio capture for live subtitles: a loopback of what the PC plays,
//! or the microphone (face-to-face conversation).
//!
//! Pure resampling/downmix math lives in [`dsp`] so it can be unit tested
//! without a cpal stream.

use serde::{Deserialize, Serialize};

/// What a subtitle window listens to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioSource {
    /// Everything the PC plays on its default output device.
    #[default]
    System,
    /// What one application plays: the process `pid` and the processes
    /// it started (a browser plays through a child process).
    App { pid: u32 },
    /// The default microphone: a face-to-face conversation, translated
    /// in both directions (see `conversation`).
    Microphone,
}

impl AudioSource {
    /// A source that delivers frames all the time, silence included (a
    /// loopback delivers nothing while nothing plays).
    pub fn is_continuous(self) -> bool {
        matches!(self, AudioSource::Microphone)
    }
}

pub mod dsp {
    //! Pure, allocation-friendly DSP helpers for the loopback pipeline.

    /// Convert a normalized float sample in `[-1.0, 1.0]` to a clamped
    /// `i16` suitable for PCM16 output.
    pub fn pcm16_from_f32(v: f32) -> i16 {
        let clamped = v.clamp(-1.0, 1.0);
        (clamped * 32767.0) as i16
    }

    /// Downmix an interleaved multi-channel i16 buffer to mono by
    /// averaging channels per frame. Returns an empty vector if
    /// `channels` is zero or the buffer length is not a clean multiple.
    pub fn downmix_interleaved_i16(data: &[i16], channels: usize) -> Vec<i16> {
        if channels == 0 {
            return Vec::new();
        }
        let frames = data.len() / channels;
        let mut out = Vec::with_capacity(frames);
        for frame in data.chunks_exact(channels) {
            let mut acc: i32 = 0;
            for v in frame {
                acc += *v as i32;
            }
            out.push((acc / channels as i32) as i16);
        }
        out
    }

    /// Linear-interpolating resampler for speech. `fractional` is
    /// in/out: callers thread it across successive callbacks so that
    /// phase is continuous. Output rate is 16 kHz; input rate is given
    /// in `source_rate_hz`.
    pub fn resample_to_16k_linear(
        mono: &[i16],
        source_rate_hz: u32,
        fractional: &mut f32,
    ) -> Vec<i16> {
        if mono.is_empty() || source_rate_hz == 0 {
            return Vec::new();
        }
        let ratio = source_rate_hz as f32 / 16_000.0;
        if ratio <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity((mono.len() as f32 / ratio) as usize + 4);
        let mut idx = *fractional;
        while (idx as usize) + 1 < mono.len() {
            let i = idx as usize;
            let frac = idx - i as f32;
            let a = mono[i] as f32;
            let b = mono[i + 1] as f32;
            out.push((a + (b - a) * frac) as i16);
            idx += ratio;
        }
        // Carry the phase remainder into the next callback.
        let leftover = idx - mono.len().max(1) as f32 + 1.0;
        *fractional = if leftover < 0.0 { 0.0 } else { leftover };
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn pcm16_conversion_rounds_and_clamps() {
            assert_eq!(pcm16_from_f32(0.0), 0);
            assert_eq!(pcm16_from_f32(1.0), 32767);
            assert_eq!(pcm16_from_f32(-1.0), -32767);
            assert_eq!(pcm16_from_f32(2.0), 32767);
            assert_eq!(pcm16_from_f32(-2.0), -32767);
        }

        #[test]
        fn downmix_stereo_averages_channels() {
            let stereo: Vec<i16> = vec![100, 200, 300, 400, -100, -200];
            let mono = downmix_interleaved_i16(&stereo, 2);
            assert_eq!(mono, vec![150, 350, -150]);
        }

        #[test]
        fn downmix_mono_is_identity() {
            let mono_in: Vec<i16> = vec![1, 2, 3, 4];
            let mono = downmix_interleaved_i16(&mono_in, 1);
            assert_eq!(mono, mono_in);
        }

        #[test]
        fn downmix_zero_channels_returns_empty() {
            assert!(downmix_interleaved_i16(&[1, 2, 3], 0).is_empty());
        }

        #[test]
        fn resample_same_rate_is_near_identity() {
            let mut frac = 0.0_f32;
            let input: Vec<i16> = (0..16).map(|i| (i * 1000) as i16).collect();
            let out = resample_to_16k_linear(&input, 16_000, &mut frac);
            // Length should match within 1; values should follow the linear
            // ramp (since our input is exactly linear, interpolation is exact).
            assert!((out.len() as i32 - input.len() as i32).abs() <= 1);
            for (i, v) in out.iter().enumerate() {
                assert_eq!(*v as i32, (i as i32) * 1000);
            }
        }

        #[test]
        fn resample_downsamples_48k_to_16k_by_roughly_three_to_one() {
            let mut frac = 0.0_f32;
            let input: Vec<i16> = vec![0; 480]; // 10ms of silence at 48 kHz
            let out = resample_to_16k_linear(&input, 48_000, &mut frac);
            // 480 / 3 = 160 samples of 16 kHz silence; allow ±1 for phase.
            assert!((159..=161).contains(&out.len()), "got {}", out.len());
            assert!(out.iter().all(|&v| v == 0));
        }

        #[test]
        fn pcm16_from_nan_returns_zero_not_panic() {
            // f32::NaN.clamp(...) returns NaN; casting NaN→i16 yields 0 in
            // Rust. We rely on that to keep the audio pipeline robust when
            // an upstream driver emits NaNs.
            assert_eq!(pcm16_from_f32(f32::NAN), 0);
        }

        #[test]
        fn pcm16_from_infinity_clamps() {
            assert_eq!(pcm16_from_f32(f32::INFINITY), 32767);
            assert_eq!(pcm16_from_f32(f32::NEG_INFINITY), -32767);
        }

        #[test]
        fn downmix_drops_trailing_partial_frame() {
            // 5 samples with 2 channels is 2 complete frames + 1 leftover.
            let stereo: Vec<i16> = vec![10, 20, 30, 40, 99];
            let mono = downmix_interleaved_i16(&stereo, 2);
            assert_eq!(mono, vec![15, 35]);
        }

        #[test]
        fn downmix_71_surround_averages_eight_channels() {
            let frame: Vec<i16> = vec![8, 8, 8, 8, 8, 8, 8, 8];
            assert_eq!(downmix_interleaved_i16(&frame, 8), vec![8]);
        }

        #[test]
        fn resample_with_zero_source_rate_returns_empty() {
            let mut frac = 0.0;
            assert!(resample_to_16k_linear(&[1, 2, 3], 0, &mut frac).is_empty());
        }

        #[test]
        fn resample_empty_input_returns_empty() {
            let mut frac = 0.0;
            assert!(resample_to_16k_linear(&[], 16_000, &mut frac).is_empty());
        }

        #[test]
        fn resample_upsamples_8k_to_16k_roughly_doubling() {
            let mut frac = 0.0;
            let input: Vec<i16> = vec![0; 100];
            let out = resample_to_16k_linear(&input, 8_000, &mut frac);
            // ratio = 0.5 → ~2x samples; allow small rounding slack.
            assert!((195..=210).contains(&out.len()), "got {}", out.len());
        }

        #[test]
        fn resample_single_sample_input_cannot_interpolate() {
            // With only one sample we need at least two to interpolate
            // between, so output must be empty without panicking.
            let mut frac = 0.0;
            let out = resample_to_16k_linear(&[42], 16_000, &mut frac);
            assert!(out.is_empty());
        }

        #[test]
        fn resample_handles_i16_extremes_without_overflow() {
            let mut frac = 0.0;
            let input: Vec<i16> = vec![i16::MIN, i16::MAX, i16::MIN, i16::MAX];
            let out = resample_to_16k_linear(&input, 16_000, &mut frac);
            // The point of this test is that the interpolator does not
            // panic on the i16 extremes: a naive `(a + b) / 2` would
            // overflow when `a = i16::MIN` and `b = i16::MAX`. Reaching
            // this line with a populated output proves the resampler
            // stays inside the type bounds; i16 literally cannot violate
            // its own MIN/MAX so we check shape, not magnitude.
            assert!(!out.is_empty());
        }

        #[test]
        fn resample_preserves_continuity_across_calls() {
            // Two consecutive calls of equal-rate data should produce the
            // same total number of output samples as one big call.
            let input: Vec<i16> = (0..32).map(|i| i as i16 * 100).collect();
            let (a, b) = input.split_at(16);
            let mut frac1 = 0.0_f32;
            let mut out = resample_to_16k_linear(a, 16_000, &mut frac1);
            out.extend(resample_to_16k_linear(b, 16_000, &mut frac1));
            let mut frac2 = 0.0_f32;
            let whole = resample_to_16k_linear(&input, 16_000, &mut frac2);
            // Phase may differ by at most 1 sample at the boundary.
            assert!((out.len() as i32 - whole.len() as i32).abs() <= 1);
        }
    }
}

pub mod gate {
    //! Voice gate for a microphone.
    //!
    //! A microphone delivers frames all the time: room noise between
    //! the sentences, never a pause. The subtitle loop ends a sentence
    //! when frames stop arriving, as a loopback does while nothing
    //! plays. The gate makes a microphone behave the same way: it passes
    //! frames while someone speaks (and a little before and after) and
    //! holds back the rest.
    //!
    //! "Speaking" is judged against the room's own noise floor, tracked
    //! as the quietest recent level, so a noisy microphone does not
    //! count as speech all the time.

    use std::collections::VecDeque;

    /// Speech is this many times louder than the noise floor (~10 dB).
    const OVER_FLOOR: f64 = 3.0;

    /// The threshold never drops below this (RMS): the silence gate of
    /// the subtitle loop.
    pub const MIN_THRESHOLD: f64 = 100.0;

    /// Nor rises above this (about -24 dBFS): ordinary speech into a
    /// microphone stays audible however loud the room is.
    const MAX_THRESHOLD: f64 = 2000.0;

    /// The floor creeps up by this factor per 10 ms above it (doubling
    /// in about 3.5 s) and drops at once to a quieter frame: the gaps
    /// between words bring it back during speech.
    const FLOOR_RISE_PER_10MS: f64 = 1.002;

    /// Kept open this long after the last loud frame (300 ms): the
    /// quiet end of a word, and the gaps between words.
    const HANGOVER_SAMPLES: usize = 4_800;

    /// Loud without a single quieter frame for this long (3 s): not
    /// speech, which dips to the room's level between words, but a floor
    /// that is too low (digital silence, then the microphone comes on:
    /// creeping up from there would pass noise on for many seconds).
    /// The quietest level of the run becomes the floor.
    const LOUD_RUN_SAMPLES: usize = 48_000;

    /// Audio from just before the gate opened that is passed on with it
    /// (200 ms): the quiet onset of the first word.
    const PREROLL_SAMPLES: usize = 3_200;

    #[derive(Default)]
    pub struct VoiceGate {
        /// `None` until the first frame.
        floor: Option<f64>,
        /// Samples the gate stays open for without another loud frame.
        open_for: usize,
        /// Samples since the last frame below the threshold, and the
        /// quietest level among them.
        loud_run: usize,
        loud_min: f64,
        preroll: VecDeque<Vec<i16>>,
        preroll_samples: usize,
    }

    fn rms(samples: &[i16]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
        (sum / samples.len() as f64).sqrt()
    }

    impl VoiceGate {
        /// Level (RMS) from which a frame counts as speech.
        pub fn threshold(&self) -> f64 {
            (self.floor.unwrap_or(0.0) * OVER_FLOOR).clamp(MIN_THRESHOLD, MAX_THRESHOLD)
        }

        /// Takes the next captured frame and returns the frames to pass
        /// on, oldest first: none while nobody speaks.
        pub fn push(&mut self, frame: Vec<i16>) -> Vec<Vec<i16>> {
            if frame.is_empty() {
                return Vec::new();
            }
            let level = rms(&frame);
            let floor = match self.floor {
                Some(floor) if level >= floor => {
                    floor * FLOOR_RISE_PER_10MS.powf(frame.len() as f64 / 160.0)
                }
                _ => level,
            };
            self.floor = Some(floor.max(1.0));
            if level >= self.threshold() {
                self.loud_min = if self.loud_run == 0 {
                    level
                } else {
                    self.loud_min.min(level)
                };
                self.loud_run += frame.len();
                if self.loud_run >= LOUD_RUN_SAMPLES {
                    self.floor = Some(self.loud_min.max(1.0));
                    self.loud_run = 0;
                }
            } else {
                self.loud_run = 0;
            }
            if level >= self.threshold() {
                self.open_for = HANGOVER_SAMPLES;
                let mut out: Vec<Vec<i16>> = self.preroll.drain(..).collect();
                self.preroll_samples = 0;
                out.push(frame);
                return out;
            }
            if self.open_for > 0 {
                self.open_for = self.open_for.saturating_sub(frame.len());
                return vec![frame];
            }
            self.preroll_samples += frame.len();
            self.preroll.push_back(frame);
            while self.preroll_samples > PREROLL_SAMPLES {
                match self.preroll.pop_front() {
                    Some(old) => self.preroll_samples -= old.len(),
                    None => break,
                }
            }
            Vec::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// 10 ms of a square wave at RMS `level`.
        fn frame(level: i16) -> Vec<i16> {
            (0..160)
                .map(|i| if i % 2 == 0 { level } else { -level })
                .collect()
        }

        /// Samples passed on for `count` frames at `level`.
        fn feed(gate: &mut VoiceGate, level: i16, count: usize) -> usize {
            (0..count)
                .map(|_| {
                    gate.push(frame(level))
                        .iter()
                        .map(|f| f.len())
                        .sum::<usize>()
                })
                .sum()
        }

        #[test]
        fn room_noise_is_held_back_and_speech_passes_with_its_onset() {
            let mut gate = VoiceGate::default();
            // One second of room noise: nothing reaches the loop.
            assert_eq!(feed(&mut gate, 60, 100), 0);
            // Speech opens the gate, with the 200 ms before it.
            assert_eq!(feed(&mut gate, 3000, 1), 160 + PREROLL_SAMPLES);
            assert_eq!(feed(&mut gate, 3000, 49), 49 * 160);
        }

        #[test]
        fn the_gate_closes_after_the_hangover_so_the_loop_sees_a_pause() {
            let mut gate = VoiceGate::default();
            feed(&mut gate, 60, 50);
            feed(&mut gate, 3000, 50);
            // Two seconds of noise after the sentence: only the hangover
            // (300 ms) is passed on.
            assert_eq!(feed(&mut gate, 60, 200), HANGOVER_SAMPLES);
        }

        #[test]
        fn short_gaps_between_words_stay_open() {
            let mut gate = VoiceGate::default();
            feed(&mut gate, 60, 50);
            feed(&mut gate, 3000, 30);
            // A 200 ms gap, then the next word: nothing is cut out.
            assert_eq!(feed(&mut gate, 60, 20), 20 * 160);
            assert_eq!(feed(&mut gate, 3000, 30), 30 * 160);
        }

        #[test]
        fn a_noisy_microphone_does_not_count_as_speech() {
            let mut gate = VoiceGate::default();
            // Hiss well above the fixed silence gate (RMS 100).
            assert_eq!(feed(&mut gate, 400, 300), 0);
            assert!(gate.threshold() > 400.0);
            // Speech over the hiss still opens it.
            assert!(feed(&mut gate, 4000, 10) > 0);
        }

        #[test]
        fn the_threshold_recovers_after_long_speech() {
            let mut gate = VoiceGate::default();
            feed(&mut gate, 60, 50);
            // Ten seconds of speech without a gap raise the floor …
            feed(&mut gate, 3000, 1000);
            // … and the first quiet frame brings it back.
            feed(&mut gate, 60, 1);
            assert_eq!(gate.threshold(), MIN_THRESHOLD.max(60.0 * OVER_FLOOR));
        }

        #[test]
        fn noise_after_digital_silence_is_shut_out_within_seconds() {
            let mut gate = VoiceGate::default();
            // A muted microphone, then it comes on in a noisy room.
            feed(&mut gate, 0, 100);
            let passed = feed(&mut gate, 400, 1000);
            assert!(
                passed <= PREROLL_SAMPLES + LOUD_RUN_SAMPLES + HANGOVER_SAMPLES,
                "{passed}"
            );
            assert!(gate.threshold() > 400.0);
            // From then on only speech passes.
            assert_eq!(feed(&mut gate, 400, 100), 0);
            assert!(feed(&mut gate, 4000, 10) > 0);
        }

        #[test]
        fn digital_silence_keeps_the_minimum_threshold() {
            let mut gate = VoiceGate::default();
            assert_eq!(feed(&mut gate, 0, 100), 0);
            assert_eq!(gate.threshold(), MIN_THRESHOLD);
            // Faint hiss right after silence is not speech.
            assert_eq!(feed(&mut gate, 40, 100), 0);
        }
    }
}

pub mod level {
    //! Input level metering for the live subtitle window.
    //!
    //! Captured audio is reduced to one RMS level per [`BIN_SAMPLES`]
    //! (25 ms) so the window can draw what arrives and compare it with
    //! the silence gate of the subtitle loop.

    /// Samples per level bin: 25 ms at 16 kHz (40 bins per second).
    pub const BIN_SAMPLES: usize = 400;

    /// Level reported for digital silence, in dBFS.
    pub const FLOOR_DB: i8 = -96;

    /// RMS of 16-bit samples as dBFS (full scale = 0 dB).
    pub fn dbfs(rms: f64) -> f32 {
        if rms <= 0.0 {
            return FLOOR_DB as f32;
        }
        ((20.0 * (rms / 32768.0).log10()) as f32).clamp(FLOOR_DB as f32, 0.0)
    }

    /// Turns a stream of PCM16 frames into level bins. Frame boundaries
    /// do not matter: a bin completes every [`BIN_SAMPLES`] samples.
    #[derive(Default)]
    pub struct Meter {
        sum_squares: f64,
        count: usize,
        bins: Vec<i8>,
    }

    impl Meter {
        pub fn push(&mut self, samples: &[i16]) {
            for &s in samples {
                self.sum_squares += (s as f64) * (s as f64);
                self.count += 1;
                if self.count == BIN_SAMPLES {
                    let rms = (self.sum_squares / BIN_SAMPLES as f64).sqrt();
                    self.bins.push(dbfs(rms).round() as i8);
                    self.sum_squares = 0.0;
                    self.count = 0;
                }
            }
        }

        /// The bins completed since the last call, oldest first.
        pub fn take(&mut self) -> Vec<i8> {
            std::mem::take(&mut self.bins)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn dbfs_scale() {
            assert_eq!(dbfs(0.0), -96.0);
            assert!((dbfs(32768.0) - 0.0).abs() < 0.01);
            assert!((dbfs(3276.8) + 20.0).abs() < 0.01);
            // The subtitle loop's silence gate (RMS 100) sits near -50 dBFS.
            assert!((dbfs(100.0) + 50.3).abs() < 0.1);
        }

        #[test]
        fn bins_do_not_depend_on_frame_boundaries() {
            let tone: Vec<i16> = (0..BIN_SAMPLES * 3)
                .map(|i| if i % 2 == 0 { 1000 } else { -1000 })
                .collect();
            let mut whole = Meter::default();
            whole.push(&tone);
            let mut pieces = Meter::default();
            for frame in tone.chunks(160) {
                pieces.push(frame);
            }
            // RMS 1000 = -30.3 dBFS.
            assert_eq!(whole.take(), vec![-30, -30, -30]);
            assert_eq!(pieces.take(), vec![-30, -30, -30]);
        }

        #[test]
        fn take_returns_each_bin_once_and_keeps_the_partial_bin() {
            let mut m = Meter::default();
            m.push(&vec![0i16; BIN_SAMPLES + 50]);
            assert_eq!(m.take(), vec![FLOOR_DB]);
            assert!(m.take().is_empty());
            m.push(&vec![0i16; BIN_SAMPLES - 50]);
            assert_eq!(m.take(), vec![FLOOR_DB]);
        }
    }
}

// `cpal::Stream` is `!Send`, so we cannot hand it to async code that may
// migrate across threads. The capture lives on its own dedicated OS thread;
// control flows through a stop-notify and the audio frames flow through an
// `mpsc` channel that the rest of the app reads from.
#[cfg(feature = "audio")]
pub mod backend {
    //! Capture backend: a loopback, or the default microphone.
    //!
    //! Each platform exposes loopback differently:
    //! - **Windows**: WASAPI loopback on the default render device, or a
    //!   process loopback for one application ([`AudioSource::App`]).
    //! - **macOS**: requires a virtual loopback driver (BlackHole, Loopback).
    //! - **Linux**: PulseAudio exposes a `*.monitor` source per sink.
    //!
    //! Captured frames are downmixed to mono and resampled to 16 kHz PCM16
    //! so they can be fed straight into whisper.cpp / Whisper-compatible
    //! servers without further conversion.
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    use anyhow::{anyhow, Context, Result};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{SampleFormat, StreamConfig};
    use parking_lot::Mutex;
    use tokio::sync::mpsc;

    use super::AudioSource;

    /// One frame of mono PCM16 sampled at 16 kHz.
    pub type AudioFrame = Vec<i16>;

    /// Send-safe handle to a running loopback capture. Drop to stop.
    pub struct LoopbackStream {
        stop: Arc<AtomicBool>,
        lost: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
        device: String,
    }

    impl Drop for LoopbackStream {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    impl LoopbackStream {
        /// Capture everything the PC plays.
        pub fn start(sink: mpsc::Sender<AudioFrame>) -> Result<Self> {
            Self::start_with(AudioSource::System, sink)
        }

        /// Start a dedicated audio capture thread for `source`. The
        /// thread owns the stream (cpal's is `!Send`), so the returned
        /// handle stays Send + Sync.
        pub fn start_with(source: AudioSource, sink: mpsc::Sender<AudioFrame>) -> Result<Self> {
            let stop = Arc::new(AtomicBool::new(false));
            let lost = Arc::new(AtomicBool::new(false));
            let (stop_t, lost_t) = (stop.clone(), lost.clone());
            // An early answer from the spawned thread, so that a failure
            // to open the source is returned from here.
            let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<String>>();
            let thread = thread::Builder::new()
                .name("moonlens-audio".into())
                .spawn(move || {
                    let run = match source {
                        AudioSource::System | AudioSource::Microphone => {
                            run_capture(source, sink, stop_t, ready_tx.clone())
                        }
                        AudioSource::App { pid } => {
                            run_app_capture(pid, sink, stop_t, ready_tx.clone())
                        }
                    };
                    if let Err(e) = run {
                        tracing::warn!("audio: capture of {source:?} ended: {e:#}");
                        lost_t.store(true, Ordering::Relaxed);
                        let _ = ready_tx.send(Err(e));
                    }
                })?;
            // Wait for the thread to confirm the source or fail fast.
            let device = ready_rx
                .recv_timeout(std::time::Duration::from_millis(START_TIMEOUT_MS))
                .map_err(|e| {
                    anyhow!(crate::tr!(
                        "audio:the_audio_capture_thread_did_not_respond",
                        e = e
                    ))
                })??;
            Ok(Self {
                stop,
                lost,
                thread: Some(thread),
                device,
            })
        }

        /// Set when the capture ended by itself: the application it
        /// listened to has exited, or its stream failed.
        pub fn lost(&self) -> Arc<AtomicBool> {
            self.lost.clone()
        }

        /// Name of the device (or application) whose audio is captured.
        pub fn device(&self) -> &str {
            &self.device
        }
    }

    /// Activating a process loopback answers through a system callback;
    /// allow for a busy PC.
    const START_TIMEOUT_MS: u64 = 5000;

    fn run_capture(
        source: AudioSource,
        sink: mpsc::Sender<AudioFrame>,
        stop: Arc<AtomicBool>,
        ready: std::sync::mpsc::Sender<Result<String>>,
    ) -> Result<()> {
        let host = cpal::default_host();
        let (device, config) = match source {
            AudioSource::Microphone => pick_microphone(&host)?,
            _ => pick_loopback(&host)?,
        };
        let name = device
            .name()
            .unwrap_or_else(|_| crate::tr!("common:unknown"));
        tracing::info!(
            "audio: capturing \"{name}\" ({} Hz, {} ch, {:?})",
            config.sample_rate().0,
            config.channels(),
            config.sample_format()
        );
        let source_rate = config.sample_rate().0;
        let source_channels = config.channels();
        let sample_format = config.sample_format();
        let stream_config: StreamConfig = config.into();

        let state = Arc::new(Mutex::new(ResamplerState {
            source_rate,
            source_channels,
            fractional: 0.0,
        }));
        let state_cb = state.clone();
        let err_fn = |e| tracing::warn!("audio stream error: {e}");

        let stream = match sample_format {
            SampleFormat::F32 => device.build_input_stream(
                &stream_config,
                move |data: &[f32], _| forward(data, &sink, &state_cb, pcm16_from_f32),
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_input_stream(
                &stream_config,
                move |data: &[i16], _| forward(data, &sink, &state_cb, |v| v),
                err_fn,
                None,
            )?,
            SampleFormat::U16 => device.build_input_stream(
                &stream_config,
                move |data: &[u16], _| {
                    forward(data, &sink, &state_cb, |v| {
                        ((v as i32) - 32768).clamp(i16::MIN as i32, i16::MAX as i32) as i16
                    })
                },
                err_fn,
                None,
            )?,
            other => return Err(anyhow!("unsupported sample format {other:?}")),
        };
        stream.play()?;
        // Tell the caller we are running; from here on errors only get
        // logged to keep the audio thread up.
        let _ = ready.send(Ok(name));
        // Park until told to stop; the cpal callback drives output from
        // the audio backend's own threads.
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        drop(stream);
        Ok(())
    }

    /// Capture what the process `pid` and its child processes play
    /// (Windows 10 2004 or later). The process loopback has no format of
    /// its own: it converts to the one asked for.
    #[cfg(windows)]
    fn run_app_capture(
        pid: u32,
        sink: mpsc::Sender<AudioFrame>,
        stop: Arc<AtomicBool>,
        ready: std::sync::mpsc::Sender<Result<String>>,
    ) -> Result<()> {
        use wasapi::{AudioClient, Direction, SampleType, StreamMode, WaveFormat};

        const RATE: u32 = 48_000;
        const CHANNELS: u16 = 2;
        const BYTES_PER_SAMPLE: usize = 2;

        let name = crate::audio_apps::process_name(pid).ok_or_else(|| {
            anyhow!(crate::tr!(
                "audio:the_target_app_was_not_found_pid",
                pid = pid
            ))
        })?;
        // S_FALSE (already initialized on this thread) is fine as well.
        let _ = wasapi::initialize_mta();
        let format = WaveFormat::new(
            BYTES_PER_SAMPLE * 8,
            BYTES_PER_SAMPLE * 8,
            SampleType::Int,
            RATE as usize,
            CHANNELS as usize,
            None,
        );
        let mut client = AudioClient::new_application_loopback_client(pid, true).map_err(|e| {
            anyhow!(crate::tr!(
                "audio:per_app_audio_capture_is_unsupported_or",
                e = e
            ))
        })?;
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 0,
        };
        client
            .initialize_client(&format, Direction::Capture, mode, None)
            .map_err(|e| {
                anyhow!(crate::tr!(
                    "audio:could_not_initialize_per_app_audio_capture",
                    e = e
                ))
            })?;
        let event = client.set_get_eventhandle().map_err(|e| {
            anyhow!(crate::tr!(
                "audio:could_not_set_the_audio_capture_event",
                e = e
            ))
        })?;
        let capture = client.get_audiocaptureclient().map_err(|e| {
            anyhow!(crate::tr!(
                "audio:could_not_get_the_audio_capture_client",
                e = e
            ))
        })?;
        client
            .start_stream()
            .map_err(|e| anyhow!(crate::tr!("audio:could_not_start_audio_capture_e", e = e)))?;
        tracing::info!(
            "audio: capturing application \"{name}\" (PID {pid}, {RATE} Hz, {CHANNELS} ch)"
        );
        let _ = ready.send(Ok(name.clone()));

        let state = Arc::new(Mutex::new(ResamplerState {
            source_rate: RATE,
            source_channels: CHANNELS,
            fractional: 0.0,
        }));
        let frame_bytes = BYTES_PER_SAMPLE * CHANNELS as usize;
        let mut bytes = std::collections::VecDeque::<u8>::new();
        let mut checked = std::time::Instant::now();
        while !stop.load(Ordering::Relaxed) {
            while capture
                .get_next_packet_size()
                .map_err(|e| anyhow!(crate::tr!("audio:audio_capture_failed_e", e = e)))?
                .unwrap_or(0)
                > 0
            {
                capture
                    .read_from_device_to_deque(&mut bytes)
                    .map_err(|e| anyhow!(crate::tr!("audio:audio_capture_failed_e", e = e)))?;
            }
            let whole = bytes.len() - bytes.len() % frame_bytes;
            if whole > 0 {
                let raw: Vec<u8> = bytes.drain(..whole).collect();
                let samples: Vec<i16> = raw
                    .as_chunks::<BYTES_PER_SAMPLE>()
                    .0
                    .iter()
                    .map(|b| i16::from_le_bytes(*b))
                    .collect();
                forward(&samples, &sink, &state, |v| v);
            }
            // Nothing arrives while the application is silent: the wait
            // then runs out, which is not an error.
            let _ = event.wait_for_event(100);
            if checked.elapsed() >= std::time::Duration::from_secs(1) {
                checked = std::time::Instant::now();
                if !crate::audio_apps::is_alive(pid) {
                    let _ = client.stop_stream();
                    return Err(anyhow!(crate::tr!(
                        "audio:the_target_app_has_exited_name",
                        name = name
                    )));
                }
            }
        }
        let _ = client.stop_stream();
        Ok(())
    }

    #[cfg(not(windows))]
    fn run_app_capture(
        _pid: u32,
        _sink: mpsc::Sender<AudioFrame>,
        _stop: Arc<AtomicBool>,
        _ready: std::sync::mpsc::Sender<Result<String>>,
    ) -> Result<()> {
        Err(anyhow!(crate::tr!(
            "audio:per_app_audio_capture_is_windows_only"
        )))
    }

    /// Lightweight mono / linear-interpolation resampler. Good enough for
    /// speech at 16 kHz; replace with `rubato` if you need higher fidelity.
    pub struct ResamplerState {
        pub source_rate: u32,
        pub source_channels: u16,
        pub fractional: f32,
    }

    use super::dsp;

    fn forward<T: Copy>(
        data: &[T],
        sink: &mpsc::Sender<AudioFrame>,
        state: &Arc<Mutex<ResamplerState>>,
        to_i16: impl Fn(T) -> i16,
    ) {
        let (rate, channels, mut fractional) = {
            let s = state.lock();
            (s.source_rate, s.source_channels as usize, s.fractional)
        };
        if channels == 0 || rate == 0 {
            return;
        }
        // Pre-convert samples to i16, interleaved, so `dsp::downmix_...`
        // can operate on the shared i16 path regardless of source format.
        let mut interleaved: Vec<i16> = Vec::with_capacity(data.len());
        for v in data {
            interleaved.push(to_i16(*v));
        }
        let mono = dsp::downmix_interleaved_i16(&interleaved, channels);
        let out = dsp::resample_to_16k_linear(&mono, rate, &mut fractional);
        state.lock().fractional = fractional;
        if !out.is_empty() {
            let _ = sink.try_send(out);
        }
    }

    fn pcm16_from_f32(v: f32) -> i16 {
        dsp::pcm16_from_f32(v)
    }

    /// The default recording device and its stream format.
    fn pick_microphone(host: &cpal::Host) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
        let device = host
            .default_input_device()
            .context(crate::tr!("audio:no_microphone_found"))?;
        let config = device
            .default_input_config()
            .context(crate::tr!("audio:could_not_read_the_microphones_audio"))?;
        Ok((device, config))
    }

    /// The device to capture and the stream format to open it with.
    ///
    /// Windows (WASAPI): an input stream opened on a *render* device is a
    /// loopback of what that device plays. cpal reports its format as
    /// the output config; `default_input_config` fails on a render
    /// device, so asking for it made the search below fall through to a
    /// microphone and the subtitles never heard the PC's own sound.
    #[cfg(windows)]
    fn pick_loopback(host: &cpal::Host) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
        let device = host
            .default_output_device()
            .context(crate::tr!("audio:no_playback_device_found"))?;
        let config = device.default_output_config().context(crate::tr!(
            "audio:could_not_read_the_playback_devices_audio"
        ))?;
        Ok((device, config))
    }

    /// Linux: PulseAudio / PipeWire expose a `*.monitor` source per sink.
    /// macOS: a virtual loopback driver (BlackHole, Loopback).
    #[cfg(not(windows))]
    fn pick_loopback(host: &cpal::Host) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
        let device = pick_loopback_device(host)
            .context(crate::tr!("audio:no_audio_capture_device_found"))?;
        let config = device
            .default_input_config()
            .context(crate::tr!("audio:could_not_read_the_capture_devices_audio"))?;
        Ok((device, config))
    }

    #[cfg(not(windows))]
    fn pick_loopback_device(host: &cpal::Host) -> Option<cpal::Device> {
        if let Some(dev) = host.default_output_device() {
            if dev.default_input_config().is_ok() {
                return Some(dev);
            }
        }
        let inputs = host.input_devices().ok()?;
        let mut candidates: Vec<cpal::Device> = inputs.collect();
        candidates.sort_by_key(|d| {
            let name = d.name().unwrap_or_default().to_lowercase();
            let score = if name.contains("monitor") {
                0
            } else if name.contains("loopback") || name.contains("blackhole") {
                1
            } else {
                2
            };
            (score, name)
        });
        candidates.into_iter().next()
    }
}

#[cfg(not(feature = "audio"))]
pub mod backend {
    use anyhow::Result;
    use tokio::sync::mpsc;

    use super::AudioSource;

    pub type AudioFrame = Vec<i16>;
    pub struct LoopbackStream;
    impl LoopbackStream {
        pub fn device(&self) -> &str {
            ""
        }
        pub fn lost(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
            Default::default()
        }
        pub fn start(sink: mpsc::Sender<AudioFrame>) -> Result<Self> {
            Self::start_with(AudioSource::System, sink)
        }
        pub fn start_with(_source: AudioSource, _sink: mpsc::Sender<AudioFrame>) -> Result<Self> {
            anyhow::bail!("audio feature disabled at compile time")
        }
    }
}
