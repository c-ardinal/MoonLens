//! System-audio loopback capture. Enabled under the `audio` feature.
//!
//! The plan is to wire `cpal` with WASAPI loopback on Windows, the default
//! input device (configured as BlackHole/monitor) on macOS, and the
//! PulseAudio monitor source on Linux. For the initial skeleton we expose
//! the shape of the API so downstream modules compile.

#[cfg(feature = "audio")]
pub mod backend {
    use anyhow::Result;

    pub struct LoopbackStream;

    impl LoopbackStream {
        pub fn start(_sink: tokio::sync::mpsc::Sender<Vec<i16>>) -> Result<Self> {
            // TODO: build a cpal stream with WASAPI loopback / monitor source.
            Ok(Self)
        }
    }
}

#[cfg(not(feature = "audio"))]
pub mod backend {
    pub struct LoopbackStream;
}
