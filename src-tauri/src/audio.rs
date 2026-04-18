//! System audio loopback capture for live subtitles.
//!
//! `cpal::Stream` is `!Send`, so we cannot hand it to async code that may
//! migrate across threads. The capture lives on its own dedicated OS
//! thread; control flows through a stop-notify and the audio frames flow
//! through an `mpsc` channel that the rest of the app reads from.
//!
//! Each platform exposes loopback differently:
//! - **Windows**: WASAPI loopback on the default render device.
//! - **macOS**: requires a virtual loopback driver (BlackHole, Loopback);
//!   we look for a device whose name contains "BlackHole" or "Loopback".
//! - **Linux**: PulseAudio exposes a `*.monitor` source per sink.
//!
//! Captured frames are downmixed to mono and resampled to 16 kHz PCM16
//! so they can be fed straight into whisper.cpp / Whisper-compatible
//! servers without further conversion.

#[cfg(feature = "audio")]
pub mod backend {
    use std::sync::Arc;
    use std::thread;

    use anyhow::{anyhow, Context, Result};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{SampleFormat, StreamConfig};
    use parking_lot::Mutex;
    use tokio::sync::mpsc;

    /// One frame of mono PCM16 sampled at 16 kHz.
    pub type AudioFrame = Vec<i16>;

    /// Send-safe handle to a running loopback capture. Drop to stop.
    pub struct LoopbackStream {
        stop: Arc<std::sync::atomic::AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl Drop for LoopbackStream {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    impl LoopbackStream {
        /// Start a dedicated audio capture thread. The thread owns the
        /// cpal stream (which is `!Send`), so the returned handle stays
        /// Send + Sync.
        pub fn start(sink: mpsc::Sender<AudioFrame>) -> Result<Self> {
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stop_t = stop.clone();
            // Use a oneshot for an early bail-out signal from the spawned
            // thread so we surface device-pick errors synchronously.
            let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();
            let thread = thread::Builder::new()
                .name("moonlens-audio".into())
                .spawn(move || {
                    if let Err(e) = run_capture(sink, stop_t, ready_tx.clone()) {
                        let _ = ready_tx.send(Err(e));
                    }
                })?;
            // Wait for the thread to confirm device selection or fail fast.
            ready_rx
                .recv_timeout(std::time::Duration::from_millis(2000))
                .map_err(|e| anyhow!("audio thread did not start: {e}"))??;
            Ok(Self {
                stop,
                thread: Some(thread),
            })
        }
    }

    fn run_capture(
        sink: mpsc::Sender<AudioFrame>,
        stop: Arc<std::sync::atomic::AtomicBool>,
        ready: std::sync::mpsc::Sender<Result<()>>,
    ) -> Result<()> {
        let host = cpal::default_host();
        let device =
            pick_loopback_device(&host).context("no suitable loopback input device found")?;
        let config = device
            .default_input_config()
            .context("no default input config for loopback device")?;
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
                move |data: &[f32], _| forward(data, &sink, &state_cb, |v| pcm16_from_f32(v)),
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
        let _ = ready.send(Ok(()));
        // Park until told to stop; the cpal callback drives output from
        // the audio backend's own threads.
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        drop(stream);
        Ok(())
    }

    /// Lightweight mono / linear-interpolation resampler. Good enough for
    /// speech at 16 kHz; replace with `rubato` if you need higher fidelity.
    pub struct ResamplerState {
        pub source_rate: u32,
        pub source_channels: u16,
        pub fractional: f32,
    }

    fn pcm16_from_f32(v: f32) -> i16 {
        let clamped = v.clamp(-1.0, 1.0);
        (clamped * 32767.0) as i16
    }

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
        let mut mono = Vec::with_capacity(data.len() / channels);
        for frame in data.chunks_exact(channels) {
            let mut acc: i32 = 0;
            for v in frame {
                acc += to_i16(*v) as i32;
            }
            mono.push((acc / channels as i32) as i16);
        }
        let target = 16_000.0_f32;
        let ratio = rate as f32 / target;
        let mut out = Vec::with_capacity((mono.len() as f32 / ratio) as usize + 4);
        let mut idx = fractional;
        while (idx as usize) + 1 < mono.len() {
            let i = idx as usize;
            let frac = idx - i as f32;
            let a = mono[i] as f32;
            let b = mono[i + 1] as f32;
            out.push((a + (b - a) * frac) as i16);
            idx += ratio;
        }
        fractional = idx - mono.len().max(1) as f32 + 1.0;
        if fractional < 0.0 {
            fractional = 0.0;
        }
        state.lock().fractional = fractional;

        if !out.is_empty() {
            let _ = sink.try_send(out);
        }
    }

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

    pub type AudioFrame = Vec<i16>;
    pub struct LoopbackStream;
    impl LoopbackStream {
        pub fn start(_sink: mpsc::Sender<AudioFrame>) -> Result<Self> {
            anyhow::bail!("audio feature disabled at compile time")
        }
    }
}
